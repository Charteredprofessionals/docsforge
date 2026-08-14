//! rules/evaluate.rs — Conditional document rules (TASK-115, REQ-036).
//!
//! Bundle-level rules decide whether each document is generated. Each rule
//! binds a `condition_expr` (the closed DSL from `parser.rs`) to a
//! `document_id`; the document is included when its (enabled) conditions hold.
//! `evaluate_rules` returns a per-document Include/Exclude decision, and
//! `evaluate_preview` summarizes the inclusion count plus every skipped
//! document with a human-readable reason. Both share the same evaluation path.

use std::collections::HashMap;

use crate::core::error::DocForgeError;
use crate::core::rules::parser::{parse, Expr};
use crate::core::rules::validate_rule_expression;
use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

/// A stored conditional-document rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub bundle_version_id: String,
    pub document_id: Option<String>,
    pub field_id: Option<String>,
    pub operator: Option<String>,
    pub value_json: Option<String>,
    pub condition_expr: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
}

/// Per-document inclusion decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDecision {
    pub document_id: String,
    pub document_name: String,
    pub included: bool,
    pub reason: String,
}

/// Summary of a conditional-document evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesPreview {
    pub total_documents: usize,
    pub included_count: usize,
    pub skipped: Vec<SkippedDocument>,
}

/// A document excluded by the rules, with its human reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedDocument {
    pub document_id: String,
    pub document_name: String,
    pub reason: String,
}

/// Adds a conditional rule for a document. The `condition_expr` is validated
/// (parsed + field references checked) before persisting.
pub fn add_rule(
    conn: &Connection,
    bundle_version_id: &str,
    document_id: &str,
    condition_expr: &str,
    description: Option<&str>,
) -> Result<Rule, DocForgeError> {
    validate_rule_expression(conn, bundle_version_id, condition_expr)?;

    let id = format!("rule_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO rules (id, bundle_version_id, document_id, condition_expr, description, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        rusqlite::params![id, bundle_version_id, document_id, condition_expr, description],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert rule: {e}")))?;

    Ok(Rule {
        id,
        bundle_version_id: bundle_version_id.to_string(),
        document_id: Some(document_id.to_string()),
        field_id: None,
        operator: None,
        value_json: None,
        condition_expr: Some(condition_expr.to_string()),
        description: description.map(str::to_string),
        enabled: true,
    })
}

/// Removes a rule by id.
pub fn remove_rule(conn: &Connection, rule_id: &str) -> Result<(), DocForgeError> {
    conn.execute("DELETE FROM rules WHERE id = ?1", [rule_id])
        .map_err(|e| DocForgeError::StorageIo(format!("Delete rule: {e}")))?;
    Ok(())
}

/// Lists all rules for a bundle version.
pub fn list_rules(conn: &Connection, bundle_version_id: &str) -> Result<Vec<Rule>, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, bundle_version_id, document_id, field_id, operator, value_json,
                    condition_expr, description, enabled
             FROM rules WHERE bundle_version_id = ?1 ORDER BY document_id",
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list_rules: {e}")))?;
    let rows = stmt
        .query_map([bundle_version_id], |r| {
            Ok(Rule {
                id: r.get(0)?,
                bundle_version_id: r.get(1)?,
                document_id: r.get(2)?,
                field_id: r.get(3)?,
                operator: r.get(4)?,
                value_json: r.get(5)?,
                condition_expr: r.get(6)?,
                description: r.get(7)?,
                enabled: r.get::<_, i32>(8)? != 0,
            })
        })
        .map_err(|e| DocForgeError::StorageIo(format!("Query rules: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| DocForgeError::StorageIo(format!("Map rule row: {e}")))?);
    }
    Ok(out)
}

fn matter_to_map(matter_data: &Value) -> HashMap<String, Value> {
    match matter_data {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => HashMap::new(),
    }
}

/// Evaluates the rules for every document in the bundle version, returning a
/// per-document Include/Exclude decision (REQ-036).
pub fn evaluate_rules(
    conn: &Connection,
    bundle_version_id: &str,
    matter_data: &Value,
) -> Result<Vec<DocumentDecision>, DocForgeError> {
    let values = matter_to_map(matter_data);
    let rules = list_rules(conn, bundle_version_id)?;

    let documents: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id FROM bundle_documents WHERE bundle_version_id = ?1 ORDER BY position")
            .map_err(|e| DocForgeError::StorageIo(format!("Prepare documents: {e}")))?;
        let rows = stmt
            .query_map([bundle_version_id], |r| r.get::<_, String>(0))
            .map_err(|e| DocForgeError::StorageIo(format!("Query documents: {e}")))?;
        let mut out = Vec::new();
        for r in rows {
            let doc_id = r.map_err(|e| DocForgeError::StorageIo(format!("Map document row: {e}")))?;
            out.push((doc_id.clone(), doc_id));
        }
        out
    };

    let mut decisions = Vec::new();
    for (doc_id, doc_name) in documents {
        let doc_rules: Vec<&Rule> = rules
            .iter()
            .filter(|r| r.enabled && r.document_id.as_deref() == Some(doc_id.as_str()))
            .collect();

        if doc_rules.is_empty() {
            decisions.push(DocumentDecision {
                document_id: doc_id,
                document_name: doc_name,
                included: true,
                reason: "No conditional rule; included by default".to_string(),
            });
            continue;
        }

        let mut failing = Vec::new();
        let mut included = true;
        for rule in &doc_rules {
            let expr: Expr = match rule.condition_expr.as_deref() {
                Some(e) => parse(e)?,
                None => continue,
            };
            let result = crate::core::rules::evaluate(&expr, &values)?;
            let holds = result.as_bool().unwrap_or(false);
            if !holds {
                included = false;
                failing.push(rule.condition_expr.clone().unwrap_or_default());
            }
        }

        let reason = if included {
            "All conditions satisfied".to_string()
        } else {
            format!("Condition(s) not met: {}", failing.join(" AND "))
        };
        decisions.push(DocumentDecision {
            document_id: doc_id,
            document_name: doc_name,
            included,
            reason,
        });
    }
    Ok(decisions)
}

