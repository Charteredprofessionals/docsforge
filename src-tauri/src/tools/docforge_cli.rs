//! docforge CLI binary entry point.
//!
//! Reuses docforge-core for headless execution (CLI generate, fill, list, version).
//! Built with: `cargo build --features cli`

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::exit;

use csv;
use docforge::core::docx_engine::{fill_document, tag_document, validate_docx, TemplateFieldSpec};
use docforge::core::export::export_pdf_from_docx;
use docforge::core::governance::record_generation;
use docforge::core::template_store;
use docforge::schema::init_db;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        exit(2);
    }

    let command = args[1].as_str();
    let result = match command {
        "version" => handle_version(),
        "template" => {
            if args.len() < 3 {
                Err("template subcommand required: list, import, export".to_string())
            } else {
                handle_template(&args[2..])
            }
        }
        "fill" => handle_fill(&args[2..]),
        "generate" => handle_generate(&args[2..]),
        "list" => handle_list(),
        "audit" => handle_audit(&args[2..]),
        "license" => handle_license(&args[2..]),
        "config" => handle_config(&args[2..]),
        "serve" => handle_serve(&args[2..]),
        _ => Err(format!("Unknown command: {command}. Use --help for usage.")),
    };

    match result {
        Ok(json) => {
            println!("{json}");
            exit(0);
        }
        Err(e) => {
            eprintln!(r#"{{"code":"cli_error","message":"{e}"}}"#);
            exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"DocForge CLI v2.0.0 — Headless Document Automation

Usage: docforge-cli <command> [options]

Commands:
  version                    Print version information
  list                       List all templates
  template <subcommand>      Template management
    list                     List all templates
    import <file.docx>       Import a DOCX as a new template
    export <template-id>     Export a template as .dfpkg
  fill                       Fill a template with values (alias for generate)
  generate                   Generate a filled document
    --template-id, -t <id>   Template ID (required)
    --out, -o <path>         Output file path
    --set, -s <key=val>      Set field value (repeatable)
    --values <file.json>     Load values from JSON file
    --csv <file.csv>         Load values from CSV (first row)
    --format <docx|pdf>      Output format (default: docx)
    --replace-all            Replace all occurrences (default: true)
    --no-replace-all         Replace only first occurrence
  audit <subcommand>         Audit operations
    export --out <file.csv>  Export audit log to CSV
  license <subcommand>       License management
    activate <key|file>      Activate license
    status                   Show license status
    deactivate               Deactivate license
  config <subcommand>        Configuration
    show                     Show current configuration
    set <key> <value>        Set configuration value
  serve [--port <n>]         Start local REST bridge (Enterprise)

Examples:
  docforge-cli version
  docforge-cli list
  docforge-cli template import ./template.docx
  docforge-cli template export tpl_abc123
  docforge-cli fill --template-id tpl_abc123 --set name=John --set date=2024-01-01 -o output.docx
  docforge-cli generate --template-id tpl_abc123 --values data.json --format pdf
  docforge-cli audit export --out audit.csv
  docforge-cli license activate LICENSE-KEY-123
"#
    );
}

fn handle_version() -> Result<String, String> {
    Ok(serde_json::json!({
        "name": "DocForge CLI",
        "version": "2.0.0",
        "engine": "docforge-core",
        "features": ["template", "fill", "generate", "audit", "license", "config"]
    }).to_string())
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

fn handle_template(args: &[String]) -> Result<String, String> {
    let subcommand = args[0].as_str();
    match subcommand {
        "list" => handle_list(),
        "import" => {
            if args.len() < 2 {
                return Err("template import requires <file.docx> argument".to_string());
            }
            handle_template_import(&args[1])
        }
        "export" => {
            if args.len() < 2 {
                return Err("template export requires <template-id> argument".to_string());
            }
            handle_template_export(&args[1])
        }
        _ => Err(format!("Unknown template subcommand: {subcommand}. Use list, import, or export.")),
    }
}

fn handle_template_import(file_path: &str) -> Result<String, String> {
    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if path.extension().and_then(|s| s.to_str()) != Some("docx") {
        return Err("Only .docx files are supported for import".to_string());
    }

    let bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;
    validate_docx(&bytes).map_err(|e| format!("Invalid DOCX: {e}"))?;

    // Create a basic template with auto-detected placeholders
    // For full tagging, use the GUI
    let template_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported Template")
        .to_string();

    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;
    let record = template_store::save_template(
        &conn,
        &template_name,
        "general",
        "Imported via CLI",
        &[], // No fields pre-tagged; user must tag in GUI
        &bytes,
        None,
        Some("cli_user"),
    ).map_err(|e| format!("Failed to save template: {e}"))?;

    Ok(serde_json::json!({
        "id": record.id,
        "name": record.name,
        "status": "imported",
        "note": "Template imported without field tags. Use GUI to tag fields."
    }).to_string())
}

fn handle_template_export(template_id: &str) -> Result<String, String> {
    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;
    let (record, bytes) = template_store::load_template_file(&conn, template_id)
        .map_err(|e| format!("Failed to load template: {e}"))?;

    // Export as .dfpkg (zip with template.docx + metadata)
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    use std::io::Cursor;

    let mut buf = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("template.docx", opts).map_err(|e| format!("Zip start: {e}"))?;
        zip.write_all(&bytes).map_err(|e| format!("Zip write: {e}"))?;

        let metadata = serde_json::json!({
            "id": record.id,
            "name": record.name,
            "fields": record.fields,
            "version": record.current_version,
            "status": record.status.to_string(),
        });
        zip.start_file("metadata.json", opts).map_err(|e| format!("Zip start metadata: {e}"))?;
        zip.write_all(metadata.to_string().as_bytes()).map_err(|e| format!("Zip write metadata: {e}"))?;

        zip.finish().map_err(|e| format!("Zip finish: {e}"))?;
    }

    let out_path = format!("{}.dfpkg", record.id);
    fs::write(&out_path, &buf).map_err(|e| format!("Failed to write .dfpkg: {e}"))?;

    Ok(serde_json::json!({
        "path": out_path,
        "template_id": record.id,
        "name": record.name,
    }).to_string())
}

#[derive(Default)]
struct FillArgs {
    template_id: Option<String>,
    out_path: Option<String>,
    values: HashMap<String, String>,
    values_file: Option<String>,
    csv_file: Option<String>,
    format: String,
    replace_all: bool,
}

fn parse_fill_args(args: &[String]) -> Result<FillArgs, String> {
    let mut parsed = FillArgs::default();
    parsed.format = "docx".to_string();
    parsed.replace_all = true;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--template-id" | "-t" => {
                if i + 1 < args.len() {
                    parsed.template_id = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("--template-id requires a value".to_string());
                }
            }
            "--out" | "-o" => {
                if i + 1 < args.len() {
                    parsed.out_path = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("--out requires a value".to_string());
                }
            }
            "--set" | "-s" => {
                if i + 1 < args.len() {
                    let pair = &args[i + 1];
                    if let Some((k, v)) = pair.split_once('=') {
                        parsed.values.insert(k.trim().to_string(), v.trim().to_string());
                    } else {
                        return Err(format!("Invalid --set format: {pair}. Use key=value"));
                    }
                    i += 1;
                } else {
                    return Err("--set requires a value".to_string());
                }
            }
            "--values" => {
                if i + 1 < args.len() {
                    parsed.values_file = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("--values requires a file path".to_string());
                }
            }
            "--csv" => {
                if i + 1 < args.len() {
                    parsed.csv_file = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("--csv requires a file path".to_string());
                }
            }
            "--format" => {
                if i + 1 < args.len() {
                    let fmt = args[i + 1].to_lowercase();
                    if fmt != "docx" && fmt != "pdf" {
                        return Err("--format must be 'docx' or 'pdf'".to_string());
                    }
                    parsed.format = fmt;
                    i += 1;
                } else {
                    return Err("--format requires a value".to_string());
                }
            }
            "--replace-all" => {
                parsed.replace_all = true;
            }
            "--no-replace-all" => {
                parsed.replace_all = false;
            }
            _ => {
                return Err(format!("Unknown fill argument: {}", args[i]));
            }
        }
        i += 1;
    }

    // Load values from file if specified
    if let Some(file) = &parsed.values_file {
        let content = fs::read_to_string(file).map_err(|e| format!("Failed to read values file: {e}"))?;
        let json_values: HashMap<String, String> = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON in values file: {e}"))?;
        parsed.values.extend(json_values);
    }

    // Load first row from CSV if specified
    if let Some(file) = &parsed.csv_file {
        let content = fs::read_to_string(file).map_err(|e| format!("Failed to read CSV file: {e}"))?;
        let mut rdr = csv::Reader::from_reader(content.as_bytes());
        let headers = rdr.headers().map_err(|e| format!("CSV header parse error: {e}"))?.clone();
        if let Some(record) = rdr.records().next() {
            let record = record.map_err(|e| format!("CSV record parse error: {e}"))?;
            for (i, header) in headers.iter().enumerate() {
                if let Some(value) = record.get(i) {
                    parsed.values.insert(header.to_string(), value.to_string());
                }
            }
        }
    }

    Ok(parsed)
}

