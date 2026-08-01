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
pub mod alerting;
pub mod notifications;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, AsyncRead, AsyncWrite};
use tracing::{info, error, instrument};
use crate::protocol::{Command, parse_command, ProtocolError};
use crate::storage::{Storage};
use crate::auth::AuthManager;
use crate::middleware::{MiddlewareChain, ClientContext, MiddlewareAction};
use std::sync::{Arc, RwLock};

fn check_change_limits(
    matched: &[crate::storage::Record],
    modifications: &[(String, String)],
    options: &crate::middleware::SessionOptions,
) -> Result<(), crate::storage::StorageError> {
    if options.limit.is_some_and(|limit| matched.len() > limit) {
        return Err(crate::storage::StorageError::TooManyEntries(matched.len()));
    }
    if options.addonly {
        let overridden = matched.iter().any(|record| {
            modifications.iter().any(|(field, _)| record.fields.contains_key(field))
        });
        if overridden {
            return Err(crate::storage::StorageError::AddOnlyViolation);
        }
    }
    Ok(())
}

fn check_delete_limit(
    matched: &[crate::storage::Record],
    options: &crate::middleware::SessionOptions,
) -> Result<(), crate::storage::StorageError> {
    if options.limit.is_some_and(|limit| matched.len() > limit) {
        return Err(crate::storage::StorageError::TooManyEntries(matched.len()));
    }
    Ok(())
}

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
        options: crate::middleware::SessionOptions::default(),
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

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (is_forwarded, input) = crate::sync::strip_sync_prefix(trimmed);

        if is_forwarded {
            info!("Received command: [SYNC] {}", crate::protocol::redact_wire_line_for_logging(input));
        } else {
            info!("Received command: {}", crate::protocol::redact_wire_line_for_logging(input));
        }

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
                        let mut field_map = std::collections::HashMap::new();
                        for (k, v) in fields {
                            field_map.insert(k.clone(), v.clone());
                        }
                        
                        let team = context.teams.first().cloned();

                        let field_map_for_notification = field_map.clone();
                        let result = {
                            let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            lock.upsert_record(field_map, context.fingerprint.clone(), team)
                        };

                        match result {
                            Ok(_) => {
                                let _ = crate::tui::EVENT_TX.send(format!("[{}] Added/Updated record", context.peer_addr));
                                writer.write_all(b"200:Ok\n").await?;

                                if !is_forwarded {
                                    crate::notifications::notify(crate::notifications::NotificationEvent::Add {
                                        fields: field_map_for_notification,
                                    });
                                }

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
                            lock.query(selections, default_type)
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
                    Command::Change { selections, modifications, force: _ } => {
                        // `force` is parsed but has no effect: it exists in the RFC to permit
                        // overriding fields marked "Encrypt", a concept Pharos's Record/Storage
                        // model doesn't have. Nothing to force-override yet.
                        let result = {
                            let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            if context.options.limit.is_none() && !context.options.addonly {
                                // No session limits configured - skip the extra pre-flight scan.
                                lock.change_record(selections, modifications, context.fingerprint.clone(), &context.teams)
                            } else {
                                match lock.query(selections, None) {
                                    Ok(matched) => match check_change_limits(&matched, modifications, &context.options) {
                                        Ok(()) => lock.change_record(selections, modifications, context.fingerprint.clone(), &context.teams),
                                        Err(e) => Err(e),
                                    },
                                    Err(e) => Err(e),
                                }
                            }
                        };

                        match result {
                            Ok(count) => {
                                if count > 0 {
                                    let noun = if count == 1 { "entry" } else { "entries" };
                                    writer.write_all(format!("200:{} {} changed.\n", count, noun).as_bytes()).await?;

                                    // Replicate change to peers, unless this command was itself a
                                    // replica of another node's change (would otherwise ping-pong
                                    // between peers forever - see Issue #170).
                                    if !is_forwarded && !my_addr.is_empty() {
                                        let storage_clone = Arc::clone(&storage);
                                        let cmd_str = input.to_string();
                                        let my_addr_clone = my_addr.clone();
                                        tokio::spawn(async move {
                                            crate::sync::replicate_command(storage_clone, cmd_str, my_addr_clone).await;
                                        });
                                    }

                                    if !is_forwarded {
                                        crate::notifications::notify(crate::notifications::NotificationEvent::Change {
                                            selections: selections.clone(),
                                            modifications: modifications.clone(),
                                            count,
                                        });
                                    }
                                } else {
                                    writer.write_all(b"501:No matches to change\n").await?;
                                }
                            }
                            Err(crate::storage::StorageError::TooManyEntries(n)) => {
                                writer.write_all(format!("518:Too many entries selected by change command ({} matched)\n", n).as_bytes()).await?;
                            }
                            Err(crate::storage::StorageError::AddOnlyViolation) => {
                                writer.write_all(b"521:Change command would have overridden existing field, and addonly option is on\n").await?;
                            }
                            Err(crate::storage::StorageError::Unauthorized) => {
                                writer.write_all(b"403:Forbidden: Unauthorized record modification\n").await?;
                            }
                            Err(e) => {
                                error!("Storage error: {}", e);
                                writer.write_all(b"500:Internal storage error\n").await?;
                            }
                        }
                    }
                    Command::Delete(selections) => {
                        let result = {
                            let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                            if context.options.limit.is_none() {
                                // No session limit configured - skip the extra pre-flight scan.
                                lock.delete_record(selections, context.fingerprint.clone(), &context.teams)
                            } else {
                                match lock.query(selections, None) {
                                    Ok(matched) => match check_delete_limit(&matched, &context.options) {
                                        Ok(()) => lock.delete_record(selections, context.fingerprint.clone(), &context.teams),
                                        Err(e) => Err(e),
                                    },
                                    Err(e) => Err(e),
                                }
                            }
                        };

                        match result {
                            Ok(count) => {
                                if count > 0 {
                                    writer.write_all(b"200:Ok\n").await?;

                                    // Replicate delete to peers, unless this command was itself a
                                    // replica of another node's delete.
                                    if !is_forwarded && !my_addr.is_empty() {
                                        let storage_clone = Arc::clone(&storage);
                                        let cmd_str = input.to_string();
                                        let my_addr_clone = my_addr.clone();
                                        tokio::spawn(async move {
                                            crate::sync::replicate_command(storage_clone, cmd_str, my_addr_clone).await;
                                        });
                                    }

                                    if !is_forwarded {
                                        crate::notifications::notify(crate::notifications::NotificationEvent::Delete {
                                            selections: selections.clone(),
                                            count,
                                        });
                                    }
                                } else {
                                    writer.write_all(b"501:No matches to delete\n").await?;
                                }
                            }
                            Err(crate::storage::StorageError::TooManyEntries(n)) => {
                                writer.write_all(format!("518:Too many entries selected by delete command ({} matched)\n", n).as_bytes()).await?;
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
                    Command::Set(tokens) => {
                        if tokens.is_empty() {
                            writer.write_all(format!("-200:echo:{}\n", if context.options.echo { "on" } else { "off" }).as_bytes()).await?;
                            writer.write_all(format!("-200:limit:{}\n", match context.options.limit {
                                Some(l) => l.to_string(),
                                None => "off".to_string(),
                            }).as_bytes()).await?;
                            writer.write_all(format!("-200:charset:{}\n", context.options.charset).as_bytes()).await?;
                            writer.write_all(format!("-200:verbose:{}\n", if context.options.verbose { "on" } else { "off" }).as_bytes()).await?;
                            writer.write_all(format!("-200:addonly:{}\n", if context.options.addonly { "on" } else { "off" }).as_bytes()).await?;
                            writer.write_all(format!("-200:nolog:{}\n", if context.options.nolog { "on" } else { "off" }).as_bytes()).await?;
                            writer.write_all(format!("-200:external:{}\n", if context.options.external { "on" } else { "off" }).as_bytes()).await?;
                            writer.write_all(b"200:Done.\n").await?;
                        } else {
                            let mut new_options = context.options.clone();
                            let mut valid = true;
                            for token in tokens {
                                let mut parts = token.splitn(2, '=');
                                let key = parts.next().unwrap_or("").trim().to_lowercase();
                                let val = parts.next().unwrap_or("on").trim();
                                
                                match key.as_str() {
                                    "limit" => {
                                        if val.eq_ignore_ascii_case("off") {
                                            new_options.limit = None;
                                        } else if let Ok(n) = val.parse::<usize>() {
                                            new_options.limit = Some(n);
                                        } else {
                                            valid = false;
                                            break;
                                        }
                                    }
                                    "echo" => {
                                        if val.eq_ignore_ascii_case("on") {
                                            new_options.echo = true;
                                        } else if val.eq_ignore_ascii_case("off") {
                                            new_options.echo = false;
                                        } else {
                                            valid = false;
                                            break;
                                        }
                                    }
                                    "verbose" => {
                                        if val.eq_ignore_ascii_case("on") {
                                            new_options.verbose = true;
                                        } else if val.eq_ignore_ascii_case("off") {
                                            new_options.verbose = false;
                                        } else {
                                            valid = false;
                                            break;
                                        }
                                    }
                                    "addonly" => {
                                        if val.eq_ignore_ascii_case("on") {
                                            new_options.addonly = true;
                                        } else if val.eq_ignore_ascii_case("off") {
                                            new_options.addonly = false;
                                        } else {
                                            valid = false;
                                            break;
                                        }
                                    }
                                    "nolog" => {
                                        if val.eq_ignore_ascii_case("on") {
                                            new_options.nolog = true;
                                        } else if val.eq_ignore_ascii_case("off") {
                                            new_options.nolog = false;
                                        } else {
                                            valid = false;
                                            break;
                                        }
                                    }
                                    "external" => {
                                        if val.eq_ignore_ascii_case("on") {
                                            new_options.external = true;
                                        } else if val.eq_ignore_ascii_case("off") {
                                            new_options.external = false;
                                        } else {
                                            valid = false;
                                            break;
                                        }
                                    }
                                    "charset" => {
                                        // Note: Pharos doesn't actually perform charset conversion.
                                        // This is accepted-and-echoed state only, matching existing project convention of not building unused functionality.
                                        new_options.charset = val.to_string();
                                    }
                                    _ => {
                                        valid = false;
                                        break;
                                    }
                                }
                            }
                            if valid {
                                context.options = new_options;
                                writer.write_all(b"200:Done.\n").await?;
                            } else {
                                writer.write_all(b"512:Illegal value\n").await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Record, StorageError};
    use crate::middleware::SessionOptions;
    use std::collections::HashMap;

    #[test]
    fn test_check_delete_limit() {
        let matched = vec![
            Record { id: 1, record_type: None, fields: HashMap::new(), owner_fingerprint: None, owner_team: None },
            Record { id: 2, record_type: None, fields: HashMap::new(), owner_fingerprint: None, owner_team: None },
        ];
        
        let mut options = SessionOptions::default();
        // Default is None (no limit)
        assert!(check_delete_limit(&matched, &options).is_ok());

        // Limit matches count
        options.limit = Some(2);
        assert!(check_delete_limit(&matched, &options).is_ok());

        // Limit strictly less than count
        options.limit = Some(1);
        match check_delete_limit(&matched, &options) {
            Err(StorageError::TooManyEntries(n)) => assert_eq!(n, 2),
            _ => panic!("Expected TooManyEntries error"),
        }
    }

    #[test]
    fn test_check_change_limits() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "alice".to_string());
        let matched = vec![
            Record { id: 1, record_type: None, fields, owner_fingerprint: None, owner_team: None },
        ];

        let mut options = SessionOptions::default();
        let modifications = vec![("name".to_string(), "bob".to_string())];

        // Default limit/addonly is permissive
        assert!(check_change_limits(&matched, &modifications, &options).is_ok());

        // Addonly checks
        options.addonly = true;
        // Overwriting existing field "name" should fail
        match check_change_limits(&matched, &modifications, &options) {
            Err(StorageError::AddOnlyViolation) => {},
            _ => panic!("Expected AddOnlyViolation error"),
        }

        // Modifying non-existent field should succeed even with addonly
        let new_modifications = vec![("age".to_string(), "30".to_string())];
        assert!(check_change_limits(&matched, &new_modifications, &options).is_ok());
    }
}
