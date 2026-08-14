//! schema.rs — Canonical field schema types and validation for DocForge (TASK-106, REQ-026/027).
//!
//! Defines the canonical `FieldDef` / `FieldGroup` domain types, the closed set of
//! 13 `FieldType` variants, and the validation routines that enforce REQ-026 (field
//! attributes) and REQ-027 (group scope). Persistence lives in `registry.rs`; this
//! module is pure domain logic with no database access.

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::core::error::DocForgeError;

/// The closed set of canonical field types for the v2.0.0 Bundle schema (REQ-026).
///
/// Serde uses `snake_case` so that `MultilineText` serializes to `multiline_text`,
/// which is exactly the `CHECK` value stored in the `fields.type` column. All
/// variants are unit variants, so the enum is `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// Single-line free text.
    Text,
    /// Multi-line free text.
    MultilineText,
    /// Numeric value (integer or floating point).
    Number,
    /// Monetary amount.
    Currency,
    /// Ratio/percentage value (0–100 domain, stored as a number).
    Percentage,
    /// Calendar date (ISO `YYYY-MM-DD`).
    Date,
    /// Calendar date and time (ISO 8601 / `YYYY-MM-DDTHH:MM:SS`).
    Datetime,
    /// Boolean true/false.
    Boolean,
    /// Electronic mail address.
    Email,
    /// Telephone number.
    Phone,
    /// Web address / URL.
    Url,
    /// Single choice from a fixed `options` list.
    Select,
    /// Multiple choices from a fixed `options` list.
    Multiselect,
}

impl FieldType {
    /// Returns the stable database / serialization string for this field type.
    pub fn as_db_str(self) -> &'static str {
        match self {
            FieldType::Text => "text",
            FieldType::MultilineText => "multiline_text",
            FieldType::Number => "number",
            FieldType::Currency => "currency",
            FieldType::Percentage => "percentage",
            FieldType::Date => "date",
            FieldType::Datetime => "datetime",
            FieldType::Boolean => "boolean",
            FieldType::Email => "email",
            FieldType::Phone => "phone",
            FieldType::Url => "url",
            FieldType::Select => "select",
            FieldType::Multiselect => "multiselect",
        }
    }
}

impl FromStr for FieldType {
    type Err = DocForgeError;

    /// Parses a `fields.type` string (or JSON `snake_case` form) into a `FieldType`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text" => Ok(FieldType::Text),
            "multiline_text" => Ok(FieldType::MultilineText),
            "number" => Ok(FieldType::Number),
            "currency" => Ok(FieldType::Currency),
            "percentage" => Ok(FieldType::Percentage),
            "date" => Ok(FieldType::Date),
            "datetime" => Ok(FieldType::Datetime),
            "boolean" => Ok(FieldType::Boolean),
            "email" => Ok(FieldType::Email),
            "phone" => Ok(FieldType::Phone),
            "url" => Ok(FieldType::Url),
            "select" => Ok(FieldType::Select),
            "multiselect" => Ok(FieldType::Multiselect),
            other => Err(DocForgeError::InvalidInput(format!(
                "unknown field type '{other}'"
            ))),
        }
    }
}

/// A canonical, Bundle-level field definition (REQ-026).
///
/// `id` is a generated UUID primary key; `field_id` is the stable logical id
/// (e.g. `"company.name"`) referenced by mappings, rules, and matter values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDef {
    /// Generated UUID primary key of the `fields` row.
    pub id: String,
    /// Stable logical id, e.g. `"company.name"`. Unique within a bundle version.
    pub field_id: String,
    /// Human-readable label shown in the Matter form.
    pub label: String,
    /// Optional human description / help text.
    pub description: Option<String>,
    /// The closed field type (REQ-026).
    pub field_type: FieldType,
    /// Whether the field must be supplied for a matter to validate.
    pub required: bool,
    /// Optional typed default value.
    pub default: Option<serde_json::Value>,
    /// Optional type-specific validation (e.g. `{ "min": 0, "max": 100, "pattern": "..." }`).
    pub validation: Option<serde_json::Value>,
    /// Optional fixed choices for `Select` / `Multiselect` fields.
    pub options: Vec<String>,
    /// Optional display format hint (e.g. a date format string).
    pub format: Option<String>,
    /// Optional owning group id (`field_groups.id`); `None` = ungrouped.
    pub group_id: Option<String>,
    /// Display / order position within the bundle version's field list.
    pub position: i64,
}

/// Scope of a field group, driving the Matter form's visual separation (REQ-027).
///
/// `Shared` groups fan a single value out across all documents in the Bundle;
/// `DocumentSpecific` groups hold values relevant to one document only. Both are
/// serialized `snake_case` to match the `field_groups.scope` `CHECK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupScope {
    /// Value reused across every document in the Bundle.
    Shared,
    /// Value relevant to a single document only.
    DocumentSpecific,
}

