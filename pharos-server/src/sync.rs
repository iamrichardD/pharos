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

/// Wire-level marker prefixed onto a command before it's replicated to a peer, so the receiving
/// node's `handle_connection` loop can tell "this came from another node's replicate_command, not
/// a real client" and avoid re-replicating it (or notifying on it) forever. Works uniformly for
/// add/change/delete, unlike the old add-only "stuff a fake field into the command" trick, which
/// only worked because add's wire grammar happens to be a flat field=value list - that trick can't
/// be safely extended to change (would silently become another modification) or delete (would
/// silently become another, almost-certainly-nonexistent selection criterion).
pub const SYNC_PREFIX: &str = "SYNC ";

/// Wraps a command for replication to a peer. Pure and total - just string concatenation.
pub fn wrap_for_sync(command: &str) -> String {
    format!("{}{}", SYNC_PREFIX, command)
}

/// Detects and strips the sync marker from a raw wire line. Returns `(is_forwarded, rest)`.
/// Pure and total.
pub fn strip_sync_prefix(line: &str) -> (bool, &str) {
    match line.strip_prefix(SYNC_PREFIX) {
        Some(rest) => (true, rest),
        None => (false, line),
    }
}

pub async fn register_self(storage: Arc<RwLock<dyn Storage>>, addr: &str) -> anyhow::Result<()> {
    info!("Registering self as pharos-server at {}", addr);
    let mut fields = HashMap::new();
    fields.insert("hostname".to_string(), addr.to_string());
    fields.insert("role".to_string(), "pharos-server".to_string());
    fields.insert("type".to_string(), "machine".to_string());
    fields.insert("status".to_string(), "online".to_string());

    let mut lock = storage.write().map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
    lock.upsert_record(fields.into_iter().collect(), None, None)?;
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
            let mut fields: Vec<(String, String)> = Vec::new();
            for field in record.fields {
                fields.push((field.key, field.value));
            }
            // Tag as forwarded to avoid immediate re-replication back to the peer
            fields.push(("forwarded".to_string(), "true".to_string()));
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
    
    let sync_command = wrap_for_sync(&command);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_wrap_any_command_with_sync_prefix() {
        assert_eq!(wrap_for_sync("add hostname=x"), "SYNC add hostname=x");
        assert_eq!(wrap_for_sync("change hostname=x make status=down"), "SYNC change hostname=x make status=down");
        assert_eq!(wrap_for_sync("delete hostname=x"), "SYNC delete hostname=x");
    }

    #[test]
    fn test_should_detect_and_strip_sync_prefix() {
        assert_eq!(strip_sync_prefix("SYNC add hostname=x"), (true, "add hostname=x"));
    }

    #[test]
    fn test_should_not_treat_unprefixed_command_as_forwarded() {
        assert_eq!(strip_sync_prefix("add hostname=x"), (false, "add hostname=x"));
    }

    #[test]
    fn test_should_not_misdetect_a_command_that_merely_contains_sync_as_a_substring() {
        // Must only match a prefix at the very start of the line, not anywhere in it.
        let line = "add hostname=SYNC-server-01 status=up";
        assert_eq!(strip_sync_prefix(line), (false, line));
    }
}
