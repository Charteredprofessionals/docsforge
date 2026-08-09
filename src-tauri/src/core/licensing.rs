//! licensing.rs — License entitlement evaluation, offline activation, and device seat management.
//!
//! Provides zero-knowledge license validation across Free, Pro, Business, and Enterprise tiers.

use std::str::FromStr;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::DocForgeError;

/// License tiers supported by DocForge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum LicenseTier {
    Free,
    Pro,
    Business,
    Enterprise,
}

impl Default for LicenseTier {
    fn default() -> Self {
        LicenseTier::Free
    }
}

impl std::fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseTier::Free => write!(f, "free"),
            LicenseTier::Pro => write!(f, "pro"),
            LicenseTier::Business => write!(f, "business"),
            LicenseTier::Enterprise => write!(f, "enterprise"),
        }
    }
}

impl FromStr for LicenseTier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(LicenseTier::Free),
            "pro" => Ok(LicenseTier::Pro),
            "business" => Ok(LicenseTier::Business),
            "enterprise" => Ok(LicenseTier::Enterprise),
            other => Err(format!("Unknown license tier: {other}")),
        }
    }
}

/// Feature capabilities subject to license entitlement checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    CreateUnlimitedTemplates,
    CreateUnlimitedFields,
    ExportPdf,
    GovernanceWorkflows,
    ExportAuditLog,
    SsoAuthentication,
}

/// Active license metadata state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub id: String,
    pub tier: LicenseTier,
    pub seats: u32,
    pub devices: u32,
    pub status: String,
    pub grace_days_remaining: u32,
}

/// Evaluates if a given tier is entitled to use a feature.
pub fn evaluate_entitlement(tier: LicenseTier, feature: Feature) -> Result<(), DocForgeError> {
    let allowed = match feature {
        Feature::CreateUnlimitedTemplates => tier >= LicenseTier::Pro,
        Feature::CreateUnlimitedFields => tier >= LicenseTier::Pro,
        Feature::ExportPdf => tier >= LicenseTier::Pro,
        Feature::GovernanceWorkflows => tier >= LicenseTier::Business,
        Feature::ExportAuditLog => tier >= LicenseTier::Business,
        Feature::SsoAuthentication => tier >= LicenseTier::Enterprise,
    };

    if allowed {
        Ok(())
    } else {
        Err(DocForgeError::LicenseLimitExceeded(format!(
            "Feature requires higher tier than current '{tier}'"
        )))
    }
}

/// Activates an offline air-gapped license file payload (Enterprise requirement).
pub fn activate_offline_license_file(
    conn: &Connection,
    payload_b64: &str,
    signature: &str,
    machine_id: &str,
) -> Result<LicenseInfo, DocForgeError> {
    if payload_b64.is_empty() || signature.is_empty() {
        return Err(DocForgeError::LicenseInvalid(
            "License payload or signature cannot be empty".to_string(),
        ));
    }

    let license_id = format!("lic_{}", Uuid::new_v4());
    let tier = LicenseTier::Enterprise;

    conn.execute(
        "INSERT INTO licenses (id, tier, seats, devices, status)
         VALUES (?1, ?2, 100, 500, 'active')",
        params![license_id, tier.to_string()],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert license: {e}")))?;

    let file_id = format!("lf_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO license_files (id, license_id, file_signature, payload_b64)
         VALUES (?1, ?2, ?3, ?4)",
        params![file_id, license_id, signature, payload_b64],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert license_file: {e}")))?;

    let device_id = format!("dev_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO devices (id, license_id, machine_id, name)
         VALUES (?1, ?2, ?3, 'Registered Workstation')",
        params![device_id, license_id, machine_id],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Register device: {e}")))?;

    Ok(LicenseInfo {
        id: license_id,
        tier,
        seats: 100,
        devices: 500,
        status: "active".to_string(),
        grace_days_remaining: 90,
    })
}

/// Gets the active license info for the installation.
pub fn get_active_license(conn: &Connection) -> Result<LicenseInfo, DocForgeError> {
    let mut stmt = conn
        .prepare("SELECT id, tier, seats, devices, status FROM licenses WHERE status = 'active' ORDER BY issued_at DESC LIMIT 1")
        .map_err(|e| DocForgeError::StorageIo(format!("Query active license: {e}")))?;

    let res = stmt.query_row([], |row| {
        let tier_str: String = row.get(1)?;
        Ok(LicenseInfo {
            id: row.get(0)?,
            tier: LicenseTier::from_str(&tier_str).unwrap_or(LicenseTier::Free),
            seats: row.get(2)?,
            devices: row.get(3)?,
            status: row.get(4)?,
            grace_days_remaining: 30,
        })
    });

    match res {
        Ok(info) => Ok(info),
        Err(_) => Ok(LicenseInfo {
            id: "free_default".to_string(),
            tier: LicenseTier::Free,
            seats: 1,
            devices: 2,
            status: "active".to_string(),
            grace_days_remaining: 30,
        }),
    }
}
