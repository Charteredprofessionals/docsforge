//! fields.rs — Typed field schema and validation rules.
//!
//! Supports Text, Date, Dropdown, Checkbox, and Signature field types with Rust-enforced validation.

use serde::{Deserialize, Serialize};
use crate::core::error::DocForgeError;

/// Supported field types in DocForge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Date,
    Dropdown { options: Vec<String> },
    Checkbox,
    Signature,
}

impl Default for FieldType {
    fn default() -> Self {
        FieldType::Text
    }
}

/// Field validation specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldSpec {
    pub id: String,
    pub label: String,
    pub tag_name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default_value: Option<String>,
}

impl FieldSpec {
    /// Validates a supplied value against this field's type and constraints.
    pub fn validate_value(&self, value: &str) -> Result<(), DocForgeError> {
        if self.required && value.trim().is_empty() {
            return Err(DocForgeError::InvalidFieldValue {
                field_name: self.label.clone(),
                reason: "Field is required and cannot be empty".to_string(),
            });
        }

        if value.is_empty() {
            return Ok(());
        }

        match &self.field_type {
            FieldType::Text => Ok(()),
            FieldType::Date => {
                // ISO-8601 YYYY-MM-DD validation check
                if value.len() < 10 || !value.contains('-') {
                    return Err(DocForgeError::InvalidFieldValue {
                        field_name: self.label.clone(),
                        reason: "Must be a valid ISO date (YYYY-MM-DD)".to_string(),
                    });
                }
                Ok(())
            }
            FieldType::Dropdown { options } => {
                if !options.contains(&value.to_string()) {
                    return Err(DocForgeError::InvalidFieldValue {
                        field_name: self.label.clone(),
                        reason: format!("Selected value '{value}' is not among allowed options"),
                    });
                }
                Ok(())
            }
            FieldType::Checkbox => {
                let lower = value.to_lowercase();
                if lower != "true" && lower != "false" && lower != "yes" && lower != "no" {
                    return Err(DocForgeError::InvalidFieldValue {
                        field_name: self.label.clone(),
                        reason: "Checkbox value must be boolean (true/false/yes/no)".to_string(),
                    });
                }
                Ok(())
            }
            FieldType::Signature => Ok(()),
        }
    }
}
