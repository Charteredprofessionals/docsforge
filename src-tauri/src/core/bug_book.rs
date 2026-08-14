//! core/bug_book.rs — Persistent bug/error log for the Admin Console "Bug Book".
//!
//! Automatically and manually records application crashes and runtime errors with
//! timestamp, error type, severity, status, affected-user/endpoint context, full
//! stack trace, and supplementary attachments. Supports filtered listing/sorting/
//! search and CSV/PDF export for reporting.

use rusqlite::{params, params_from_iter, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::DocForgeError;
use crate::core::export::pdf::export_pdf;

/// Severity levels, ordered most→least severe (used for sorting and validation).
pub const SEVERITIES: &[&str] = &["critical", "high", "medium", "low"];
/// Lifecycle statuses for a bug entry.
pub const STATUSES: &[&str] = &["open", "in_progress", "resolved", "wont_fix"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BugAttachment {
    pub id: String,
    pub bug_id: String,
    pub filename: String,
    pub mime_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BugEntry {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub error_type: String,
    pub severity: String,
    pub status: String,
    pub context: String,
    pub message: String,
    pub stack_trace: String,
    pub source: String,
    pub category: String,
    pub keywords: String,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<String>,
    pub attachments: Vec<BugAttachment>,
}

/// Input for creating a bug entry (auto or manual).
pub struct NewBug {
    pub error_type: String,
    pub severity: String,
    pub status: String,
    pub context: String,
    pub message: String,
    pub stack_trace: String,
    pub source: String,
    pub category: String,
    pub keywords: String,
}

/// Filter/sort criteria for listing and exporting bug entries.
#[derive(Debug, Clone, Default)]
pub struct BugFilter {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub keyword: Option<String>,
    pub sort_by: String,
    pub sort_dir: String,
    pub limit: Option<u32>,
}

pub fn validate_severity(severity: &str) -> Result<(), DocForgeError> {
    if SEVERITIES.contains(&severity) {
        Ok(())
    } else {
        Err(DocForgeError::Internal(format!(
            "Invalid severity '{severity}'. Must be one of: {}",
            SEVERITIES.join(", ")
        )))
    }
}

pub fn validate_status(status: &str) -> Result<(), DocForgeError> {
    if STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(DocForgeError::Internal(format!(
            "Invalid status '{status}'. Must be one of: {}",
            STATUSES.join(", ")
        )))
    }
}

fn row_to_entry(
    r: &rusqlite::Row<'_>,
) -> SqlResult<(
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
)> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
        r.get(13)?,
    ))
}

fn load_attachments(conn: &Connection, bug_id: &str) -> Result<Vec<BugAttachment>, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, bug_id, filename, mime_type, created_at FROM bug_attachments
             WHERE bug_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare attachments: {e}")))?;
    let rows = stmt
        .query_map(params![bug_id], |r| {
            Ok(BugAttachment {
                id: r.get(0)?,
                bug_id: r.get(1)?,
                filename: r.get(2)?,
                mime_type: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| DocForgeError::StorageIo(format!("Query attachments: {e}")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map attachment: {e}")))?);
    }
    Ok(out)
}

fn assemble_entry(
    conn: &Connection,
    base: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ),
) -> Result<BugEntry, DocForgeError> {
    let attachments = load_attachments(conn, &base.0)?;
    Ok(BugEntry {
        id: base.0,
        created_at: base.1,
        updated_at: base.2,
        error_type: base.3,
        severity: base.4,
        status: base.5,
        context: base.6,
        message: base.7,
        stack_trace: base.8,
        source: base.9,
        category: base.10,
        keywords: base.11,
        resolved_by: base.12,
        resolved_at: base.13,
        attachments,
    })
}

/// Inserts a new bug entry and returns the fully hydrated record (with attachments).
pub fn create_bug(conn: &Connection, new: &NewBug) -> Result<BugEntry, DocForgeError> {
    validate_severity(&new.severity)?;
    validate_status(&new.status)?;

    let id = format!("bug_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO bug_book
            (id, error_type, severity, status, context, message, stack_trace, source, category, keywords)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            new.error_type,
            new.severity,
            new.status,
            new.context,
            new.message,
            new.stack_trace,
            new.source,
            new.category,
            new.keywords
        ],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert bug: {e}")))?;

    get_bug(conn, &id)
}

/// Records a critical crash (used by the global panic hook). Best-effort.
pub fn record_crash(conn: &Connection, payload: &str, location: &str) -> Result<BugEntry, DocForgeError> {
    let new = NewBug {
        error_type: "panic".to_string(),
        severity: "critical".to_string(),
        status: "open".to_string(),
        context: location.to_string(),
        message: payload.chars().take(2000).collect(),
        stack_trace: String::new(),
        source: "auto".to_string(),
        category: "crash".to_string(),
        keywords: "panic,crash".to_string(),
    };
    create_bug(conn, &new)
}

/// Fetches a single bug entry by id, including its attachments.
pub fn get_bug(conn: &Connection, id: &str) -> Result<BugEntry, DocForgeError> {
    let base: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT id, created_at, updated_at, error_type, severity, status, context, message,
                    stack_trace, source, category, keywords, resolved_by, resolved_at
             FROM bug_book WHERE id = ?1",
            params![id],
            row_to_entry,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DocForgeError::StorageMissing(format!("Bug '{id}' not found"))
            }
            _ => DocForgeError::StorageIo(format!("Get bug: {e}")),
        })?;
    assemble_entry(conn, base)
}

