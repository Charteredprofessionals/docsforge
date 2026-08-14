//! rules/mod.rs — Rules DSL engine root (TASK-114, REQ-037).
//!
//! Safe, deterministic rule expressions for conditional documents. The parser
//! (see `parser.rs`) only accepts a closed grammar, so evaluation is
//! side-effect-free and bounded. `validate_rule_expression` additionally
//! rejects references to unknown fields at parse time.

pub mod evaluate;
pub mod parser;

pub use evaluate::{
    add_rule, evaluate_preview, evaluate_rules, list_rules, remove_rule, DocumentDecision, Rule,
    RulesPreview, SkippedDocument,
};
pub use parser::{
    collect_field_refs, parse, BinOp, Expr, Literal, UnaryOp,
};

use std::collections::HashMap;

use crate::core::error::DocForgeError;
use rusqlite::Connection;
use serde_json::Value;

/// Evaluates a parsed expression against a field-value map.
///
/// `values` maps canonical field ids to their JSON values. A referenced field
/// that is missing resolves to `null`. The result is always a JSON value
/// (boolean for logical/comparison, the literal otherwise).
pub fn evaluate(expr: &Expr, values: &HashMap<String, Value>) -> Result<Value, DocForgeError> {
    match expr {
        Expr::Literal(Literal::Number(n)) => Ok(Value::from(*n)),
        Expr::Literal(Literal::Text(s)) => Ok(Value::String(s.clone())),
        Expr::Literal(Literal::Bool(b)) => Ok(Value::Bool(*b)),
        Expr::FieldRef(name) => Ok(values.get(name).cloned().unwrap_or(Value::Null)),
        Expr::Unary(UnaryOp::Not, e) => {
            let v = evaluate(e, values)?;
            Ok(Value::Bool(!is_truthy(&v)))
        }
        Expr::Binary(l, op, r) => {
            let lv = evaluate(l, values)?;
            let rv = evaluate(r, values)?;
            Ok(eval_binary(*op, &lv, &rv))
        }
    }
}

fn eval_binary(op: BinOp, lv: &Value, rv: &Value) -> Value {
    let result = match op {
        BinOp::And => is_truthy(lv) && is_truthy(rv),
        BinOp::Or => is_truthy(lv) || is_truthy(rv),
        BinOp::Eq => values_equal(lv, rv),
        BinOp::Ne => !values_equal(lv, rv),
        BinOp::Lt => compare(lv, rv).map(|o| o == std::cmp::Ordering::Less).unwrap_or(false),
        BinOp::Le => compare(lv, rv).map(|o| o != std::cmp::Ordering::Greater).unwrap_or(false),
        BinOp::Gt => compare(lv, rv).map(|o| o == std::cmp::Ordering::Greater).unwrap_or(false),
        BinOp::Ge => compare(lv, rv).map(|o| o != std::cmp::Ordering::Less).unwrap_or(false),
    };
    Value::Bool(result)
}

/// Truthiness: booleans as-is; numbers non-zero; non-empty strings; null false.
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Null => false,
        _ => true,
    }
}

/// Equality: numbers by value, strings/bools by value, null to null.
fn values_equal(l: &Value, r: &Value) -> bool {
    match (l, r) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        // Cross-type equality for convenience: string vs number.
        (Value::Number(a), Value::String(b)) => b.parse::<f64>().map(|n| a.as_f64() == Some(n)).unwrap_or(false),
        (Value::String(a), Value::Number(b)) => a.parse::<f64>().map(|n| Some(n) == b.as_f64()).unwrap_or(false),
        _ => false,
    }
}

/// Comparison: numbers numerically, strings lexicographically (ISO dates sort
/// correctly), booleans by rank. Returns None if types are incomparable.
fn compare(l: &Value, r: &Value) -> Option<std::cmp::Ordering> {
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => a.as_f64().zip(b.as_f64()).map(|(x, y)| x.partial_cmp(&y).unwrap()),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        (Value::Number(a), Value::String(b)) => {
            b.parse::<f64>().ok().and_then(|n| a.as_f64().map(|x| x.partial_cmp(&n).unwrap()))
        }
        (Value::String(a), Value::Number(b)) => {
            a.parse::<f64>().ok().and_then(|n| n.partial_cmp(&b.as_f64().unwrap()))
        }
        _ => None,
    }
}

