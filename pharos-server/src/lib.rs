/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/lib.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the library entry point for the pharos backend server. It exports
 * the core components like protocol, storage, metrics, auth, and middleware.
 * * Traceability:
 * Related to GitHub Issue #33.
 * ======================================================================== */

pub mod protocol;
pub mod storage;
pub mod metrics;
pub mod auth;
pub mod middleware;
pub mod tui;

use tokio::net::{TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, error, instrument};
use crate::protocol::{Command, parse_command, ProtocolError};
use crate::storage::{Storage};
use crate::auth::AuthManager;
use crate::middleware::{MiddlewareChain, ClientContext, MiddlewareAction};
use std::sync::{Arc, RwLock};
use rand::rngs::OsRng;
use rand::RngCore;
use hex;

#[instrument(skip(socket, storage, auth_manager, middleware_chain))]
pub async fn handle_connection(mut socket: TcpStream, storage: Arc<RwLock<dyn Storage>>, auth_manager: Arc<AuthManager>, middleware_chain: Arc<MiddlewareChain>) -> anyhow::Result<()> {
    let peer_addr = socket.peer_addr()?.to_string();
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut context = ClientContext {
        id: None,
        authenticated: false,
        peer_addr: peer_addr.clone(),
    };
    let mut challenge = vec![0u8; 16];
    OsRng.fill_bytes(&mut challenge);
    let challenge_hex = hex::encode(challenge);

    let _ = crate::tui::EVENT_TX.send(format!("Connection established from {}", peer_addr));

    // Send initial status message as per Ph protocol expectation
    // S: 200:Database ready
    writer.write_all(b"200:Database ready
").await?;

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
            Ok(mut command) => {
                // Execute Middleware Chain (Pre-processing)
                match middleware_chain.pre_process(&mut command, &mut context) {
                    Ok(MiddlewareAction::ShortCircuit(resp)) => {
                        writer.write_all(resp.as_bytes()).await?;
                        continue;
                    }
                    Ok(MiddlewareAction::Continue) => {}
                    Err(e) => {
                        error!("Middleware error: {:?}", e);
                        writer.write_all(b"599:Internal server error (middleware)
").await?;
                        continue;
                    }
                }

                match &command {
                    Command::Status => {
                        writer.write_all(b"100:Pharos server active
200:Ok
").await?;
                    }
                    Command::Id(id) => {
                        context.id = Some(id.to_lowercase());
                        writer.write_all(b"200:Ok
").await?;
                    }
                    Command::Auth { public_key, signature } => {
                        if auth_manager.verify(public_key, signature, &challenge_hex) {
                            context.authenticated = true;
                            writer.write_all(b"200:Ok
").await?;
                        } else {
                            writer.write_all(b"403:Forbidden
").await?;
                        }
                    }
                    Command::Quit => {
                        writer.write_all(b"200:Bye!
").await?;
                        break;
                    }
                    Command::Add(fields) => {
                        if !context.authenticated {
                            writer.write_all(format!("401:Authentication required. Challenge: {}
", challenge_hex).as_bytes()).await?;
                            continue;
                        }
                        let mut field_map = std::collections::HashMap::new();
                        for (k, v) in fields {
                            field_map.insert(k.clone(), v.clone());
                        }
                        {
                            let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            lock.add_record(field_map);
                        }
                        let _ = crate::tui::EVENT_TX.send(format!("[{}] Added new record", context.peer_addr));
                        writer.write_all(b"200:Ok
").await?;
                    }
                    Command::Query { selections, returns } => {
                        let default_type = match context.id.as_deref() {
                            Some(ctx) if ctx.contains("ph") => Some(crate::storage::RecordType::Person),
                            Some(ctx) if ctx.contains("mdb") => Some(crate::storage::RecordType::Machine),
                            _ => None,
                        };

                        let (records, count) = {
                            let lock = storage.read().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            let results = lock.query(&selections, default_type);
                            let count = results.len();
                            (results, count)
                        };

                        let _ = crate::tui::EVENT_TX.send(format!("[{}] Queried records, matches: {}", context.peer_addr, count));

                        if records.is_empty() {
                            writer.write_all(b"501:No matches to query
").await?;
                        } else {
                            writer.write_all(format!("102:There were {} matches to your request.
", count).as_bytes()).await?;
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
                                    let line = format!("-200:{}:{}: {}
", index, field_name, field_val);
                                    writer.write_all(line.as_bytes()).await?;
                                }
                            }
                            writer.write_all(b"200:Ok
").await?;
                        }
                    }
                    Command::Delete(_) => {
                        if !context.authenticated {
                            writer.write_all(format!("401:Authentication required. Challenge: {}
", challenge_hex).as_bytes()).await?;
                            continue;
                        }
                        writer.write_all(b"598:Command not yet implemented
").await?;
                    }
                    Command::Change { .. } => {
                        if !context.authenticated {
                            writer.write_all(format!("401:Authentication required. Challenge: {}
", challenge_hex).as_bytes()).await?;
                            continue;
                        }
                        writer.write_all(b"598:Command not yet implemented
").await?;
                    }
                    _ => {
                        writer.write_all(b"598:Command not yet implemented
").await?;
                    }
                }

                // Post-processing
                middleware_chain.post_process(&command, &context);
            }
            Err(ProtocolError::UnknownCommand) => {
                writer.write_all(b"598:Command unknown
").await?;
            }
            Err(ProtocolError::SyntaxError) => {
                writer.write_all(b"599:Syntax error
").await?;
            }
            Err(ProtocolError::InvalidArgument) => {
                writer.write_all(b"512:Illegal value
").await?;
            }
        }
    }

    Ok(())
}
