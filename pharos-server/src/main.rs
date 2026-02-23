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

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, error, instrument};
use tracing_subscriber;
use crate::protocol::{Command, parse_command, ProtocolError};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for observability
    tracing_subscriber::fmt::init();

    let addr = "0.0.0.0:1050"; // Using 1050 for dev to avoid privileged port 105
    let listener = TcpListener::bind(addr).await?;
    info!("Pharos Server listening on {}", addr);

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                error!("Error handling connection: {:?}", e);
            }
        });
    }
}

#[instrument(skip(socket))]
async fn handle_connection(mut socket: TcpStream) -> anyhow::Result<()> {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

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
                Command::Quit => {
                    writer.write_all(b"200:Bye!\r\n").await?;
                    break;
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
