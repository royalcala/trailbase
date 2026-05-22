#![allow(clippy::needless_return)]

//! Schema introspection: reads live table and index definitions from sqlite_schema.

use rusqlite::Connection;

use crate::types::{LiveIndex, LiveSchema, LiveTable, SchemaDiffError};

/// System-managed table name prefixes that should be excluded from user schema diff.
const EXCLUDED_TABLE_PREFIXES: &[&str] = &[
    "__", // TrailBase internal tables: __user, __session, etc.
    "_schema_history",
    "sqlite_",
];

/// Returns true if a table name is managed by TrailBase or SQLite internally.
fn is_system_table(name: &str) -> bool {
    return EXCLUDED_TABLE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix));
}

/// Read all user-managed tables and indexes from the live database.
pub fn introspect_schema(conn: &Connection) -> Result<LiveSchema, SchemaDiffError> {
    let mut tables: Vec<LiveTable> = vec![];
    let mut indexes: Vec<LiveIndex> = vec![];

    // Tables
    {
        let mut stmt = conn
            .prepare(
                "SELECT name, sql FROM sqlite_schema \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
                 ORDER BY name",
            )
            .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let sql: Option<String> = row.get(1)?;
                Ok((name, sql.unwrap_or_default()))
            })
            .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

        for row in rows {
            let (name, sql) = row.map_err(|e| SchemaDiffError::Parse(e.to_string()))?;
            if is_system_table(&name) {
                continue;
            }
            tables.push(LiveTable { name, sql });
        }
    }

    // User-created indexes (skip auto-created sqlite indexes)
    {
        let mut stmt = conn
            .prepare(
                "SELECT name, tbl_name, sql FROM sqlite_schema \
                 WHERE type = 'index' AND sql IS NOT NULL \
                 ORDER BY name",
            )
            .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let table_name: String = row.get(1)?;
                let sql: Option<String> = row.get(2)?;
                Ok((name, table_name, sql.unwrap_or_default()))
            })
            .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

        for row in rows {
            let (name, table_name, sql) =
                row.map_err(|e| SchemaDiffError::Parse(e.to_string()))?;
            indexes.push(LiveIndex {
                name,
                table_name,
                sql,
            });
        }
    }

    return Ok(LiveSchema { tables, indexes });
}
