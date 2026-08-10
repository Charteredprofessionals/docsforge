//! bundles.rs — Template Bundle management.
//!
//! A Bundle groups multiple templates so they can be processed together (e.g. a
//! full document set for a client). Adopted from the `templatebuilder` sibling project.

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bundle {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BundleTemplate {
    pub id: String,
    pub bundle_id: String,
    pub template_id: String,
    pub position: i64,
}

pub fn create_bundle(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    template_ids: &[String],
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO bundles (id, name, description) VALUES (?1, ?2, ?3)",
        params![id, name, description.unwrap_or("")],
    )?;

    for (pos, tid) in template_ids.iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO bundle_templates (id, bundle_id, template_id, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![Uuid::new_v4().to_string(), id, tid, pos as i64],
        )?;
    }
    Ok(id)
}

pub fn list_bundles(conn: &Connection) -> Result<Vec<Bundle>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at FROM bundles ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Bundle {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn get_bundle_templates(conn: &Connection, bundle_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT template_id FROM bundle_templates WHERE bundle_id = ?1 ORDER BY position ASC",
    )?;
    let rows = stmt.query_map(params![bundle_id], |row| row.get::<_, String>(0))?;
    rows.collect()
}

pub fn delete_bundle(conn: &Connection, bundle_id: &str) -> Result<()> {
    conn.execute("DELETE FROM bundles WHERE id = ?1", params![bundle_id])?;
    Ok(())
}

pub fn add_template_to_bundle(
    conn: &Connection,
    bundle_id: &str,
    template_id: &str,
) -> Result<()> {
    let pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM bundle_templates WHERE bundle_id = ?1",
        params![bundle_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO bundle_templates (id, bundle_id, template_id, position)
         VALUES (?1, ?2, ?3, ?4)",
        params![Uuid::new_v4().to_string(), bundle_id, template_id, pos],
    )?;
    Ok(())
}

pub fn remove_template_from_bundle(
    conn: &Connection,
    bundle_id: &str,
    template_id: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM bundle_templates WHERE bundle_id = ?1 AND template_id = ?2",
        params![bundle_id, template_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_memory_db;

    #[test]
    fn test_bundle_crud() {
        let conn = init_memory_db().expect("memory db");
        let id = create_bundle(
            &conn,
            "Onboarding",
            Some("New hire set"),
            &["tpl_a".to_string(), "tpl_b".to_string()],
        )
        .expect("create");

        let bundles = list_bundles(&conn).expect("list");
        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "Onboarding");

        let ids = get_bundle_templates(&conn, &id).expect("get members");
        assert_eq!(ids.len(), 2);

        add_template_to_bundle(&conn, &id, "tpl_c").expect("add");
        assert_eq!(get_bundle_templates(&conn, &id).expect("get").len(), 3);

        remove_template_from_bundle(&conn, &id, "tpl_a").expect("remove");
        assert_eq!(get_bundle_templates(&conn, &id).expect("get").len(), 2);

        delete_bundle(&conn, &id).expect("delete");
        assert_eq!(list_bundles(&conn).expect("list").len(), 0);
    }
}
