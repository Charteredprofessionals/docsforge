//! bundle — Data Model v3 Bundle domain (REQ-023, REQ-024, REQ-025).
//!
//! Owns the Bundle definition lifecycle end to end: identity + manifest
//! persistence (TASK-102, `manifest`), version lifecycle (TASK-103, `version`),
//! portable `.dfpkg` packaging (TASK-104, `dfpkg`), and output configuration
//! behaviors (TASK-105, `output_config`). Does not own matter data, value entry,
//! or document rendering (architecture §4.6).

pub mod dfpkg;
pub mod manifest;
pub mod output_config;
pub mod version;

pub use manifest::{
    BundleDetail, BundleDocumentSpec, BundleManifest, BundleRecord, BundleSchema, BundleSummary,
    BundleVersionRecord, OutputConfig, OutputFormat, create_bundle, delete_bundle, get_bundle,
    get_manifest, list_bundles, save_manifest,
};