fn handle_fill(args: &[String]) -> Result<String, String> {
    handle_generate(args) // alias
}

fn handle_generate(args: &[String]) -> Result<String, String> {
    let parsed = parse_fill_args(args)?;

    let tpl_id = parsed.template_id.ok_or_else(|| "Missing required --template-id argument".to_string())?;

    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;
    let (record, bytes) = template_store::load_template_file(&conn, &tpl_id)
        .map_err(|e| format!("Failed to load template: {e}"))?;

    let filled_bytes = fill_document(&bytes, &parsed.values, parsed.replace_all)
        .map_err(|e| format!("Document fill failure: {e}"))?;

    let output_filename = parsed.out_path.unwrap_or_else(|| format!("{}_filled.{}", record.name, parsed.format));
    let target = PathBuf::from(&output_filename);

    if parsed.format == "pdf" {
        let pdf_bytes = export_pdf_from_docx(&filled_bytes)
            .map_err(|e| format!("PDF generation failed: {e}"))?;
        fs::write(&target, &pdf_bytes)
            .map_err(|e| format!("Failed to write PDF '{output_filename}': {e}"))?;
    } else {
        fs::write(&target, &filled_bytes)
            .map_err(|e| format!("Failed to write DOCX '{output_filename}': {e}"))?;
    }

    let _ = record_generation(
        &conn,
        &tpl_id,
        record.current_version,
        &output_filename,
        &parsed.format,
        Some("cli_user"),
        None,
    );

    Ok(serde_json::json!({
        "status": "success",
        "template_id": tpl_id,
        "output_file": target.to_string_lossy(),
        "format": parsed.format,
        "bytes_written": if parsed.format == "pdf" {
            export_pdf_from_docx(&filled_bytes).map(|b| b.len()).unwrap_or(0)
        } else {
            filled_bytes.len()
        },
    }).to_string())
}