impl GroupScope {
    /// Returns the stable database string for this scope.
    pub fn as_db_str(self) -> &'static str {
        match self {
            GroupScope::Shared => "shared",
            GroupScope::DocumentSpecific => "document_specific",
        }
    }
}

impl FromStr for GroupScope {
    type Err = DocForgeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "shared" => Ok(GroupScope::Shared),
            "document_specific" => Ok(GroupScope::DocumentSpecific),
            other => Err(DocForgeError::InvalidInput(format!(
                "unknown group scope '{other}'"
            ))),
        }
    }
}

/// A named grouping of fields with a scope (REQ-027).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldGroup {
    /// Generated UUID primary key of the `field_groups` row.
    pub id: String,
    /// Owning bundle version; `None` = a global reusable group available to any version.
    pub bundle_version_id: Option<String>,
    /// Display name of the group.
    pub name: String,
    /// Optional human description of the group.
    pub description: Option<String>,
    /// Scope: `Shared` (fan-out) or `DocumentSpecific`.
    pub scope: GroupScope,
    /// Display / order position within the form.
    pub sort_order: i64,
}

/// Returns `true` when `s` is a valid stable logical field id:
/// `^[a-zA-Z][a-zA-Z0-9_.]*$` (REQ-026).
fn is_valid_field_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

/// Parses a `Date` value, accepting the ISO `YYYY-MM-DD` form.
fn parse_date(value: &str) -> Result<(), DocForgeError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| {
            DocForgeError::InvalidInput(format!(
                "value '{value}' is not a valid date (expected YYYY-MM-DD)"
            ))
        })
}

/// Parses a `Datetime` value, accepting RFC 3339, `YYYY-MM-DDTHH:MM:SS`, or a bare date.
fn parse_datetime(value: &str) -> Result<(), DocForgeError> {
    if DateTime::parse_from_rfc3339(value).is_ok() {
        return Ok(());
    }
    if NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").is_ok() {
        return Ok(());
    }
    if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() {
        return Ok(());
    }
    Err(DocForgeError::InvalidInput(format!(
        "value '{value}' is not a valid datetime (expected ISO 8601)"
    )))
}

