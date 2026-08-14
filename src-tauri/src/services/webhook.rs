//! services/webhook.rs — Webhook event dispatcher for enterprise automation.
//!
//! Posts events to active `webhook_subscriptions` rows. Uses the OS-provided
//! `curl.exe` (bundled with Windows 10+) so no external HTTP dependency is
//! required. Delivery is best-effort and non-blocking: failures are silently
//! ignored so alerting never interferes with the primary operation.

use rusqlite::{params, Connection};

use crate::core::error::DocForgeError;

/// Dispatches `event_type` to all active subscriptions, returning the number delivered.
pub fn dispatch_webhook_event(
    conn: &Connection,
    event_type: &str,
    payload_json: &str,
) -> Result<usize, DocForgeError> {
    if event_type.is_empty() {
        return Err(DocForgeError::Internal("Event type cannot be empty".to_string()));
    }

    let mut stmt = conn
        .prepare(
            "SELECT target_url, secret FROM webhook_subscriptions
             WHERE event_type = ?1 AND active = 1",
        )
        .map_err(|e| DocForgeError::Internal(format!("Query webhooks: {e}")))?;

    let subs = stmt
        .query_map(params![event_type], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| DocForgeError::Internal(format!("Map webhooks: {e}")))?;

    let mut delivered = 0;
    for sub in subs.flatten() {
        let (url, secret) = sub;
        if post_webhook(&url, &secret, payload_json) {
            delivered += 1;
        }
    }
    Ok(delivered)
}

/// Best-effort HTTP POST via `curl.exe`. Returns true only on a successful response.
fn post_webhook(url: &str, secret: &str, payload: &str) -> bool {
    let mut cmd = std::process::Command::new("curl.exe");
    cmd.args(["-fsS", "-X", "POST", "-H", "Content-Type: application/json"]);
    if !secret.is_empty() {
        cmd.args(["-H", &format!("X-DocForge-Signature: {secret}")]);
    }
    cmd.args(["--data", payload, url]);
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_memory_db;

    #[test]
    fn test_empty_event_rejected() {
        let conn = init_memory_db().expect("init");
        assert!(dispatch_webhook_event(&conn, "", "{}").is_err());
    }

    #[test]
    fn test_no_subscriptions_delivers_zero() {
        let conn = init_memory_db().expect("init");
        // No webhook_subscriptions rows exist, so delivery is a no-op (0).
        let n = dispatch_webhook_event(&conn, "bug.critical", r#"{"id":"bug_x"}"#).unwrap();
        assert_eq!(n, 0);
    }
}