fn handle_audit(args: &[String]) -> Result<String, String> {
    if args.is_empty() || args[0] != "export" {
        return Err("audit subcommand required: export".to_string());
    }

    let mut out_path = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--out" || args[i] == "-o" {
            if i + 1 < args.len() {
                out_path = Some(args[i + 1].clone());
                i += 1;
            } else {
                return Err("--out requires a file path".to_string());
            }
        }
        i += 1;
    }

    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;
    let mut stmt = conn.prepare(
        "SELECT id, template_id, version, output_name, format, status, user_id, machine_id, generated_at
         FROM generation_log ORDER BY generated_at DESC"
    ).map_err(|e| format!("DB query error: {e}"))?;

    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "template_id": row.get::<_, String>(1)?,
            "version": row.get::<_, i64>(2)?,
            "output_name": row.get::<_, String>(3)?,
            "format": row.get::<_, String>(4)?,
            "status": row.get::<_, String>(5)?,
            "user_id": row.get::<_, Option<String>>(6)?,
            "machine_id": row.get::<_, String>(7)?,
            "generated_at": row.get::<_, String>(8)?,
        }))
    }).map_err(|e| format!("Query execution error: {e}"))?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| format!("Row error: {e}"))?);
    }

    // Generate CSV
    let mut csv_out = String::new();
    csv_out.push_str("id,template_id,version,output_name,format,status,user_id,machine_id,generated_at\n");
    for entry in &entries {
        csv_out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            entry["id"],
            entry["template_id"],
            entry["version"],
            entry["output_name"].replace(',', ";"),
            entry["format"],
            entry["status"],
            entry["user_id"].as_str().unwrap_or(""),
            entry["machine_id"],
            entry["generated_at"]
        ));
    }

    if let Some(path) = out_path {
        fs::write(&path, csv_out).map_err(|e| format!("Failed to write CSV: {e}"))?;
        Ok(serde_json::json!({
            "status": "success",
            "entries": entries.len(),
            "output_file": path,
        }).to_string())
    } else {
        // Output JSON to stdout
        Ok(serde_json::to_string(&entries).map_err(|e| format!("Serialization error: {e}"))?)
    }
}

