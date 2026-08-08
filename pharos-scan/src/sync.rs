/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/sync.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Implements write-path device synchronization for unattended pharos-scan.
 * Queries the Pharos inventory server to check record ownership before
 * writing, ensuring scan discoveries never overwrite records owned by
 * other data sources.
 * * Traceability:
 * Related to pharos-scan --auto device discovery mode step 2 device sync.
 * ======================================================================== */

use crate::oui::derive_scan_alias;
use crate::DiscoveredNode;
use pharos_client::{PharosClient, PharosResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    Created,
    Updated,
    Skipped,
    Failed(String),
}

/// Synchronizes a discovered network node with the Pharos inventory server.
///
/// Performs a check-then-write operation: before attempting to write or update a record,
/// it queries the Pharos server to inspect any existing record for the device's identifier
/// (hostname or derived MAC alias). If an existing record is found and its `source` field is
/// owned by a higher-priority or different data source (anything other than `"pharos-scan"`,
/// such as `pharos-pulse`), the write is skipped (`SyncOutcome::Skipped`).
///
/// This ownership check is critical because while the server enforces source field immutability,
/// it does not prevent lower-fidelity scan data (e.g. MAC-OUI manufacturer guesses) from
/// clobbering higher-fidelity fields (e.g. DMI-derived hardware manufacturer) on existing records.
/// Performing the check prior to issuing an `add` command guarantees scan discoveries never
/// overwrite records owned by other producers.
pub async fn sync_discovered_device(
    client: &mut PharosClient,
    node: &DiscoveredNode,
) -> SyncOutcome {
    // 1. Determine the identifier
    let (field_name, identifier) = if let Some(ref h) = node.hostname {
        if !h.trim().is_empty() {
            ("hostname", h.clone())
        } else if let Some(ref m) = node.mac {
            if !m.trim().is_empty() {
                ("alias", derive_scan_alias(m))
            } else {
                return SyncOutcome::Failed(
                    "no hostname or MAC available to identify device".to_string(),
                );
            }
        } else {
            return SyncOutcome::Failed(
                "no hostname or MAC available to identify device".to_string(),
            );
        }
    } else if let Some(ref m) = node.mac {
        if !m.trim().is_empty() {
            ("alias", derive_scan_alias(m))
        } else {
            return SyncOutcome::Failed(
                "no hostname or MAC available to identify device".to_string(),
            );
        }
    } else {
        return SyncOutcome::Failed(
            "no hostname or MAC available to identify device".to_string(),
        );
    };

    // 2. Query for an existing record first
    let query_cmd = format!("query type=\"machine\" {}=\"{}\"", field_name, identifier);
    let query_resp = match client.execute(&query_cmd).await {
        Ok(resp) => resp,
        Err(e) => return SyncOutcome::Failed(e.to_string()),
    };

    // 3. Check ownership
    let record_existed = match query_resp {
        PharosResponse::Matches { ref records, .. } => {
            if let Some(record) = records.first() {
                if record
                    .fields
                    .iter()
                    .any(|f| f.key == "source" && f.value != "pharos-scan")
                {
                    return SyncOutcome::Skipped;
                }
                true
            } else {
                false
            }
        }
        PharosResponse::Ok(_) => false,
        PharosResponse::Error { ref message, .. } => {
            return SyncOutcome::Failed(message.clone());
        }
        PharosResponse::AuthenticationRequired { .. } => {
            return SyncOutcome::Failed("Authentication required for query".to_string());
        }
    };

    // 4. Otherwise, write
    let mut add_cmd = format!(
        "add type=\"machine\" {}=\"{}\" ip_addr=\"{}\"",
        field_name, identifier, node.ip
    );
    if let Some(ref mac) = node.mac {
        if !mac.trim().is_empty() {
            add_cmd.push_str(&format!(" mac_addr=\"{}\"", mac));
        }
    }
    if let Some(ref manufacturer) = node.manufacturer {
        if !manufacturer.trim().is_empty() {
            add_cmd.push_str(&format!(" manufacturer=\"{}\"", manufacturer));
        }
    }

    let write_resp = match client.execute_authenticated(&add_cmd).await {
        Ok(resp) => resp,
        Err(e) => return SyncOutcome::Failed(e.to_string()),
    };

    // 5. Determine the result
    match write_resp {
        PharosResponse::Ok(_) | PharosResponse::Matches { .. } => {
            if record_existed {
                SyncOutcome::Updated
            } else {
                SyncOutcome::Created
            }
        }
        PharosResponse::Error { message, .. } => SyncOutcome::Failed(message),
        other => SyncOutcome::Failed(format!("unexpected response from write command: {:?}", other)),
    }
}
