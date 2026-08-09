//! services/telemetry.rs — Consent-gated aggregate telemetry service.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::core::error::DocForgeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConsentState {
    pub opt_in: bool,
    pub crash_reports: bool,
}

pub struct TelemetryService;

impl TelemetryService {
    pub fn get_consent(conn: &Connection) -> Result<TelemetryConsentState, DocForgeError> {
        let mut stmt = conn
            .prepare("SELECT opt_in, crash_reports FROM telemetry_consent WHERE id = 'default'")
            .map_err(|e| DocForgeError::StorageIo(format!("Query consent: {e}")))?;

        let res = stmt.query_row([], |row| {
            let opt_in: i32 = row.get(0)?;
            let crash_reports: i32 = row.get(1)?;
            Ok(TelemetryConsentState {
                opt_in: opt_in != 0,
                crash_reports: crash_reports != 0,
            })
        });

        match res {
            Ok(state) => Ok(state),
            Err(_) => Ok(TelemetryConsentState {
                opt_in: false,
                crash_reports: false,
            }),
        }
    }

    pub fn set_consent(
        conn: &Connection,
        opt_in: bool,
        crash_reports: bool,
    ) -> Result<(), DocForgeError> {
        conn.execute(
            "INSERT INTO telemetry_consent (id, opt_in, crash_reports)
             VALUES ('default', ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET opt_in = ?1, crash_reports = ?2, updated_at = datetime('now')",
            params![if opt_in { 1 } else { 0 }, if crash_reports { 1 } else { 0 }],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Update consent: {e}")))?;

        Ok(())
    }
}
