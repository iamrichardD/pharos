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
pub mod sync;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, AsyncRead, AsyncWrite};
use tracing::{info, error, instrument};
use crate::protocol::{Command, parse_command, ProtocolError};
use crate::storage::{Storage};
use crate::auth::AuthManager;
use crate::middleware::{MiddlewareChain, ClientContext, MiddlewareAction};
use std::sync::{Arc, RwLock};

#[instrument(skip(socket, storage, auth_manager, middleware_chain))]
pub async fn handle_connection<S>(socket: S, peer_addr: String, storage: Arc<RwLock<dyn Storage>>, auth_manager: Arc<AuthManager>, middleware_chain: Arc<MiddlewareChain>) -> anyhow::Result<()> 
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static
{
    let (reader, mut writer) = tokio::io::split(socket);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let mut context = ClientContext {
        id: None,
        authenticated: false,
        peer_addr: peer_addr.clone(),
        roles: Vec::new(),
        teams: Vec::new(),
        tier: crate::auth::SecurityTier::Open,
        login_alias: None,
        fingerprint: None,
    };

    let _ = crate::tui::EVENT_TX.send(format!("Connection established from {}", peer_addr));

    // Send initial status message as per Ph protocol expectation
    // S: 200:Database ready
    writer.write_all(b"200:Database ready\n").await?;

    let my_addr = std::env::var("PHAROS_SYNC_ADDR").unwrap_or_default();

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
                        writer.write_all(b"599:Internal server error (middleware)\n").await?;
                        continue;
                    }
                }

                match &command {
                    Command::Status => {
                        writer.write_all(b"100:Pharos server active\n200:Ok\n").await?;
                    }
                    Command::Id(id) => {
                        context.id = Some(id.to_lowercase());
                        writer.write_all(b"200:Ok\n").await?;
                    }
                    Command::Login(alias) => {
                        let challenge = auth_manager.generate_challenge(alias);
                        context.login_alias = Some(alias.clone());
                        writer.write_all(format!("301:{}\n", challenge).as_bytes()).await?;
                    }
                    Command::Auth { public_key, signature } => {
                        let challenge = context.login_alias.as_ref()
                            .and_then(|alias| auth_manager.get_challenge(alias));

                        if let Some(challenge) = challenge {
                            if let Some(fingerprint) = auth_manager.verify_with_fingerprint(public_key, signature, &challenge) {
                                if let Some(alias) = &context.login_alias {
                                    auth_manager.consume_challenge(alias);
                                }
                                context.authenticated = true;
                                context.roles = auth_manager.get_roles(public_key);
                                context.teams = auth_manager.get_teams(public_key);
                                context.fingerprint = Some(fingerprint);
                                writer.write_all(b"200:Ok\n").await?;
                            } else {
                                writer.write_all(b"403:Forbidden\n").await?;
                            }
                        } else {
                            writer.write_all(b"506:Request refused; must be logged in to execute (Challenge expired or not found)\n").await?;
                        }
                    }
                    Command::AuthCheck { public_key, signature, challenge } => {
                        if auth_manager.verify(public_key, signature, challenge) {
                            writer.write_all(b"200:Ok\n").await?;
                        } else {
                            writer.write_all(b"403:Forbidden\n").await?;
                        }
                    }
                    Command::Quit => {
                        writer.write_all(b"200:Bye!\n").await?;
                        break;
                    }
                    Command::Add(fields) => {
                        let is_forwarded = fields.iter().any(|(k, v)| k == "forwarded" && v == "true");

                        let mut field_map = std::collections::HashMap::new();
                        for (k, v) in fields {
                            field_map.insert(k.clone(), v.clone());
                        }
                        
                        let team = context.teams.first().cloned();

                        let result = {
                            let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            lock.upsert_record(field_map, context.fingerprint.clone(), team)
                        };

                        match result {
                            Ok(_) => {
                                let _ = crate::tui::EVENT_TX.send(format!("[{}] Added/Updated record", context.peer_addr));
                                writer.write_all(b"200:Ok\n").await?;

                                // Replicate to peers if not already forwarded
                                if !is_forwarded && !my_addr.is_empty() {
                                    let storage_clone = Arc::clone(&storage);
                                    let cmd_str = input.to_string();
                                    let my_addr_clone = my_addr.clone();
                                    tokio::spawn(async move {
                                        crate::sync::replicate_command(storage_clone, cmd_str, my_addr_clone).await;
                                    });
                                }
                            }
                            Err(crate::storage::StorageError::Collision) | Err(crate::storage::StorageError::Unauthorized) => {
                                writer.write_all(b"403:Forbidden: Unauthorized record modification\n").await?;
                            }
                            Err(e) => {
                                error!("Storage error: {}", e);
                                writer.write_all(b"500:Internal storage error\n").await?;
                            }
                        }
                    }
                    Command::Query { selections, returns } => {
                        let default_type = match context.id.as_deref() {
                            Some(ctx) if ctx.contains("ph") => Some(crate::storage::RecordType::Person),
                            Some(ctx) if ctx.contains("mdb") => Some(crate::storage::RecordType::Machine),
                            _ => None,
                        };

                        let query_result = {
                            let lock = storage.read().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            lock.query(&selections, default_type)
                        };

                        let (records, count) = match query_result {
                            Ok(results) => {
                                let count = results.len();
                                (results, count)
                            }
                            Err(crate::storage::StorageError::InvalidArgument(msg)) => {
                                writer.write_all(format!("421:Invalid argument: {}\n", msg).as_bytes()).await?;
                                continue;
                            }
                            Err(e) => {
                                error!("Query error: {}", e);
                                writer.write_all(b"500:Internal storage error\n").await?;
                                continue;
                            }
                        };

                        let _ = crate::tui::EVENT_TX.send(format!("[{}] Queried records, matches: {}", context.peer_addr, count));

                        if records.is_empty() {
                            writer.write_all(b"501:No matches to query\n").await?;
                        } else {
                            writer.write_all(format!("102:There were {} matches to your request.\n", count).as_bytes()).await?;
                            for (i, record) in records.iter().enumerate() {
                                let index = i + 1;
                                let mut keys: Vec<&String> = if returns.is_empty() {
                                    record.fields.keys().collect()
                                } else {
                                    returns.iter().filter(|k| record.fields.contains_key(*k)).collect()
                                };
                                keys.sort();

                                for field_name in keys {
                                    let field_val = record.fields.get(field_name).unwrap();
                                    let line = format!("-200:{}:{}: {}\n", index, field_name, field_val);
                                    writer.write_all(line.as_bytes()).await?;
                                }
                            }
                            writer.write_all(b"200:Ok\n").await?;
                        }
                    }
                    Command::Delete(selections) => {
                        let result = {
                            let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            lock.delete_record(selections, context.fingerprint.clone(), &context.teams)
                        };

                        match result {
                            Ok(count) => {
                                if count > 0 {
                                    writer.write_all(b"200:Ok\n").await?;

                                    // Replicate delete to peers
                                    if !my_addr.is_empty() {
                                        let storage_clone = Arc::clone(&storage);
                                        let cmd_str = input.to_string();
                                        let my_addr_clone = my_addr.clone();
                                        tokio::spawn(async move {
                                            crate::sync::replicate_command(storage_clone, cmd_str, my_addr_clone).await;
                                        });
                                    }
                                } else {
                                    writer.write_all(b"501:No matches to delete\n").await?;
                                }
                            }
                            Err(crate::storage::StorageError::Unauthorized) => {
                                writer.write_all(b"403:Forbidden: Unauthorized record deletion\n").await?;
                            }
                            Err(e) => {
                                error!("Storage error: {}", e);
                                writer.write_all(b"500:Internal storage error\n").await?;
                            }
                        }
                    }
                    _ => {
                        writer.write_all(b"598:Command not yet implemented\n").await?;
                    }
                }

                // Post-processing
                middleware_chain.post_process(&command, &context);
            }
            Err(ProtocolError::UnknownCommand) => {
                writer.write_all(b"598:Command unknown\n").await?;
            }
            Err(ProtocolError::SyntaxError) => {
                writer.write_all(b"599:Syntax error\n").await?;
            }
            Err(ProtocolError::InvalidArgument) => {
                writer.write_all(b"512:Illegal value\n").await?;
            }
        }
    }

    Ok(())
}