/// Lists bug entries matching the given filter/sort criteria.
pub fn list_bugs(conn: &Connection, filter: &BugFilter) -> Result<Vec<BugEntry>, DocForgeError> {
    let mut sql = String::from(
        "SELECT id, created_at, updated_at, error_type, severity, status, context, message,
                stack_trace, source, category, keywords, resolved_by, resolved_at
         FROM bug_book WHERE 1=1",
    );
    let mut args: Vec<String> = Vec::new();

    if let Some(d) = &filter.date_from {
        sql.push_str(" AND date(created_at) >= date(?)");
        args.push(d.clone());
    }
    if let Some(d) = &filter.date_to {
        sql.push_str(" AND date(created_at) <= date(?)");
        args.push(d.clone());
    }
    if let Some(s) = &filter.severity {
        if !s.is_empty() {
            sql.push_str(" AND severity = ?");
            args.push(s.clone());
        }
    }
    if let Some(s) = &filter.status {
        if !s.is_empty() {
            sql.push_str(" AND status = ?");
            args.push(s.clone());
        }
    }
    if let Some(kw) = &filter.keyword {
        if !kw.trim().is_empty() {
            sql.push_str(" AND (message LIKE ? OR error_type LIKE ? OR context LIKE ? OR keywords LIKE ?)");
            let like = format!("%{}%", kw.trim());
            args.push(like.clone());
            args.push(like.clone());
            args.push(like.clone());
            args.push(like);
        }
    }

    // Sorting: severity uses a fixed rank order; otherwise order by the chosen column.
    let sort_col = match filter.sort_by.as_str() {
        "severity" => "severity",
        "status" => "status",
        "error_type" => "error_type",
        _ => "created_at",
    };
    let dir = if filter.sort_dir == "asc" { "ASC" } else { "DESC" };
    if sort_col == "severity" {
        sql.push_str(&format!(
            " ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 \
             WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END {dir}"
        ));
    } else {
        sql.push_str(&format!(" ORDER BY {sort_col} {dir}"));
    }

    if let Some(limit) = filter.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list bugs: {e}")))?;
    let rows = stmt
        .query_map(params_from_iter(args), row_to_entry)
        .map_err(|e| DocForgeError::StorageIo(format!("Query list bugs: {e}")))?;

    let mut bases = Vec::new();
    for row in rows {
        bases.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map bug row: {e}")))?);
    }

    bases.into_iter().map(|b| assemble_entry(conn, b)).collect()
}

/// Transitions a bug entry to a new status, recording resolver + timestamp on resolve.
pub fn update_bug_status(
    conn: &Connection,
    id: &str,
    status: &str,
    resolved_by: Option<String>,
) -> Result<(), DocForgeError> {
    validate_status(status)?;

    let (resolved_at, resolved_by_val): (Option<String>, Option<String>) = if status == "resolved" {
        (Some("now".to_string()), resolved_by)
    } else {
        (None, None)
    };

    conn.execute(
        "UPDATE bug_book
         SET status = ?1,
             resolved_by = ?2,
             resolved_at = CASE WHEN ?3 IS NOT NULL THEN datetime('now') ELSE NULL END,
             updated_at = datetime('now')
         WHERE id = ?4",
        params![status, resolved_by_val, resolved_at, id],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Update bug status: {e}")))?;

    if conn.changes() == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Bug '{id}' not found"
        )));
    }
    Ok(())
}

