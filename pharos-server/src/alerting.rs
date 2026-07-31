/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/alerting.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Implements the "Dead Man's Switch" for pulse-tracked machine records: detects
 * when a node's last_seen_at goes stale beyond a configurable threshold without
 * a graceful offline signal, and fires a webhook and/or a local recovery script.
 * * Traceability:
 * Related to Task 15.3 (Issue #61).
 * ======================================================================== */

use crate::storage::{Record, RecordType, Storage};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

/// Tracks which (hostname, last_seen_at) pairs have already triggered an alert, so a
/// continuously-dead node isn't re-alerted on every 5-second check cycle. Keying by the exact
/// last_seen_at value (not just hostname) means a node that comes back online and later goes
/// stale again is correctly re-alerted, with no explicit "clear" step needed - a fresh heartbeat
/// changes last_seen_at, which naturally no longer matches the stored alerted-for value.
///
/// Deliberately in-memory only: alert-suppression state resets on server restart. Acceptable
/// trade-off for a home-lab alerting feature - see the module-level plan/issue for the reasoning.
#[derive(Default)]
pub struct AlertState {
    alerted: HashMap<String, String>,
}

/// Pure staleness check: is `last_seen_at` (RFC3339) older than `threshold_secs` relative to
/// `now`? An unparseable timestamp is treated as NOT stale (fail-safe - never alert on bad data
/// we can't actually interpret).
pub fn is_stale(last_seen_at: &str, now: DateTime<Utc>, threshold_secs: i64) -> bool {
    match last_seen_at.parse::<DateTime<Utc>>() {
        Ok(parsed) => now.signed_duration_since(parsed).num_seconds() > threshold_secs,
        Err(_) => false,
    }
}

/// Pure filter (no I/O, fully testable): returns the machine records that are stale, were never
/// gracefully marked offline, and haven't already been alerted for their current last_seen_at.
pub fn find_newly_stale<'a>(
    records: &'a [Record],
    now: DateTime<Utc>,
    threshold_secs: i64,
    alert_state: &AlertState,
) -> Vec<&'a Record> {
    records
        .iter()
        .filter(|r| {
            let Some(hostname) = r.fields.get("hostname") else { return false; };
            let Some(last_seen_at) = r.fields.get("last_seen_at") else { return false; };
            let status = r.fields.get("status").map(String::as_str).unwrap_or("");

            if status == "offline" {
                return false; // graceful shutdown, not a dead-man's-switch case
            }
            if !is_stale(last_seen_at, now, threshold_secs) {
                return false;
            }
            alert_state.alerted.get(hostname) != Some(last_seen_at)
        })
        .collect()
}

/// POSTs a JSON alert payload to `url`. Never panics; logs a warning on failure. Intended to be
/// called inside a detached `tokio::spawn`, not awaited inline in the health-monitor loop.
pub async fn fire_webhook(url: &str, hostname: &str, last_seen_at: &str, elapsed_secs: i64) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to build HTTP client for presence alert webhook: {}", e);
            return;
        }
    };

    let payload = serde_json::json!({
        "event": "node_down",
        "hostname": hostname,
        "last_seen_at": last_seen_at,
        "elapsed_seconds": elapsed_secs,
    });

    match client.post(url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            info!("Presence alert webhook delivered for {} ({})", hostname, url);
        }
        Ok(resp) => {
            warn!("Presence alert webhook for {} returned non-success status {}: {}", hostname, resp.status(), url);
        }
        Err(e) => {
            warn!("Failed to deliver presence alert webhook for {} to {}: {}", hostname, url, e);
        }
    }
}

/// Spawns `script_path` with `hostname` and `last_seen_at` as arguments (never via a shell
/// string - `hostname` is client-supplied data and must never be shell-interpolated). Does not
/// wait for the script to finish; a recovery script may legitimately take a while. Never panics;
/// logs a warning if the process fails to spawn at all.
pub fn fire_script(script_path: &str, hostname: &str, last_seen_at: &str) {
    match tokio::process::Command::new(script_path)
        .arg(hostname)
        .arg(last_seen_at)
        .spawn()
    {
        Ok(_) => info!("Presence alert recovery script spawned for {}: {}", hostname, script_path),
        Err(e) => warn!("Failed to spawn presence alert recovery script '{}' for {}: {}", script_path, hostname, e),
    }
}

