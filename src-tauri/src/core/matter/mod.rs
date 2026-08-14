//! matter/mod.rs — Matter domain module (TASK-110).
//!
//! A Matter is an instance of exactly one Bundle Version, holding editable
//! field values (`matter_values`) that drive document generation.

pub mod form;
pub mod matter;
pub mod matter_values;
pub mod validation;

pub use form::{populate_matter_field, render_matter_form, FormField, FormGroup, MatterForm};
pub use matter::{create_matter, delete_matter, get_matter, list_matters, update_matter_status, Matter, MatterStatus};
pub use matter_values::{
    get_matter_value, list_matter_values, matter_to_json, set_matter_value, MatterValue,
};
pub use validation::{validate_matter, ValidationIssue, ValidationLevel, ValidationReport};
