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
    pub alerted: HashMap<String, String>,
    pub version_mismatches_alerted: HashMap<String, (String, String)>,
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

/// Strips a single leading 'v'/'V' if present, so "v1.10.15" and "1.10.15"
/// compare equal. Used only for comparison — never changes what's stored
/// or displayed.
fn normalize_version(v: &str) -> &str {
    v.strip_prefix(['v', 'V']).unwrap_or(v)
}

/// Returns machine records whose self-reported `version` field disagrees with
/// their own `expected_version` field (both must be present — a record with
/// only one or neither is not a mismatch, just not opted into this check),
/// and haven't already been alerted for this exact (version, expected_version)
/// pair.
pub fn find_version_mismatches<'a>(
    records: &'a [Record],
    alert_state: &AlertState,
) -> Vec<&'a Record> {
    records
        .iter()
        .filter(|r| {
            let Some(hostname) = r.fields.get("hostname") else { return false; };
            let Some(version) = r.fields.get("version") else { return false; };
            let Some(expected_version) = r.fields.get("expected_version") else { return false; };

            if normalize_version(version) == normalize_version(expected_version) {
                return false;
            }

            match alert_state.version_mismatches_alerted.get(hostname) {
                Some((prev_v, prev_exp_v)) => prev_v != version || prev_exp_v != expected_version,
                None => true,
            }
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

/// POSTs a version mismatch alert JSON payload to `url`.
pub async fn fire_version_mismatch_webhook(
    url: &str,
    hostname: &str,
    version: &str,
    expected_version: &str,
) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to build HTTP client for version mismatch webhook: {}", e);
            return;
        }
    };

    let payload = serde_json::json!({
        "event": "version_mismatch",
        "hostname": hostname,
        "version": version,
        "expected_version": expected_version,
    });

    match client.post(url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            info!("Version mismatch alert webhook delivered for {} ({})", hostname, url);
        }
        Ok(resp) => {
            warn!(
                "Version mismatch alert webhook for {} returned non-success status {}: {}",
                hostname,
                resp.status(),
                url
            );
        }
        Err(e) => {
            warn!("Failed to deliver version mismatch alert webhook for {} to {}: {}", hostname, url, e);
        }
    }
}