/// Validates a canonical field definition against REQ-026 invariants.
///
/// Enforces: non-empty `field_id` matching the logical-id pattern; non-empty
/// `label`; `Select`/`Multiselect` require non-empty `options`; a present `default`
/// must type-check against the field type; numeric field types require numeric
/// `min`/`max` in `validation`. Returns a precise `InvalidInput` naming the field.
pub fn validate_field_schema(field: &FieldDef) -> Result<(), DocForgeError> {
    let fid = &field.field_id;
    if !is_valid_field_id(fid) {
        return Err(DocForgeError::InvalidInput(format!(
            "field '{fid}' has an invalid field_id: must match ^[a-zA-Z][a-zA-Z0-9_.]*$"
        )));
    }
    if field.label.trim().is_empty() {
        return Err(DocForgeError::InvalidInput(format!(
            "field '{fid}' label must not be empty"
        )));
    }
    if matches!(field.field_type, FieldType::Select | FieldType::Multiselect)
        && field.options.is_empty()
    {
        return Err(DocForgeError::InvalidInput(format!(
            "field '{fid}' of type {:?} requires a non-empty options list",
            field.field_type
        )));
    }
    if let Some(default) = &field.default {
        validate_value(field.field_type, default, false).map_err(|e| {
            DocForgeError::InvalidInput(format!("field '{fid}' default is invalid: {e}"))
        })?;
    }
    if matches!(
        field.field_type,
        FieldType::Number | FieldType::Currency | FieldType::Percentage
    ) {
        if let Some(validation) = &field.validation {
            if let Some(obj) = validation.as_object() {
                for key in ["min", "max"] {
                    if let Some(val) = obj.get(key) {
                        if !val.is_number() {
                            return Err(DocForgeError::InvalidInput(format!(
                                "field '{fid}' validation.{key} must be numeric"
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Type-checks a matter value against a field type and the `required` constraint.
///
/// Strings back `Text`/`MultilineText`/`Email`/`Phone`/`Url`/`Select`/`Date`/`Datetime`;
/// numbers back `Number`/`Currency`/`Percentage`; booleans back `Boolean`; an array of
/// strings backs `Multiselect`. A `null` value satisfies any non-required field and
/// fails a required one. `Select` membership in `options` is enforced at the
/// `FieldDef` level, not here (the value's `options` are not in scope).
pub fn validate_value(
    field_type: FieldType,
    value: &serde_json::Value,
    required: bool,
) -> Result<(), DocForgeError> {
    if value.is_null() {
        if required {
            return Err(DocForgeError::InvalidInput(
                "value is required but is null".to_string(),
            ));
        }
        return Ok(());
    }
    match field_type {
        FieldType::Text
        | FieldType::MultilineText
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url => {
            if !value.is_string() {
                return Err(DocForgeError::InvalidInput(format!(
                    "expected a string for field type '{field_type:?}'"
                )));
            }
        }
        FieldType::Number | FieldType::Currency | FieldType::Percentage => {
            if !value.is_number() {
                return Err(DocForgeError::InvalidInput(format!(
                    "expected a number for field type '{field_type:?}'"
                )));
            }
        }
        FieldType::Boolean => {
            if !value.is_boolean() {
                return Err(DocForgeError::InvalidInput(
                    "expected a boolean value".to_string(),
                ));
            }
        }
        FieldType::Date => {
            let s = value
                .as_str()
                .ok_or_else(|| DocForgeError::InvalidInput("expected a string date".to_string()))?;
            parse_date(s)?;
        }
        FieldType::Datetime => {
            let s = value.as_str().ok_or_else(|| {
                DocForgeError::InvalidInput("expected a string datetime".to_string())
            })?;
            parse_datetime(s)?;
        }
        FieldType::Select => {
            if !value.is_string() {
                return Err(DocForgeError::InvalidInput(
                    "expected a string value for a select field".to_string(),
                ));
            }
        }
        FieldType::Multiselect => {
            let arr = value.as_array().ok_or_else(|| {
                DocForgeError::InvalidInput(
                    "expected an array value for a multiselect field".to_string(),
                )
            })?;
            if !arr.iter().all(serde_json::Value::is_string) {
                return Err(DocForgeError::InvalidInput(
                    "multiselect must be an array of strings".to_string(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Builds a valid default value appropriate to each field type.
    fn default_for(field_type: FieldType) -> serde_json::Value {
        match field_type {
            FieldType::Boolean => json!(true),
            FieldType::Number | FieldType::Currency | FieldType::Percentage => json!(1),
            FieldType::Date => json!("2020-01-01"),
            FieldType::Datetime => json!("2020-01-01T00:00:00"),
            FieldType::Multiselect => json!(["a"]),
            _ => json!("sample"),
        }
    }

    /// Builds a minimal valid `FieldDef` for the given type.
    fn sample_field(field_type: FieldType) -> FieldDef {
        FieldDef {
            id: String::new(),
            field_id: "doc.sample".to_string(),
            label: "Sample".to_string(),
            description: None,
            field_type,
            required: true,
            default: Some(default_for(field_type)),
            validation: None,
            options: if matches!(field_type, FieldType::Select | FieldType::Multiselect) {
                vec!["a".to_string(), "b".to_string()]
            } else {
                Vec::new()
            },
            format: None,
            group_id: None,
            position: 0,
        }
    }

    #[test]
    fn test_all_field_types_validate() {
        let types = [
            FieldType::Text,
            FieldType::MultilineText,
            FieldType::Number,
            FieldType::Currency,
            FieldType::Percentage,
            FieldType::Date,
            FieldType::Datetime,
            FieldType::Boolean,
            FieldType::Email,
            FieldType::Phone,
            FieldType::Url,
            FieldType::Select,
            FieldType::Multiselect,
        ];
        for ty in types {
            let field = sample_field(ty);
            validate_field_schema(&field).expect("schema validation should pass");
            let value = field.default.clone().expect("default present");
            validate_value(ty, &value, true).expect("value validation should pass");
        }
    }

    #[test]
    fn test_validate_rejects_bad_field_id() {
        let mut field = sample_field(FieldType::Text);
        field.field_id = "has space".to_string();
        assert!(validate_field_schema(&field).is_err());
        field.field_id = "1starts_with_digit".to_string();
        assert!(validate_field_schema(&field).is_err());
    }

    #[test]
    fn test_validate_select_requires_options() {
        let mut field = sample_field(FieldType::Select);
        field.options = Vec::new();
        let err = validate_field_schema(&field).expect_err("select needs options");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }

    #[test]
    fn test_validate_value_type_mismatch() {
        assert!(validate_value(FieldType::Number, &json!("not a number"), true).is_err());
        assert!(validate_value(FieldType::Boolean, &json!("true"), true).is_err());
        assert!(validate_value(FieldType::Number, &json!(42), true).is_ok());
        assert!(validate_value(FieldType::Text, &json!("x"), false).is_ok());
        assert!(validate_value(FieldType::Text, &json!(null), true).is_err());
        assert!(validate_value(FieldType::Text, &json!(null), false).is_ok());
    }

    #[test]
    fn test_validate_date_parsing() {
        assert!(validate_value(FieldType::Date, &json!("2020-13-99"), true).is_err());
        assert!(validate_value(FieldType::Date, &json!("2020-01-01"), true).is_ok());
        assert!(
            validate_value(FieldType::Datetime, &json!("2020-01-01T00:00:00"), true).is_ok()
        );
    }
}
