//! services/governance.rs — Governance service facade for audit exports and admin commands.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::core::error::DocForgeError;
use crate::core::governance::{authorize, Action, UserRole};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditExportRow {
    pub log_id: String,
    pub template_id: String,
    pub template_name: Option<String>,
    pub version: i32,
    pub output_name: String,
    pub format: String,
    pub status: String,
    pub user_id: Option<String>,
    pub machine_id: Option<String>,
    pub generated_at: String,
}

pub struct GovernanceService;

impl GovernanceService {
    pub fn export_audit_log(
        conn: &Connection,
        user_role: UserRole,
    ) -> Result<Vec<AuditExportRow>, DocForgeError> {
        authorize(user_role, Action::ExportAuditLog)?;

        let mut stmt = conn
            .prepare(
                "SELECT log_id, template_id, template_name, version, output_name, format,
                        status, user_id, machine_id, generated_at
                 FROM view_audit_export
                 ORDER BY generated_at DESC",
            )
            .map_err(|e| DocForgeError::StorageIo(format!("Prepare view_audit_export: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(AuditExportRow {
                    log_id: row.get(0)?,
                    template_id: row.get(1)?,
                    template_name: row.get(2)?,
                    version: row.get(3)?,
                    output_name: row.get(4)?,
                    format: row.get(5)?,
                    status: row.get(6)?,
                    user_id: row.get(7)?,
                    machine_id: row.get(8)?,
                    generated_at: row.get(9)?,
                })
            })
            .map_err(|e| DocForgeError::StorageIo(format!("Query audit log: {e}")))?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(item) = r {
                results.push(item);
            }
        }
        Ok(results)
    }
}
