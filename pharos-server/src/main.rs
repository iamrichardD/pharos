/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the entry point for the pharos backend server. It handles the 
 * lifecycle of the TCP listener and the RFC 2378 protocol implementation.
 * * Traceability:
 * Implements RFC 2378 Section 2.
 * ======================================================================== */

mod protocol;
mod storage;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, error, instrument};
use tracing_subscriber;
use crate::protocol::{Command, parse_command, ProtocolError};
use crate::storage::MemoryStorage;
use std::sync::{Arc, RwLock};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for observability
    tracing_subscriber::fmt::init();

    // In-memory storage for development tier
    let storage = Arc::new(RwLock::new(MemoryStorage::new()));

    let addr = "0.0.0.0:1050"; // Using 1050 for dev to avoid privileged port 105
    let listener = TcpListener::bind(addr).await?;
    info!("Pharos Server listening on {}", addr);

    loop {
        let (socket, _) = listener.accept().await?;
        let storage_ref = Arc::clone(&storage);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, storage_ref).await {
                error!("Error handling connection: {:?}", e);
            }
        });
    }
}

#[instrument(skip(socket, storage))]
async fn handle_connection(mut socket: TcpStream, storage: Arc<RwLock<MemoryStorage>>) -> anyhow::Result<()> {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut client_context: Option<String> = None;

    // Send initial status message as per Ph protocol expectation
    // S: 200:Database ready
    writer.write_all(b"200:Database ready\r\n").await?;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break; // Connection closed
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        info!("Received command: {}", input);

        match parse_command(input) {
            Ok(command) => match command {
                Command::Status => {
                    writer.write_all(b"100:Pharos server active\r\n200:Ok\r\n").await?;
                }
                Command::Id(id) => {
                    client_context = Some(id.to_lowercase());
                    writer.write_all(b"200:Ok\r\n").await?;
                }
                Command::Quit => {
                    writer.write_all(b"200:Bye!\r\n").await?;
                    break;
                }
                Command::Add(fields) => {
                    let mut field_map = std::collections::HashMap::new();
                    for (k, v) in fields {
                        field_map.insert(k, v);
                    }
                    {
                        let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                        lock.add_record(field_map);
                    }
                    writer.write_all(b"200:Ok\r\n").await?;
                }
                Command::Query { selections, returns } => {
                    let default_type = match client_context.as_deref() {
                        Some(ctx) if ctx.contains("ph") => Some(crate::storage::RecordType::Person),
                        Some(ctx) if ctx.contains("mdb") => Some(crate::storage::RecordType::Machine),
                        _ => None,
                    };

                    let records = {
                        let lock = storage.read().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                        lock.query(&selections, default_type).iter().map(|&r| r.clone()).collect::<Vec<_>>()
                    };

                    if records.is_empty() {
                        writer.write_all(b"501:No matches to query\r\n").await?;
                    } else {
                        writer.write_all(format!("102:There were {} matches to your request.\r\n", records.len()).as_bytes()).await?;
                        for (i, record) in records.iter().enumerate() {
                            let index = i + 1;
                            // Sort keys for deterministic output in response lines
                            let mut keys: Vec<&String> = if returns.is_empty() {
                                record.fields.keys().collect()
                            } else {
                                returns.iter().filter(|k| record.fields.contains_key(*k)).collect()
                            };
                            keys.sort();

                            for field_name in keys {
                                let field_val = record.fields.get(field_name).unwrap();
                                let line = format!("-200:{}:{}: {}\r\n", index, field_name, field_val);
                                writer.write_all(line.as_bytes()).await?;
                            }
                        }
                        writer.write_all(b"200:Ok\r\n").await?;
                    }
                }
                _ => {
                    writer.write_all(b"598:Command not yet implemented\r\n").await?;
                }
            },
            Err(ProtocolError::UnknownCommand) => {
                writer.write_all(b"598:Command unknown\r\n").await?;
            }
            Err(ProtocolError::SyntaxError) => {
                writer.write_all(b"599:Syntax error\r\n").await?;
            }
            Err(ProtocolError::InvalidArgument) => {
                writer.write_all(b"512:Illegal value\r\n").await?;
            }
        }
    }

    Ok(())
}
