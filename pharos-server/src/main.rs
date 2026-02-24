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
mod metrics;
mod auth;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, error, instrument};
use tracing_subscriber;
use crate::protocol::{Command, parse_command, ProtocolError};
use crate::storage::{Storage, MemoryStorage, FileStorage, LdapStorage};
use crate::metrics::{CPU_USAGE, MEMORY_USAGE_BYTES, TOTAL_RECORDS, gather_metrics, check_health_thresholds};
use crate::auth::AuthManager;
use std::sync::{Arc, RwLock};
use sysinfo::System;
use warp::Filter;
use std::time::Duration;
use std::path::PathBuf;
use std::env;
use rand::RngCore;
use rand::rngs::OsRng;
use hex;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for observability
    tracing_subscriber::fmt::init();

    // Determine storage backend based on environment variables
    let storage: Arc<RwLock<dyn Storage>> = if let Ok(url) = env::var("PHAROS_LDAP_URL") {
        info!("Initializing LdapStorage at {}", url);
        let bind_dn = env::var("PHAROS_LDAP_BIND_DN").unwrap_or_default();
        let bind_pw = env::var("PHAROS_LDAP_BIND_PW").unwrap_or_default();
        let base_dn = env::var("PHAROS_LDAP_BASE_DN").unwrap_or_default();
        Arc::new(RwLock::new(LdapStorage::new(url, bind_dn, bind_pw, base_dn)))
    } else if let Ok(path) = env::var("PHAROS_STORAGE_PATH") {
        info!("Initializing FileStorage at {:?}", path);
        Arc::new(RwLock::new(FileStorage::new(PathBuf::from(path))))
    } else {
        info!("Initializing in-memory storage (Development Tier)");
        Arc::new(RwLock::new(MemoryStorage::new()))
    };

    // Initialize AuthManager
    let keys_dir = env::var("PHAROS_KEYS_DIR").unwrap_or_else(|_| "/home/rdelgado/.ssh/keys".to_string());
    let auth_manager = Arc::new(AuthManager::new(Path::new(&keys_dir)));

    // --- Metrics Scrape Server (Pull Method) ---
    let storage_for_metrics: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
    let metrics_route = warp::path("metrics").map(move || {
        // Update storage count on scrape
        if let Ok(lock) = storage_for_metrics.read() {
            TOTAL_RECORDS.set(lock.record_count() as i64);
        }
        gather_metrics()
    });
    
    tokio::spawn(async move {
        info!("Prometheus metrics server starting on 0.0.0.0:9090/metrics");
        warp::serve(metrics_route).run(([0, 0, 0, 0], 9090)).await;
    });

    // --- Background Metrics Collection & Health Monitoring ---
    let storage_for_monitor: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
    tokio::spawn(async move {
        let mut sys = System::new_all();
        loop {
            // Update system info
            sys.refresh_all();
            
            // Record CPU Usage (average over all CPUs)
            let cpu_load: f32 = sys.cpus().iter().map(|cpu: &sysinfo::Cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32;
            CPU_USAGE.set(cpu_load as f64);

            // Record Memory Usage
            let used_mem = sys.used_memory();
            MEMORY_USAGE_BYTES.set(used_mem as i64);

            // Record Storage Count
            if let Ok(lock) = storage_for_monitor.read() {
                TOTAL_RECORDS.set(lock.record_count() as i64);
            }

            // Health Monitor Threshold Warnings
            // CPU > 90% or Memory > 1GB (Arbitrary for MVP demonstration)
            check_health_thresholds(90.0, 1024 * 1024 * 1024);

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    let addr = "0.0.0.0:1050"; // Using 1050 for dev to avoid privileged port 105
    let listener = TcpListener::bind(addr).await?;
    info!("Pharos Server listening on {}", addr);

    loop {
        let (socket, _) = listener.accept().await?;
        let storage_ref: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
        let auth_ref = Arc::clone(&auth_manager);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, storage_ref, auth_ref).await {
                error!("Error handling connection: {:?}", e);
            }
        });
    }
}

#[instrument(skip(socket, storage, auth_manager))]
async fn handle_connection(mut socket: TcpStream, storage: Arc<RwLock<dyn Storage>>, auth_manager: Arc<AuthManager>) -> anyhow::Result<()> {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut client_context: Option<String> = None;
    let mut authenticated = false;
    let mut challenge = vec![0u8; 16];
    OsRng.fill_bytes(&mut challenge);
    let challenge_hex = hex::encode(challenge);

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
                Command::Auth { public_key, signature } => {
                    if auth_manager.verify(&public_key, &signature, &challenge_hex) {
                        authenticated = true;
                        writer.write_all(b"200:Ok\r\n").await?;
                    } else {
                        writer.write_all(b"403:Forbidden\r\n").await?;
                    }
                }
                Command::Quit => {
                    writer.write_all(b"200:Bye!\r\n").await?;
                    break;
                }
                Command::Add(fields) => {
                    if !authenticated {
                        writer.write_all(format!("401:Authentication required. Challenge: {}\r\n", challenge_hex).as_bytes()).await?;
                        continue;
                    }
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

                    let (records, count) = {
                        let lock = storage.read().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                        let results = lock.query(&selections, default_type);
                        let count = results.len();
                        (results, count)
                    };

                    if records.is_empty() {
                        writer.write_all(b"501:No matches to query\r\n").await?;
                    } else {
                        writer.write_all(format!("102:There were {} matches to your request.\r\n", count).as_bytes()).await?;
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
                Command::Delete(_) => {
                    if !authenticated {
                        writer.write_all(format!("401:Authentication required. Challenge: {}\r\n", challenge_hex).as_bytes()).await?;
                        continue;
                    }
                    writer.write_all(b"598:Command not yet implemented\r\n").await?;
                }
                Command::Change { .. } => {
                    if !authenticated {
                        writer.write_all(format!("401:Authentication required. Challenge: {}\r\n", challenge_hex).as_bytes()).await?;
                        continue;
                    }
                    writer.write_all(b"598:Command not yet implemented\r\n").await?;
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


