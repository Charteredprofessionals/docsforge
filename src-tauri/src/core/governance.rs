//! governance.rs — RBAC enforcement, template approval workflows, and audit logging.
//!
//! Enforces security matrices across Viewer/Filler/Creator/Approver/Admin roles and
//! provides sole append-only writing access to `generation_log`.

use std::str::FromStr;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::DocForgeError;
use crate::core::template::TemplateStatus;

/// RBAC roles in DocForge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Viewer,
    Filler,
    Creator,
    Approver,
    Admin,
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Admin // Desktop app defaults to Admin for single-user mode
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Viewer => write!(f, "viewer"),
            UserRole::Filler => write!(f, "filler"),
            UserRole::Creator => write!(f, "creator"),
            UserRole::Approver => write!(f, "approver"),
            UserRole::Admin => write!(f, "admin"),
        }
    }
}

impl FromStr for UserRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "viewer" => Ok(UserRole::Viewer),
            "filler" => Ok(UserRole::Filler),
            "creator" => Ok(UserRole::Creator),
            "approver" => Ok(UserRole::Approver),
            "admin" => Ok(UserRole::Admin),
            other => Err(format!("Unknown user role: {other}")),
        }
    }
}

/// System actions governed by RBAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ViewTemplate,
    FillTemplate,
    CreateTemplate,
    DeleteTemplate,
    ApproveTemplate,
    ManageUsers,
    ExportAuditLog,
    BackupDatabase,
    RestoreDatabase,
    DeleteDatabase,
    CreateBundle,
    DeleteBundle,
    ManageBugs,
    ExportBugs,
}

/// Evaluates if a role is authorized to perform an action.
pub fn authorize(role: UserRole, action: Action) -> Result<(), DocForgeError> {
    let allowed = match action {
        // ViewTemplate: Anyone can view (Viewer, Filler, Creator, Approver, Admin)
        Action::ViewTemplate => true,
        // FillTemplate: Filler or above
        Action::FillTemplate => role >= UserRole::Filler,
        // CreateTemplate: Creator or above
        Action::CreateTemplate => role >= UserRole::Creator,
        // DeleteTemplate: Admin only
        Action::DeleteTemplate => role == UserRole::Admin,
        // ApproveTemplate: Approver or Admin
        Action::ApproveTemplate => role >= UserRole::Approver,
        // ManageUsers: Admin only
        Action::ManageUsers => role == UserRole::Admin,
        // ExportAuditLog: Approver or Admin
        Action::ExportAuditLog => role >= UserRole::Approver,
        // BackupDatabase: Admin only
        Action::BackupDatabase => role == UserRole::Admin,
        // RestoreDatabase: Admin only
        Action::RestoreDatabase => role == UserRole::Admin,
        // DeleteDatabase: Admin only
        Action::DeleteDatabase => role == UserRole::Admin,
        // CreateBundle: Creator or above
        Action::CreateBundle => role >= UserRole::Creator,
        // DeleteBundle: Admin only
        Action::DeleteBundle => role == UserRole::Admin,
        // ManageBugs: Approver or above (view/modify bugs)
        Action::ManageBugs => role >= UserRole::Approver,
        // ExportBugs: Approver or above
        Action::ExportBugs => role >= UserRole::Approver,
    };

    if allowed {
        Ok(())
    } else {
        Err(DocForgeError::Forbidden(format!(
            "Role '{role}' is not authorized to perform action '{action:?}'"
        )))
    }
}

/// Initialize the local user on first run (creates default Admin user for desktop).
pub fn initialize_local_user(conn: &Connection) -> Result<(), DocForgeError> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM users WHERE org_id IS NULL", [], |r| r.get(0))
        .unwrap_or(0);

    if count == 0 {
        let user_id = format!("usr_{}", Uuid::new_v4());
        conn.execute(
            "INSERT INTO users (id, name, email, role, active) VALUES (?1, ?2, ?3, ?4, 1)",
            params![user_id, "Local User", "user@localhost", "admin"],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Create default user: {e}")))?;
    }

    Ok(())
}