/// Called once per health-monitor tick. Queries all machine records, finds newly-stale ones,
/// fires configured alerts (detached, never blocking this function's caller), and updates
/// `alert_state`. Does nothing if both `webhook_url` and `script_path` are `None`.
pub async fn check_presence(
    storage: &Arc<RwLock<dyn Storage>>,
    alert_state: &mut AlertState,
    threshold_secs: i64,
    webhook_url: Option<&str>,
    script_path: Option<&str>,
) {
    if webhook_url.is_none() && script_path.is_none() {
        return;
    }

    let records = {
        let Ok(lock) = storage.read() else { return; };
        match lock.query(&[], Some(RecordType::Machine)) {
            Ok(r) => r,
            Err(e) => {
                warn!("Presence check: failed to query machine records: {}", e);
                return;
            }
        }
    };

    let now = Utc::now();
    let stale = find_newly_stale(&records, now, threshold_secs, alert_state);

    for record in stale {
        let hostname = record.fields.get("hostname").unwrap().clone();
        let last_seen_at = record.fields.get("last_seen_at").unwrap().clone();
        let elapsed_secs = last_seen_at
            .parse::<DateTime<Utc>>()
            .map(|t| now.signed_duration_since(t).num_seconds())
            .unwrap_or(threshold_secs);

        warn!("Presence alert: {} has not reported in {}s (threshold {}s)", hostname, elapsed_secs, threshold_secs);

        if let Some(url) = webhook_url {
            let url = url.to_string();
            let hostname_c = hostname.clone();
            let last_seen_at_c = last_seen_at.clone();
            tokio::spawn(async move {
                fire_webhook(&url, &hostname_c, &last_seen_at_c, elapsed_secs).await;
            });
        }
        if let Some(path) = script_path {
            fire_script(path, &hostname, &last_seen_at);
        }

        alert_state.alerted.insert(hostname, last_seen_at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as StdHashMap;

    fn make_record(hostname: &str, last_seen_at: &str, status: &str) -> Record {
        let mut fields = StdHashMap::new();
        fields.insert("hostname".to_string(), hostname.to_string());
        fields.insert("last_seen_at".to_string(), last_seen_at.to_string());
        fields.insert("status".to_string(), status.to_string());
        Record {
            id: 1,
            record_type: Some(RecordType::Machine),
            fields,
            owner_fingerprint: None,
            owner_team: None,
        }
    }

    #[test]
    fn test_should_flag_stale_timestamp_when_older_than_threshold() {
        let now = Utc::now();
        let stale_ts = (now - chrono::Duration::seconds(10_000)).to_rfc3339();
        assert!(is_stale(&stale_ts, now, 7200));
    }

    #[test]
    fn test_should_not_flag_fresh_timestamp_as_stale() {
        let now = Utc::now();
        let fresh_ts = (now - chrono::Duration::seconds(10)).to_rfc3339();
        assert!(!is_stale(&fresh_ts, now, 7200));
    }

    #[test]
    fn test_should_treat_unparseable_timestamp_as_not_stale() {
        let now = Utc::now();
        assert!(!is_stale("not-a-timestamp", now, 7200));
    }

    #[test]
    fn test_should_find_stale_online_machine_as_newly_stale() {
        let now = Utc::now();
        let stale_ts = (now - chrono::Duration::seconds(10_000)).to_rfc3339();
        let records = vec![make_record("dead-host", &stale_ts, "online")];
        let alert_state = AlertState::default();
        let result = find_newly_stale(&records, now, 7200, &alert_state);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].fields.get("hostname").unwrap(), "dead-host");
    }

    #[test]
    fn test_should_not_flag_gracefully_offline_machine() {
        let now = Utc::now();
        let stale_ts = (now - chrono::Duration::seconds(10_000)).to_rfc3339();
        let records = vec![make_record("shutdown-host", &stale_ts, "offline")];
        let alert_state = AlertState::default();
        let result = find_newly_stale(&records, now, 7200, &alert_state);
        assert!(result.is_empty());
    }

    #[test]
    fn test_should_not_repeat_alert_for_same_last_seen_at() {
        let now = Utc::now();
        let stale_ts = (now - chrono::Duration::seconds(10_000)).to_rfc3339();
        let records = vec![make_record("dead-host", &stale_ts, "online")];
        let mut alert_state = AlertState::default();
        alert_state.alerted.insert("dead-host".to_string(), stale_ts.clone());
        let result = find_newly_stale(&records, now, 7200, &alert_state);
        assert!(result.is_empty(), "should not re-alert for the same last_seen_at value");
    }

    #[test]
    fn test_should_re_alert_when_node_recovers_and_goes_stale_again() {
        let now = Utc::now();
        let old_stale_ts = (now - chrono::Duration::seconds(20_000)).to_rfc3339();
        let new_stale_ts = (now - chrono::Duration::seconds(10_000)).to_rfc3339();
        let records = vec![make_record("flaky-host", &new_stale_ts, "online")];
        let mut alert_state = AlertState::default();
        // Alerted before for an OLDER last_seen_at - node must have sent at least one heartbeat
        // since then (new_stale_ts is more recent), so this must be treated as a fresh staleness.
        alert_state.alerted.insert("flaky-host".to_string(), old_stale_ts);
        let result = find_newly_stale(&records, now, 7200, &alert_state);
        assert_eq!(result.len(), 1, "a node that recovered and went stale again must be re-alerted");
    }
}
