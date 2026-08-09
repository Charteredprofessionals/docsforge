//! infra/mod.rs — Infrastructure and OS abstraction layer for DocForge.
//!
//! Provides platform services including at-rest encryption and machine identification.

pub mod crypto;
pub mod print_bridge;

pub use crypto::{decrypt_at_rest, encrypt_at_rest, get_or_create_machine_id};
