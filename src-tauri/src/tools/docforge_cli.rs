//! docforge CLI binary entry point.
//!
//! Reuses docforge-core for headless execution (CLI generate, fill, list, version).

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

use docforge::core::docx_engine::fill_document;
use docforge::core::governance::record_generation;
use docforge::core::template_store;
use docforge::schema::init_db;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            r#"{{"code":"invalid_args","message":"Usage: docforge-cli <command> [options]. Commands: version, list, fill --template-id ID [--out file.docx] [--set key=val ...]"}}"#
        );
        exit(2);
    }

    let command = args[1].as_str();
    match command {
        "version" => {
            println!(r#"{{"name":"DocForge CLI","version":"2.0.0","engine":"docforge-core"}}"#);
            exit(0);
        }
        "list" => {
            match handle_list() {
                Ok(json) => {
                    println!("{json}");
                    exit(0);
                }
                Err(e) => {
                    eprintln!(r#"{{"code":"storage_error","message":"{e}"}}"#);
                    exit(1);
                }
            }
        }
        "generate" | "fill" => {
            match handle_fill(&args[2..]) {
                Ok(json) => {
                    println!("{json}");
                    exit(0);
                }
                Err(e) => {
                    eprintln!(r#"{{"code":"fill_error","message":"{e}"}}"#);
                    exit(1);
                }
            }
        }
        _ => {
            eprintln!(r#"{{"code":"unknown_command","message":"Unknown command: {command}"}}"#);
            exit(2);
        }
    }
}

fn handle_list() -> Result<String, String> {
    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;
    let records = template_store::list_templates(&conn, None)
        .map_err(|e| format!("Failed to list templates: {e}"))?;

    let list_payload: Vec<serde_json::Value> = records
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "version": r.current_version,
                "status": r.status.to_string(),
                "field_count": r.fields.len(),
                "created_at": r.created_at,
                "updated_at": r.updated_at,
            })
        })
        .collect();

    serde_json::to_string(&list_payload).map_err(|e| format!("Serialization error: {e}"))
}

fn handle_fill(args: &[String]) -> Result<String, String> {
    let mut template_id: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut values: HashMap<String, String> = HashMap::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--template-id" | "-t" => {
                if i + 1 < args.len() {
                    template_id = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--out" | "-o" => {
                if i + 1 < args.len() {
                    out_path = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--set" | "-s" => {
                if i + 1 < args.len() {
                    let pair = &args[i + 1];
                    if let Some((k, v)) = pair.split_once('=') {
                        values.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let tpl_id = template_id.ok_or_else(|| "Missing required --template-id argument".to_string())?;

    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;
    let (record, bytes) = template_store::load_template_file(&conn, &tpl_id)
        .map_err(|e| format!("Failed to load template: {e}"))?;

    let filled_bytes = fill_document(&bytes, &values)
        .map_err(|e| format!("Document fill failure: {e}"))?;

    let output_filename = out_path.unwrap_or_else(|| format!("{}_filled.docx", record.name));
    let target = PathBuf::from(&output_filename);

    fs::write(&target, &filled_bytes)
        .map_err(|e| format!("Failed to write output file '{output_filename}': {e}"))?;

    let _ = record_generation(
        &conn,
        &tpl_id,
        record.current_version,
        &output_filename,
        "docx",
        Some("cli_user"),
        None,
    );

    Ok(serde_json::json!({
        "status": "success",
        "template_id": tpl_id,
        "output_file": target.to_string_lossy(),
        "bytes_written": filled_bytes.len(),
    })
    .to_string())
}