/// Attaches a supplementary log/screenshot to a bug entry. `data_b64` is the
/// base64-encoded file contents.
pub fn add_attachment(
    conn: &Connection,
    bug_id: &str,
    filename: &str,
    mime_type: &str,
    data_b64: &str,
) -> Result<BugAttachment, DocForgeError> {
    // Ensure the parent bug exists.
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM bug_book WHERE id = ?1",
            params![bug_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !exists {
        return Err(DocForgeError::StorageMissing(format!(
            "Bug '{bug_id}' not found"
        )));
    }

    let id = format!("att_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO bug_attachments (id, bug_id, filename, mime_type, data_b64)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, bug_id, filename, mime_type, data_b64],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert attachment: {e}")))?;

    let att = conn
        .query_row(
            "SELECT id, bug_id, filename, mime_type, created_at FROM bug_attachments WHERE id = ?1",
            params![id],
            |r| {
                Ok(BugAttachment {
                    id: r.get(0)?,
                    bug_id: r.get(1)?,
                    filename: r.get(2)?,
                    mime_type: r.get(3)?,
                    created_at: r.get(4)?,
                })
            },
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Read attachment: {e}")))?;
    Ok(att)
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Renders the filtered bug list to CSV (header row + one row per entry).
pub fn export_bugs_csv(conn: &Connection, filter: &BugFilter) -> Result<String, DocForgeError> {
    let bugs = list_bugs(conn, filter)?;
    let mut out = String::new();
    out.push_str(
        "id,created_at,severity,status,error_type,context,message,source,category,keywords,resolved_by\n",
    );
    for b in bugs {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&b.id),
            csv_escape(&b.created_at),
            csv_escape(&b.severity),
            csv_escape(&b.status),
            csv_escape(&b.error_type),
            csv_escape(&b.context),
            csv_escape(&b.message),
            csv_escape(&b.source),
            csv_escape(&b.category),
            csv_escape(&b.keywords),
            csv_escape(b.resolved_by.as_deref().unwrap_or("")),
        ));
    }
    Ok(out)
}