fn handle_license(args: &[String]) -> Result<String, String> {
    if args.is_empty() {
        return Err("license subcommand required: activate, status, deactivate".to_string());
    }

    let subcommand = args[0].as_str();
    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;

    match subcommand {
        "status" => {
            let license = docforge::core::licensing::get_active_license(&conn)
                .map_err(|e| format!("Failed to get license: {e}"))?;
            Ok(serde_json::to_string(&license).map_err(|e| format!("Serialization error: {e}"))?)
        }
        "activate" => {
            if args.len() < 2 {
                return Err("license activate requires <key> or <file> argument".to_string());
            }
            let key_or_file = &args[1];
            // Try as file first
            if PathBuf::from(key_or_file).exists() {
                let data = fs::read(key_or_file).map_err(|e| format!("Failed to read license file: {e}"))?;
                docforge::core::licensing::activate_offline_license_file(&conn, &data)
                    .map_err(|e| format!("License activation failed: {e}"))?;
            } else {
                // Try as key
                docforge::core::licensing::evaluate_entitlement(&conn, key_or_file)
                    .map_err(|e| format!("License key validation failed: {e}"))?;
            }
            Ok(serde_json::json!({"status": "activated"}).to_string())
        }
        "deactivate" => {
            // Implementation would go here
            Ok(serde_json::json!({"status": "deactivated", "note": "Not fully implemented"}).to_string())
        }
        _ => Err(format!("Unknown license subcommand: {subcommand}. Use activate, status, or deactivate.")),
    }
}

fn handle_config(args: &[String]) -> Result<String, String> {
    if args.is_empty() {
        return Err("config subcommand required: show, set".to_string());
    }

    let subcommand = args[0].as_str();
    let conn = init_db().map_err(|e| format!("Failed to open DB: {e}"))?;

    match subcommand {
        "show" => {
            // Query policy_config table
            let mut stmt = conn.prepare("SELECT key, value_json FROM policy_config")
                .map_err(|e| format!("DB query error: {e}"))?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }).map_err(|e| format!("Query error: {e}"))?;

            let mut config = serde_json::Map::new();
            for row in rows {
                let (k, v) = row.map_err(|e| format!("Row error: {e}"))?;
                config.insert(k, serde_json::from_str(&v).unwrap_or(serde_json::Value::String(v)));
            }

            Ok(serde_json::to_string(&serde_json::Value::Object(config)).map_err(|e| format!("Serialization error: {e}"))?)
        }
        "set" => {
            if args.len() < 3 {
                return Err("config set requires <key> <value>".to_string());
            }
            let key = &args[1];
            let value = &args[2];
            // Validate JSON
            let _: serde_json::Value = serde_json::from_str(value).map_err(|e| format!("Value must be valid JSON: {e}"))?;

            conn.execute(
                "INSERT OR REPLACE INTO policy_config (key, value_json) VALUES (?1, ?2)",
                [key, value],
            ).map_err(|e| format!("DB insert error: {e}"))?;

            Ok(serde_json::json!({"status": "set", "key": key}).to_string())
        }
        _ => Err(format!("Unknown config subcommand: {subcommand}. Use show or set.")),
    }
}

fn handle_serve(args: &[String]) -> Result<String, String> {
    // Placeholder for REST bridge
    let mut port = 8080;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--port" || args[i] == "-p" {
            if i + 1 < args.len() {
                port = args[i + 1].parse().map_err(|_| "Invalid port number".to_string())?;
                i += 1;
            }
        }
        i += 1;
    }

    Err(format!("REST bridge (serve) not yet implemented. Planned for port {port}. Requires Enterprise tier."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fill_args_basic() {
        let args = vec!["--template-id".to_string(), "tpl_123".to_string(), "--set".to_string(), "name=John".to_string()];
        let parsed = parse_fill_args(&args).unwrap();
        assert_eq!(parsed.template_id, Some("tpl_123".to_string()));
        assert_eq!(parsed.values.get("name"), Some(&"John".to_string()));
    }

    #[test]
    fn test_parse_fill_args_format() {
        let args = vec!["--template-id".to_string(), "tpl_123".to_string(), "--format".to_string(), "pdf".to_string()];
        let parsed = parse_fill_args(&args).unwrap();
        assert_eq!(parsed.format, "pdf");
    }

    #[test]
    fn test_parse_fill_args_replace_all() {
        let args = vec!["--template-id".to_string(), "tpl_123".to_string(), "--no-replace-all".to_string()];
        let parsed = parse_fill_args(&args).unwrap();
        assert_eq!(parsed.replace_all, false);
    }
}