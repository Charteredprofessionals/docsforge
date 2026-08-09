//! docforge-onprem binary entry point.
//!
//! Air-gapped on-premises build of DocForge with telemetry hard-disabled at compile time.

fn main() {
    println!(r#"{{"name":"DocForge On-Premises Engine","mode":"air-gapped","telemetry":"disabled"}}"#);
}