/// Called once per health-monitor tick. Queries machine records, finds version mismatches,
/// fires configured alerts, and updates `alert_state`.
pub async fn check_version_mismatches(
    storage: &Arc<RwLock<dyn Storage>>,
    alert_state: &mut AlertState,
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
                warn!("Version mismatch check: failed to query machine records: {}", e);
                return;
            }
        }
    };

    let mismatches = find_version_mismatches(&records, alert_state);

    for record in mismatches {
        let hostname = record.fields.get("hostname").unwrap().clone();
        let version = record.fields.get("version").unwrap().clone();
        let expected_version = record.fields.get("expected_version").unwrap().clone();

        warn!(
            "Version mismatch alert: {} has version '{}', expected '{}'",
            hostname, version, expected_version
        );

        if let Some(url) = webhook_url {
            let url = url.to_string();
            let hostname_c = hostname.clone();
            let version_c = version.clone();
            let expected_version_c = expected_version.clone();
            tokio::spawn(async move {
                fire_version_mismatch_webhook(&url, &hostname_c, &version_c, &expected_version_c).await;
            });
        }
        if let Some(path) = script_path {
            let detail = format!("{}:{}", version, expected_version);
            fire_script(path, &hostname, &detail);
        }

        alert_state
            .version_mismatches_alerted
            .insert(hostname, (version, expected_version));
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

    #[test]
    fn test_should_flag_mismatched_version_when_expected_version_differs() {
        let mut fields = StdHashMap::new();
        fields.insert("hostname".to_string(), "console-host".to_string());
        fields.insert("version".to_string(), "v9.9.9-test".to_string());
        fields.insert("expected_version".to_string(), "v1.0.0-different".to_string());
        let record = Record {
            id: 1,
            record_type: Some(RecordType::Machine),
            fields,
            owner_fingerprint: None,
            owner_team: None,
        };

        let alert_state = AlertState::default();
        let records = vec![record];
        let result = find_version_mismatches(&records, &alert_state);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].fields.get("hostname").unwrap(), "console-host");
    }

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("v1.10.15"), "1.10.15");
        assert_eq!(normalize_version("1.10.15"), "1.10.15");
        assert_eq!(normalize_version("V1.10.15"), "1.10.15");
        assert_eq!(normalize_version("2.0.0"), "2.0.0");
    }

    #[test]
    fn test_should_not_flag_matching_version_when_v_prefix_differs() {
        let mut fields = StdHashMap::new();
        fields.insert("hostname".to_string(), "console-host".to_string());
        fields.insert("version".to_string(), "v1.10.15".to_string());
        fields.insert("expected_version".to_string(), "1.10.15".to_string());
        let record = Record {
            id: 1,
            record_type: Some(RecordType::Machine),
            fields,
            owner_fingerprint: None,
            owner_team: None,
        };

        let alert_state = AlertState::default();
        let records = vec![record];
        let result = find_version_mismatches(&records, &alert_state);
        assert!(result.is_empty(), "v1.10.15 and 1.10.15 should not be flagged as mismatch");
    }

    #[test]
    fn test_should_not_flag_matching_version_and_expected_version() {
        let mut fields = StdHashMap::new();
        fields.insert("hostname".to_string(), "console-host".to_string());
        fields.insert("version".to_string(), "v9.9.9-test".to_string());
        fields.insert("expected_version".to_string(), "v9.9.9-test".to_string());
        let record = Record {
            id: 1,
            record_type: Some(RecordType::Machine),
            fields,
            owner_fingerprint: None,
            owner_team: None,
        };

        let alert_state = AlertState::default();
        let records = vec![record];
        let result = find_version_mismatches(&records, &alert_state);
        assert!(result.is_empty());
    }

    #[test]
    fn test_should_ignore_records_missing_version_or_expected_version() {
        let mut fields1 = StdHashMap::new();
        fields1.insert("hostname".to_string(), "host1".to_string());
        fields1.insert("version".to_string(), "v1.0.0".to_string());

        let mut fields2 = StdHashMap::new();
        fields2.insert("hostname".to_string(), "host2".to_string());
        fields2.insert("expected_version".to_string(), "v1.0.0".to_string());

        let records = vec![
            Record { id: 1, record_type: Some(RecordType::Machine), fields: fields1, owner_fingerprint: None, owner_team: None },
            Record { id: 2, record_type: Some(RecordType::Machine), fields: fields2, owner_fingerprint: None, owner_team: None },
        ];

        let alert_state = AlertState::default();
        let result = find_version_mismatches(&records, &alert_state);
        assert!(result.is_empty());
    }

    #[test]
    fn test_should_not_repeat_version_mismatch_alert_for_same_pair() {
        let mut fields = StdHashMap::new();
        fields.insert("hostname".to_string(), "console-host".to_string());
        fields.insert("version".to_string(), "v9.9.9-test".to_string());
        fields.insert("expected_version".to_string(), "v1.0.0-different".to_string());
        let record = Record {
            id: 1,
            record_type: Some(RecordType::Machine),
            fields,
            owner_fingerprint: None,
            owner_team: None,
        };

        let mut alert_state = AlertState::default();
        alert_state.version_mismatches_alerted.insert(
            "console-host".to_string(),
            ("v9.9.9-test".to_string(), "v1.0.0-different".to_string()),
        );

        let records = vec![record];
        let result = find_version_mismatches(&records, &alert_state);
        assert!(result.is_empty(), "should dedup identical (version, expected_version) alerts");
    }

    #[test]
    fn test_should_re_alert_when_expected_version_or_version_changes() {
        let mut fields = StdHashMap::new();
        fields.insert("hostname".to_string(), "console-host".to_string());
        fields.insert("version".to_string(), "v9.9.9-test".to_string());
        fields.insert("expected_version".to_string(), "v2.0.0-new".to_string());
        let record = Record {
            id: 1,
            record_type: Some(RecordType::Machine),
            fields,
            owner_fingerprint: None,
            owner_team: None,
        };

        let mut alert_state = AlertState::default();
        alert_state.version_mismatches_alerted.insert(
            "console-host".to_string(),
            ("v9.9.9-test".to_string(), "v1.0.0-different".to_string()),
        );

        let records = vec![record];
        let result = find_version_mismatches(&records, &alert_state);
        assert_eq!(result.len(), 1, "should re-alert when expected_version changes");
    }
}
