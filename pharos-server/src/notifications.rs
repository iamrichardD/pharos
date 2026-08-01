/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/notifications.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Implements the Webhook Notification Engine: fires a configurable webhook after
 * every successful add/change/delete, in a generic, Slack-compatible, or
 * Discord-compatible payload format.
 * * Traceability:
 * Related to Task 15.2 (Issue #60).
 * ======================================================================== */

use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tracing::{info, warn};

/// A record-modification event that may trigger a webhook notification.
pub enum NotificationEvent {
    Add {
        fields: HashMap<String, String>,
    },
    Change {
        selections: Vec<(Option<String>, String)>,
        modifications: Vec<(String, String)>,
        count: usize,
    },
    Delete {
        selections: Vec<(Option<String>, String)>,
        count: usize,
    },
}

/// Human-readable one-line summary of an event, shared by the Slack and Discord payload formats.
fn summarize(event: &NotificationEvent) -> String {
    fn selections_to_string(selections: &[(Option<String>, String)]) -> String {
        selections
            .iter()
            .map(|(k, v)| format!("{}={}", k.as_deref().unwrap_or("*"), v))
            .collect::<Vec<_>>()
            .join(", ")
    }

    match event {
        NotificationEvent::Add { fields } => {
            let mut parts: Vec<String> = fields.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            parts.sort();
            format!("Pharos: record added/updated ({})", parts.join(", "))
        }
        NotificationEvent::Change { selections, modifications, count } => {
            let modifications_str = modifications
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Pharos: {} record(s) matching [{}] changed to [{}]",
                count,
                selections_to_string(selections),
                modifications_str
            )
        }
        NotificationEvent::Delete { selections, count } => {
            format!(
                "Pharos: {} record(s) matching [{}] deleted",
                count,
                selections_to_string(selections)
            )
        }
    }
}

fn selections_to_json(selections: &[(Option<String>, String)]) -> serde_json::Value {
    serde_json::Value::Array(
        selections
            .iter()
            .map(|(field, value)| serde_json::json!({ "field": field, "value": value }))
            .collect(),
    )
}

/// Builds the generic (custom-REST-API) JSON payload for an event.
fn generic_payload(event: &NotificationEvent) -> serde_json::Value {
    let timestamp = chrono::Utc::now().to_rfc3339();
    match event {
        NotificationEvent::Add { fields } => serde_json::json!({
            "event": "add",
            "timestamp": timestamp,
            "fields": fields,
        }),
        NotificationEvent::Change { selections, modifications, count } => serde_json::json!({
            "event": "change",
            "timestamp": timestamp,
            "count": count,
            "selections": selections_to_json(selections),
            "modifications": modifications.iter().cloned().collect::<HashMap<String, String>>(),
        }),
        NotificationEvent::Delete { selections, count } => serde_json::json!({
            "event": "delete",
            "timestamp": timestamp,
            "count": count,
            "selections": selections_to_json(selections),
        }),
    }
}

/// Builds the actual JSON body to POST, based on the configured format:
/// - "slack": Slack incoming-webhook compatible (`{"text": "..."}`)
/// - "discord": Discord webhook compatible (`{"content": "..."}`)
/// - anything else (including unset/default): generic custom-REST-API JSON
fn build_payload(event: &NotificationEvent, format: &str) -> serde_json::Value {
    match format {
        "slack" => serde_json::json!({ "text": summarize(event) }),
        "discord" => serde_json::json!({ "content": summarize(event) }),
        _ => generic_payload(event),
    }
}

/// Fires a webhook notification for `event`, if `PHAROS_WEBHOOK_URL` is configured. Does nothing
/// (returns immediately) if it's unset - the feature is opt-in. The actual HTTP POST happens in a
/// detached `tokio::spawn` task, so this function never blocks its caller - call it as a plain,
/// synchronous fire-and-forget statement right after a successful write, no `.await` needed at
/// the call site.
pub fn notify(event: NotificationEvent) {
    let Ok(url) = env::var("PHAROS_WEBHOOK_URL") else {
        return;
    };
    let format = env::var("PHAROS_WEBHOOK_FORMAT").unwrap_or_else(|_| "generic".to_string());
    let payload = build_payload(&event, &format);

    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to build HTTP client for webhook notification: {}", e);
                return;
            }
        };

        match client.post(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Webhook notification delivered to {}", url);
            }
            Ok(resp) => {
                warn!("Webhook notification to {} returned non-success status {}", url, resp.status());
            }
            Err(e) => {
                warn!("Failed to deliver webhook notification to {}: {}", url, e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_build_generic_payload_for_add_event() {
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "vm1".to_string());
        let event = NotificationEvent::Add { fields };
        let payload = build_payload(&event, "generic");
        assert_eq!(payload["event"], "add");
        assert_eq!(payload["fields"]["hostname"], "vm1");
    }

    #[test]
    fn test_should_build_slack_payload_with_text_field() {
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "vm1".to_string());
        let event = NotificationEvent::Add { fields };
        let payload = build_payload(&event, "slack");
        assert!(payload["text"].as_str().unwrap().contains("hostname=vm1"));
        assert!(payload.get("content").is_none());
    }

    #[test]
    fn test_should_build_discord_payload_with_content_field() {
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "vm1".to_string());
        let event = NotificationEvent::Add { fields };
        let payload = build_payload(&event, "discord");
        assert!(payload["content"].as_str().unwrap().contains("hostname=vm1"));
        assert!(payload.get("text").is_none());
    }

    #[test]
    fn test_should_default_to_generic_format_for_unknown_format_string() {
        let event = NotificationEvent::Delete { selections: vec![], count: 1 };
        let payload = build_payload(&event, "not-a-real-format");
        assert_eq!(payload["event"], "delete");
    }

    #[test]
    fn test_should_summarize_change_event_with_selections_and_modifications() {
        let event = NotificationEvent::Change {
            selections: vec![(Some("hostname".to_string()), "vm1".to_string())],
            modifications: vec![("status".to_string(), "down".to_string())],
            count: 1,
        };
        let summary = summarize(&event);
        assert!(summary.contains("hostname=vm1"));
        assert!(summary.contains("status=down"));
        assert!(summary.contains('1'));
    }

    #[test]
    fn test_should_summarize_delete_event_with_count_and_selections() {
        let event = NotificationEvent::Delete {
            selections: vec![(Some("hostname".to_string()), "vm1".to_string())],
            count: 3,
        };
        let summary = summarize(&event);
        assert!(summary.contains("3"));
        assert!(summary.contains("hostname=vm1"));
        assert!(summary.contains("deleted"));
    }

    #[test]
    fn test_should_use_wildcard_marker_for_selection_with_no_field_name() {
        let event = NotificationEvent::Delete {
            selections: vec![(None, "anything".to_string())],
            count: 1,
        };
        let summary = summarize(&event);
        assert!(summary.contains("*=anything"));
    }
}
