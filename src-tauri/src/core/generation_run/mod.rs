//! generation_run/mod.rs — Generation run domain module (TASK-116).

pub mod execute;
pub mod record;

pub use execute::{execute_run, resolve_document_values, ExecuteResult, GeneratedDocument};
pub use record::{
    compute_input_hash, create_run, get_run, list_runs, GenerationRun, RunStatus, ENGINE_VERSION,
};