/// Summarizes the conditional-document evaluation: total, included count, and
/// each skipped document with its human reason (REQ-036 preview).
pub fn evaluate_preview(
    conn: &Connection,
    bundle_version_id: &str,
    matter_data: &Value,
) -> Result<RulesPreview, DocForgeError> {
    let decisions = evaluate_rules(conn, bundle_version_id, matter_data)?;
    let total = decisions.len();
    let mut skipped = Vec::new();
    let mut included_count = 0;
    for d in &decisions {
        if d.included {
            included_count += 1;
        } else {
            skipped.push(SkippedDocument {
                document_id: d.document_id.clone(),
                document_name: d.document_name.clone(),
                reason: d.reason.clone(),
            });
        }
    }
    Ok(RulesPreview {
        total_documents: total,
        included_count,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::{create_bundle, get_manifest, save_manifest};
    use crate::core::field_mapping::registry::create_field;
    use crate::core::field_mapping::schema::FieldType;
    use crate::schema::init_memory_db;

    fn setup() -> (Connection, String, String) {
        let conn = init_memory_db().expect("mem");
        let bundle = create_bundle(&conn, "Cond Test", None, None).expect("bundle");
        let bv = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("bv");

        // Two documents.
        let mut manifest = get_manifest(&conn, &bv).expect("manifest");
        manifest.documents = vec![
            crate::core::bundle::manifest::BundleDocumentSpec {
                document_id: "doc-a".to_string(),
                template_id: String::new(),
                position: 0,
                include_default: true,
                condition_ref: None,
            },
            crate::core::bundle::manifest::BundleDocumentSpec {
                document_id: "doc-b".to_string(),
                template_id: String::new(),
                position: 1,
                include_default: true,
                condition_ref: None,
            },
        ];
        save_manifest(&conn, &bv, &manifest).expect("save manifest");

        // Insert bundle_documents rows (manifest save may not persist them).
        for (i, did) in ["doc-a", "doc-b"].iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO bundle_documents (id, bundle_version_id, template_id, position, include_default)
                 VALUES (?1, ?2, NULL, ?3, 1)",
                rusqlite::params![did, bv, i as i32],
            )
            .expect("insert doc");
        }

        // A field used by rules.
        create_field(
            &conn,
            &bv,
            &crate::core::field_mapping::schema::FieldDef {
                id: String::new(),
                field_id: "is_premium".to_string(),
                label: "Is Premium".to_string(),
                description: None,
                field_type: FieldType::Boolean,
                required: false,
                default: None,
                validation: None,
                group_id: None,
                options: Vec::new(),
                format: None,
                position: 0,
            },
        )
        .expect("field");

        (conn, bv, bundle.id)
    }

    #[test]
    fn test_conditional_doc_skipped_with_reason() {
        let (conn, bv, _bundle_id) = setup();
        add_rule(&conn, &bv, "doc-b", "is_premium == true", Some("Premium only")).expect("add rule");

        // Matter where is_premium is false -> doc-b excluded.
        let data = serde_json::json!({"is_premium": false});
        let preview = evaluate_preview(&conn, &bv, &data).expect("preview");
        assert_eq!(preview.total_documents, 2);
        assert_eq!(preview.included_count, 1);
        assert_eq!(preview.skipped.len(), 1);
        assert_eq!(preview.skipped[0].document_id, "doc-b");
        assert!(preview.skipped[0].reason.contains("is_premium == true"));

        // Matter where is_premium is true -> both included.
        let data2 = serde_json::json!({"is_premium": true});
        let preview2 = evaluate_preview(&conn, &bv, &data2).expect("preview2");
        assert_eq!(preview2.included_count, 2);
        assert!(preview2.skipped.is_empty());
    }

    #[test]
    fn test_add_rule_rejects_unknown_field() {
        let (conn, bv, _bundle_id) = setup();
        let err = add_rule(&conn, &bv, "doc-a", "nonexistent == 1", None).expect_err("rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }
}