/// Validates a rule expression string against a bundle version's fields.
///
/// Parses the expression (rejecting any unsupported syntax) and verifies that
/// every referenced field exists in the `fields` table for the bundle version.
/// Returns the list of referenced field ids on success.
pub fn validate_rule_expression(
    conn: &Connection,
    bundle_version_id: &str,
    expression: &str,
) -> Result<Vec<String>, DocForgeError> {
    let expr = parse(expression)?;
    let refs = collect_field_refs(&expr);

    let mut unknown = Vec::new();
    for r in &refs {
        let exists: i32 = conn
            .query_row(
                "SELECT COUNT(1) FROM fields WHERE field_id = ?1 AND bundle_version_id = ?2",
                rusqlite::params![r, bundle_version_id],
                |row| row.get(0),
            )
            .map_err(|e| DocForgeError::StorageIo(format!("Check rule field '{r}': {e}")))?;
        if exists == 0 {
            unknown.push(r.clone());
        }
    }

    if !unknown.is_empty() {
        return Err(DocForgeError::InvalidInput(format!(
            "Rule references unknown field(s): {}",
            unknown.join(", ")
        )));
    }
    Ok(refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
    }

    #[test]
    fn test_dsl_rejects_function_calls() {
        let expr = "contains(name, 'x')";
        let conn = crate::schema::init_memory_db().expect("mem");
        let bv = "bv-x";
        // No 'fields' table rows needed since parse fails before validation on unknown syntax.
        let res = parse(expr);
        assert!(res.is_err(), "function calls must not parse");
        let _ = bv; // keep lint quiet if unused
        let _ = &conn;
    }

    #[test]
    fn test_evaluate_comparison_and_logical() {
        let expr = parse("age >= 18 && country == \"IN\"").expect("parse");
        let v = vals(&[
            ("age", Value::from(21)),
            ("country", Value::String("IN".to_string())),
        ]);
        assert_eq!(evaluate(&expr, &v).unwrap(), Value::Bool(true));

        let v2 = vals(&[
            ("age", Value::from(16)),
            ("country", Value::String("IN".to_string())),
        ]);
        assert_eq!(evaluate(&expr, &v2).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_evaluate_unary_not_and_parens() {
        let expr = parse("!(status == \"closed\")").expect("parse");
        let v = vals(&[("status", Value::String("open".to_string()))]);
        assert_eq!(evaluate(&expr, &v).unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_evaluate_string_literal_comparison() {
        // ISO date strings compare lexicographically and correctly.
        let expr = parse("incorporation_date < \"2020-01-01\"").expect("parse");
        let v = vals(&[("incorporation_date", Value::String("2019-05-01".to_string()))]);
        assert_eq!(evaluate(&expr, &v).unwrap(), Value::Bool(true));
        let v2 = vals(&[("incorporation_date", Value::String("2021-05-01".to_string()))]);
        assert_eq!(evaluate(&expr, &v2).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_validate_rule_expression_unknown_field() {
        let conn = crate::schema::init_memory_db().expect("mem");
        // Set up a bundle version + one field so validation has real data.
        let bundle = crate::core::bundle::manifest::create_bundle(&conn, "R", None, None).expect("bundle");
        let bv = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("bv");
        crate::core::field_mapping::registry::create_field(
            &conn,
            &bv,
            &crate::core::field_mapping::schema::FieldDef {
                id: String::new(),
                field_id: "age".to_string(),
                label: "Age".to_string(),
                description: None,
                field_type: crate::core::field_mapping::schema::FieldType::Number,
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

        assert!(validate_rule_expression(&conn, &bv, "age >= 18").is_ok());
        assert!(validate_rule_expression(&conn, &bv, "missing >= 18").is_err());
    }
}
