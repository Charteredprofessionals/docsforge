//! error.rs — Unified error contract for docforge-core.
//!
//! Provides the single `DocForgeError` enum that serializes to a stable JSON payload
//! `{ "code": "...", "message": "...", "detail": ... }` used across GUI, CLI, and REST interfaces.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error payload returned by Tauri commands, CLI, and REST services.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    /// Machine-readable snake_case error code.
    pub code: String,
    /// Human-readable error message describing the failure.
    pub message: String,
    /// Optional structured detail payload providing contextual diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// Core error enum representing all domain failure conditions in DocForge.
#[derive(Debug)]
pub enum DocForgeError {
    /// Provided file is not a valid DOCX or fails OPC structure validation.
    InvalidDocx(String),
    /// Zip archive exceeds size, ratio, or entry count safety thresholds.
    ZipBomb(String),
    /// Template contains unclosed `{{` tags or malformed placeholders.
    UnclosedTag { tag: String, position: Option<usize> },
    /// Referenced field tag does not exist in template definition.
    UnknownTag(String),
    /// Field value fails validation rules (type, pattern, required constraint).
    InvalidFieldValue { field_name: String, reason: String },
    /// Template or asset path does not exist on the filesystem.
    StorageMissing(String),
    /// Underlying filesystem I/O error occurred during operation.
    StorageIo(String),
    /// Operation rejected by RBAC permission rules.
    Forbidden(String),
    /// Template filling attempted on a draft/review version that is not published.
    NotPublished(String),
    /// Bundle mutation targeted a published Bundle Version, which is immutable (REQ-024).
    PublishedBundleImmutable(String),
    /// A provided input argument failed validation (empty name, malformed value).
    InvalidInput(String),
    /// License activation code or file signature is invalid.
    LicenseInvalid(String),
    /// License key or subscription period has expired.
    LicenseExpired(String),
    /// Operating limits (device count, seat limit, template cap) exceeded.
    LicenseLimitExceeded(String),
    /// Unexpected internal engine error.
    Internal(String),
}

impl DocForgeError {
    /// Returns the snake_case machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            DocForgeError::InvalidDocx(_) => "invalid_docx",
            DocForgeError::ZipBomb(_) => "zip_bomb",
            DocForgeError::UnclosedTag { .. } => "unclosed_tag",
            DocForgeError::UnknownTag(_) => "unknown_tag",
            DocForgeError::InvalidFieldValue { .. } => "invalid_field_value",
            DocForgeError::StorageMissing(_) => "storage_missing",
            DocForgeError::StorageIo(_) => "storage_io",
            DocForgeError::Forbidden(_) => "forbidden",
            DocForgeError::NotPublished(_) => "not_published",
            DocForgeError::PublishedBundleImmutable(_) => "published_bundle_immutable",
            DocForgeError::InvalidInput(_) => "invalid_input",
            DocForgeError::LicenseInvalid(_) => "license_invalid",
            DocForgeError::LicenseExpired(_) => "license_expired",
            DocForgeError::LicenseLimitExceeded(_) => "license_limit_exceeded",
            DocForgeError::Internal(_) => "internal",
        }
    }

    /// Converts the error into the stable `ErrorResponse` payload.
    pub fn to_response(&self) -> ErrorResponse {
        let code = self.code().to_string();
        let message = self.to_string();
        let detail = match self {
            DocForgeError::UnclosedTag { tag, position } => Some(serde_json::json!({
                "tag": tag,
                "position": position,
            })),
            DocForgeError::InvalidFieldValue { field_name, reason } => Some(serde_json::json!({
                "field_name": field_name,
                "reason": reason,
            })),
            _ => None,
        };

        ErrorResponse {
            code,
            message,
            detail,
        }
    }

    /// Serializes the error to a JSON string formatted for Tauri IPC / CLI output.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(&self.to_response())
            .unwrap_or_else(|_| format!(r#"{{"code":"internal","message":"{}"}}"#, self))
    }
}

impl fmt::Display for DocForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocForgeError::InvalidDocx(msg) => write!(f, "Invalid DOCX document: {msg}"),
            DocForgeError::ZipBomb(msg) => write!(f, "Security limit exceeded (ZipBomb): {msg}"),
            DocForgeError::UnclosedTag { tag, position } => match position {
                Some(pos) => write!(f, "Unclosed placeholder tag '{tag}' detected at character position {pos}"),
                None => write!(f, "Unclosed placeholder tag '{tag}' detected in document"),
            },
            DocForgeError::UnknownTag(tag) => write!(f, "Unknown field tag '{tag}' in template"),
            DocForgeError::InvalidFieldValue { field_name, reason } => {
                write!(f, "Invalid value for field '{field_name}': {reason}")
            }
            DocForgeError::StorageMissing(path) => write!(f, "Storage path missing: {path}"),
            DocForgeError::StorageIo(msg) => write!(f, "Storage I/O failure: {msg}"),
            DocForgeError::Forbidden(msg) => write!(f, "Access forbidden: {msg}"),
            DocForgeError::NotPublished(id) => {
                write!(f, "Template '{id}' is not in published state")
            }
            DocForgeError::PublishedBundleImmutable(id) => {
                write!(f, "Bundle version '{id}' is published and immutable (REQ-024); create a new version to change it")
            }
            DocForgeError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            DocForgeError::LicenseInvalid(msg) => write!(f, "Invalid license: {msg}"),
            DocForgeError::LicenseExpired(msg) => write!(f, "License expired: {msg}"),
            DocForgeError::LicenseLimitExceeded(msg) => write!(f, "License limit exceeded: {msg}"),
            DocForgeError::Internal(msg) => write!(f, "Internal engine error: {msg}"),
        }
    }
}

impl std::error::Error for DocForgeError {}

impl From<DocForgeError> for String {
    fn from(err: DocForgeError) -> Self {
        err.to_json_string()
    }
}

impl From<std::io::Error> for DocForgeError {
    fn from(err: std::io::Error) -> Self {
        DocForgeError::StorageIo(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unclosed_tag_serialization() {
        let err = DocForgeError::UnclosedTag {
            tag: "employee_name".to_string(),
            position: Some(142),
        };
        let response = err.to_response();
        assert_eq!(response.code, "unclosed_tag");
        assert!(response.message.contains("employee_name"));
        assert!(response.detail.is_some());
        let json_str = err.to_json_string();
        assert!(json_str.contains("unclosed_tag"));
        assert!(json_str.contains("employee_name"));
    }

    #[test]
    fn test_invalid_docx_code() {
        let err = DocForgeError::InvalidDocx("Header missing".to_string());
        assert_eq!(err.code(), "invalid_docx");
        assert_eq!(String::from(err).contains("invalid_docx"), true);
    }
}