/// Renders the filtered bug list to a multi-page PDF report (native, no external deps).
pub fn export_bugs_pdf(conn: &Connection, filter: &BugFilter) -> Result<Vec<u8>, DocForgeError> {
    let bugs = list_bugs(conn, filter)?;
    let mut lines: Vec<String> = vec![
        format!("DocForge Bug Book Report — {} {}", bugs.len(), "entrie(s)"),
        format!("Generated: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")),
    ];
    for b in bugs {
        lines.push(format!(
            "[{}] {} / {}",
            b.severity.to_uppercase(),
            b.status,
            b.error_type
        ));
        lines.push(format!("  ID: {}", b.id));
        lines.push(format!("  Created: {}", b.created_at));
        lines.push(format!("  Context: {}", b.context));
        lines.push(format!("  Message: {}", b.message));
        if !b.stack_trace.is_empty() {
            lines.push("  Stack trace:".to_string());
            for sl in b.stack_trace.lines().take(25) {
                lines.push(format!("    {}", sl));
            }
        }
        lines.push(format!(
            "  Source: {}  Category: {}  Keywords: {}",
            b.source, b.category, b.keywords
        ));
        lines.push("".to_string());
    }
    if lines.len() <= 2 {
        lines.push("(no matching bug entries)".to_string());
    }
    export_pdf(&lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_memory_db;

    fn sample_bug(severity: &str, status: &str, keyword: &str) -> NewBug {
        NewBug {
            error_type: "runtime_error".to_string(),
            severity: severity.to_string(),
            status: status.to_string(),
            context: "user:admin@endpoint:/admin".to_string(),
            message: format!("Something broke ({keyword})"),
            stack_trace: "Error: boom\n  at foo (app.ts:10:2)".to_string(),
            source: "auto".to_string(),
            category: "ui".to_string(),
            keywords: keyword.to_string(),
        }
    }

    #[test]
    fn test_create_and_get_bug() {
        let conn = init_memory_db().expect("init");
        let created = create_bug(&conn, &sample_bug("high", "open", "login"))
            .expect("create");
        assert_eq!(created.severity, "high");
        assert_eq!(created.status, "open");
        assert!(created.id.starts_with("bug_"));

        let fetched = get_bug(&conn, &created.id).expect("get");
        assert_eq!(fetched.message, created.message);
        assert!(fetched.attachments.is_empty());
    }

    #[test]
    fn test_invalid_severity_rejected() {
        let conn = init_memory_db().expect("init");
        let mut b = sample_bug("urgent", "open", "x");
        b.severity = "urgent".to_string();
        assert!(create_bug(&conn, &b).is_err());
    }

    #[test]
    fn test_filter_by_severity_and_keyword() {
        let conn = init_memory_db().expect("init");
        create_bug(&conn, &sample_bug("critical", "open", "payment")).unwrap();
        create_bug(&conn, &sample_bug("low", "open", "cosmetic")).unwrap();
        create_bug(&conn, &sample_bug("high", "resolved", "payment")).unwrap();

        let only_critical = list_bugs(
            &conn,
            &BugFilter {
                severity: Some("critical".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(only_critical.len(), 1);
        assert_eq!(only_critical[0].severity, "critical");

        let payment_kw = list_bugs(
            &conn,
            &BugFilter {
                keyword: Some("payment".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(payment_kw.len(), 2);

        let resolved = list_bugs(
            &conn,
            &BugFilter {
                status: Some("resolved".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn test_severity_sort_order() {
        let conn = init_memory_db().expect("init");
        create_bug(&conn, &sample_bug("low", "open", "a")).unwrap();
        create_bug(&conn, &sample_bug("critical", "open", "b")).unwrap();
        create_bug(&conn, &sample_bug("high", "open", "c")).unwrap();

        let sorted = list_bugs(
            &conn,
            &BugFilter {
                sort_by: "severity".to_string(),
                sort_dir: "asc".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        let sevs: Vec<&str> = sorted.iter().map(|b| b.severity.as_str()).collect();
        assert_eq!(sevs, vec!["critical", "high", "low"]);
    }

    #[test]
    fn test_update_status_resolves() {
        let conn = init_memory_db().expect("init");
        let b = create_bug(&conn, &sample_bug("medium", "open", "x")).unwrap();
        update_bug_status(&conn, &b.id, "resolved", Some("admin".to_string())).unwrap();
        let after = get_bug(&conn, &b.id).unwrap();
        assert_eq!(after.status, "resolved");
        assert_eq!(after.resolved_by.as_deref(), Some("admin"));
        assert!(after.resolved_at.is_some());

        // Re-opening clears resolver.
        update_bug_status(&conn, &b.id, "open", None).unwrap();
        let reopened = get_bug(&conn, &b.id).unwrap();
        assert_eq!(reopened.status, "open");
        assert!(reopened.resolved_by.is_none());
        assert!(reopened.resolved_at.is_none());
    }

    #[test]
    fn test_attachment_roundtrip() {
        let conn = init_memory_db().expect("init");
        let b = create_bug(&conn, &sample_bug("high", "open", "x")).unwrap();
        let att = add_attachment(&conn, &b.id, "trace.log", "text/plain", "bG9nIGNvbnRlbnQ=")
            .unwrap();
        assert_eq!(att.filename, "trace.log");
        let fetched = get_bug(&conn, &b.id).unwrap();
        assert_eq!(fetched.attachments.len(), 1);
        assert_eq!(fetched.attachments[0].filename, "trace.log");
    }

    #[test]
    fn test_csv_export_contains_rows() {
        let conn = init_memory_db().expect("init");
        create_bug(&conn, &sample_bug("high", "open", "payment")).unwrap();
        let csv = export_bugs_csv(&conn, &BugFilter::default()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "id,created_at,severity,status,error_type,context,message,source,category,keywords,resolved_by");
        assert_eq!(lines.len(), 2, "header + 1 row");
    }

    #[test]
    fn test_pdf_export_produces_pdf() {
        let conn = init_memory_db().expect("init");
        create_bug(&conn, &sample_bug("critical", "open", "payment")).unwrap();
        let pdf = export_bugs_pdf(&conn, &BugFilter::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF"), "output must be a PDF document");
    }
}