/// Get the current local user's role (for single-user desktop app).
pub fn get_current_user_role(conn: &Connection) -> Result<UserRole, DocForgeError> {
    // Ensure local user exists
    let _ = initialize_local_user(conn);

    let row: (String, String) = conn
        .query_row(
            "SELECT id, role FROM users WHERE org_id IS NULL AND active = 1 ORDER BY created_at ASC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Get current user: {e}")))?;

    let role = UserRole::from_str(&row.1).unwrap_or(UserRole::Admin);
    Ok(role)
}

/// Get current user info.
pub fn get_current_user(conn: &Connection) -> Result<(String, String, String, String), DocForgeError> {
    // Ensure local user exists
    let _ = initialize_local_user(conn);

    let row: (String, String, String, String) = conn
        .query_row(
            "SELECT id, role, name, email FROM users WHERE org_id IS NULL AND active = 1 ORDER BY created_at ASC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Get current user details: {e}")))?;

    Ok(row)
}

/// Set the current local user's role (admin-only operation).
pub fn set_current_user_role(conn: &Connection, new_role: UserRole) -> Result<(), DocForgeError> {
    let current_role = get_current_user_role(conn)?;
    authorize(current_role, Action::ManageUsers)?;

    let (user_id, _, _, _) = get_current_user(conn)?;

    conn.execute(
        "UPDATE users SET role = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![new_role.to_string(), user_id],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Update user role: {e}")))?;

    Ok(())
}

/// Check if the current user is authorized for an action (convenience wrapper).
pub fn require_authorization(conn: &Connection, action: Action) -> Result<(), DocForgeError> {
    let role = get_current_user_role(conn)?;
    authorize(role, action)
}

/// Transitions template lifecycle status (Draft -> Review -> Published -> Archived).
pub fn transition_template_status(
    conn: &Connection,
    template_id: &str,
    target_status: TemplateStatus,
) -> Result<(), DocForgeError> {
    // Approver or Admin required to publish
    if target_status == TemplateStatus::Published {
        authorize(get_current_user_role(conn)?, Action::ApproveTemplate)?;
    } else {
        authorize(get_current_user_role(conn)?, Action::CreateTemplate)?;
    }

    let status_str = target_status.to_string();

    let affected = conn
        .execute(
            "UPDATE templates SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status_str, template_id],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Update template status: {e}")))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Template '{template_id}' not found"
        )));
    }

    Ok(())
}

/// Appends a new generation audit log record to `generation_log` (sole append-only writer).
pub fn record_generation(
    conn: &Connection,
    template_id: &str,
    version: i32,
    output_name: &str,
    format: &str,
    user_id: Option<&str>,
    machine_id: Option<&str>,
) -> Result<String, DocForgeError> {
    let log_id = format!("gen_{}", Uuid::new_v4());

    conn.execute(
        "INSERT INTO generation_log (
            id, template_id, version, output_name, format, status, user_id, machine_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, 'success', ?6, ?7)",
        params![
            log_id,
            template_id,
            version,
            output_name,
            format,
            user_id,
            machine_id,
        ],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert generation log: {e}")))?;

    Ok(log_id)
}

/// Aggregated usage report summary. Zero document content retained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub total_generations: u64,
    pub docx_exports: u64,
    pub pdf_exports: u64,
    pub active_templates_count: u64,
}

/// Generates an aggregated usage report.
pub fn generate_usage_report(conn: &Connection) -> Result<UsageReport, DocForgeError> {
    let total_generations: u64 = conn
        .query_row("SELECT COUNT(*) FROM generation_log", [], |r| r.get(0))
        .unwrap_or(0);

    let docx_exports: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM generation_log WHERE format = 'docx'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let pdf_exports: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM generation_log WHERE format = 'pdf'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let active_templates_count: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM templates WHERE status != 'archived'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    Ok(UsageReport {
        total_generations,
        docx_exports,
        pdf_exports,
        active_templates_count,
    })
}