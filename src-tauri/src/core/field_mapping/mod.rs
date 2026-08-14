//! field_mapping/mod.rs — Canonical field schema module root (TASK-106, REQ-026/027).
//!
//! Re-exports the public domain types (`FieldDef`, `FieldGroup`, `FieldType`,
//! `GroupScope`), validation (`validate_field_schema`, `validate_value`), and the
//! CRUD surface (`create_field`, `update_field`, `list_fields`, `remove_field`,
//! `create_field_group`, `list_field_groups`).

pub mod extraction;
pub mod groups;
pub mod mapping;
pub mod registry;
pub mod schema;

pub use registry::{
    create_field, create_field_group, list_field_groups, list_fields, remove_field, update_field,
};
pub use schema::{
    FieldDef, FieldGroup, FieldType, GroupScope, validate_field_schema, validate_value,
};
