//! services/policy.rs — Enterprise policy configuration overlay service.

use rusqlite::{params, Connection};
use serde_json::Value;
use crate::core::error::DocForgeError;

pub fn set_policy_override(conn: &Connection, key: &str, value: &Value) -> Result<(), DocForgeError> {
    let val_json = serde_json::to_string(value)
        .map_err(|e| DocForgeError::Internal(format!("Serialize policy value: {e}")))?;

    conn.execute(
        "INSERT INTO policy_config (key, value_json) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = datetime('now')",
        params![key, val_json],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert policy_config: {e}")))?;

    Ok(())
}
