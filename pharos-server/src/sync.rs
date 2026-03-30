/* ========================================================================
 * Project: pharos
 * Component: Server Core - Sync Engine
 * File: pharos-server/src/sync.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements the multi-server synchronization engine. It handles
 * self-registration, peer discovery, and command replication.
 * * Traceability:
 * Related to Task 1.8, implements dynamic peer-to-peer synchronization.
 * ======================================================================== */

use crate::storage::Storage;
use pharos_client::PharosClient;
use std::sync::{Arc, RwLock};
use tracing::{info, error, debug};
use std::collections::HashMap;

pub async fn register_self(storage: Arc<RwLock<dyn Storage>>, addr: &str) -> anyhow::Result<()> {
    info!("Registering self as pharos-server at {}", addr);
    let mut fields = HashMap::new();
    fields.insert("hostname".to_string(), addr.to_string());
    fields.insert("role".to_string(), "pharos-server".to_string());
    fields.insert("type".to_string(), "machine".to_string());
    fields.insert("status".to_string(), "online".to_string());

    let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
    lock.upsert_record(fields, None, None)?;
    Ok(())
}

pub async fn bootstrap(storage: Arc<RwLock<dyn Storage>>, peer_addr: &str) -> anyhow::Result<()> {
    info!("Bootstrapping from peer: {}", peer_addr);
    let mut client = PharosClient::connect(peer_addr, "pharos-sync-bootstrap").await?;
    
    // Query all records
    let resp = client.execute("query").await?;
    if let pharos_client::PharosResponse::Matches { records, .. } = resp {
        info!("Pulling {} records from bootstrap peer", records.len());
        let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
        for record in records {
            let mut fields = HashMap::new();
            for field in record.fields {
                fields.insert(field.key, field.value);
            }
            // Tag as forwarded to avoid immediate re-replication back to the peer
            fields.insert("forwarded".to_string(), "true".to_string());
            lock.upsert_record(fields, None, None)?;
        }
    }
    
    client.quit().await?;
    Ok(())
}

pub async fn replicate_command(storage: Arc<RwLock<dyn Storage>>, command: String, my_addr: String) {
    let peers = {
        let lock = storage.read().unwrap();
        let selections = vec![(Some("role".to_string()), "pharos-server".to_string())];
        match lock.query(&selections, None) {
            Ok(records) => {
                records.into_iter()
                    .filter_map(|r| r.fields.get("hostname").cloned())
                    .filter(|addr| addr != &my_addr) // Don't push to self
                    .collect::<Vec<String>>()
            }
            Err(e) => {
                error!("Sync peer discovery error: {}", e);
                return;
            }
        }
    };

    if peers.is_empty() {
        return;
    }

    debug!("Replicating command to {} peers", peers.len());
    
    // Add forwarded=true to the command if it's an 'add' command
    let sync_command = if command.starts_with("add ") {
        format!("{} forwarded=\"true\"", command)
    } else {
        command
    };

    for peer in peers {
        let cmd = sync_command.clone();
        tokio::spawn(async move {
            match PharosClient::connect(&peer, "pharos-sync").await {
                Ok(mut client) => {
                    if let Err(e) = client.execute_authenticated(&cmd).await {
                        error!("Failed to replicate command to peer {}: {}", peer, e);
                    }
                    let _ = client.quit().await;
                }
                Err(e) => {
                    error!("Failed to connect to peer {} for replication: {}", peer, e);
                }
            }
        });
    }
}
