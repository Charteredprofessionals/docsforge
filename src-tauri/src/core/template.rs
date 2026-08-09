//! template.rs — Template entity models and DTOs.

use serde::{Deserialize, Serialize};
use crate::core::docx_engine::TemplateFieldSpec;

/// High-level template lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateStatus {
    Draft,
    Review,
    Published,
    Archived,
}

impl Default for TemplateStatus {
    fn default() -> Self {
        TemplateStatus::Draft
    }
}

impl std::fmt::Display for TemplateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateStatus::Draft => write!(f, "draft"),
            TemplateStatus::Review => write!(f, "review"),
            TemplateStatus::Published => write!(f, "published"),
            TemplateStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for TemplateStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(TemplateStatus::Draft),
            "review" => Ok(TemplateStatus::Review),
            "published" => Ok(TemplateStatus::Published),
            "archived" => Ok(TemplateStatus::Archived),
            other => Err(format!("Unknown status: {other}")),
        }
    }
}

/// Metadata record for a template stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateRecord {
    pub id: String,
    pub org_id: Option<String>,
    pub name: String,
    pub category: String,
    pub description: String,
    pub current_version: i32,
    pub status: TemplateStatus,
    pub storage_path: String,
    pub fields: Vec<TemplateFieldSpec>,
    pub content_sha256: String,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
