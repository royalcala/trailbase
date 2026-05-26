#![allow(clippy::needless_return)]

//! Schema introspection: reads live table and index definitions from sqlite_schema.

use rusqlite::Connection;

use crate::types::{LiveIndex, LiveSchema, LiveTable, LiveTrigger, LiveView, SchemaDiffError};

/// System-managed table name prefixes that should be excluded from user schema diff.
const EXCLUDED_TABLE_PREFIXES: &[&str] = &[
  "__", // TrailBase internal tables: __user, __session, etc.
  "_schema_history",
  "_schema_diff_meta",
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
  let mut views: Vec<LiveView> = vec![];
  let mut triggers: Vec<LiveTrigger> = vec![];

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
      let (name, table_name, sql) = row.map_err(|e| SchemaDiffError::Parse(e.to_string()))?;
      indexes.push(LiveIndex {
        name,
        table_name,
        sql,
      });
    }
  }

  // Views
  {
    let mut stmt = conn
      .prepare(
        "SELECT name, sql FROM sqlite_schema \
                 WHERE type = 'view' AND sql IS NOT NULL \
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
      views.push(LiveView { name, sql });
    }
  }

  // Triggers
  {
    let mut stmt = conn
      .prepare(
        "SELECT name, tbl_name, sql FROM sqlite_schema \
                 WHERE type = 'trigger' AND sql IS NOT NULL \
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
      let (name, table_name, sql) = row.map_err(|e| SchemaDiffError::Parse(e.to_string()))?;
      triggers.push(LiveTrigger {
        name,
        table_name,
        sql,
      });
    }
  }

  return Ok(LiveSchema {
    tables,
    indexes,
    views,
    triggers,
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn excludes_system_tables_and_keeps_user_tables() {
    let conn = Connection::open_in_memory().expect("conn");
    conn
      .execute_batch(
        "
        CREATE TABLE __internal(id INTEGER PRIMARY KEY);
        CREATE TABLE _schema_history(version INTEGER);
        CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);
        ",
      )
      .expect("seed schema");

    let live = introspect_schema(&conn).expect("introspect");
    assert!(live.tables.iter().any(|t| t.name == "users"));
    assert!(!live.tables.iter().any(|t| t.name == "__internal"));
    assert!(!live.tables.iter().any(|t| t.name == "_schema_history"));
  }

  #[test]
  fn includes_only_user_created_indexes() {
    let conn = Connection::open_in_memory().expect("conn");
    conn
      .execute_batch(
        "
        CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT UNIQUE, name TEXT);
        CREATE INDEX users_name_idx ON users(name);
        ",
      )
      .expect("seed schema");

    let live = introspect_schema(&conn).expect("introspect");
    assert!(live.indexes.iter().any(|i| i.name == "users_name_idx"));
    // SQLite auto-indexes for UNIQUE constraints have sql = NULL and should be excluded.
    assert!(
      !live
        .indexes
        .iter()
        .any(|i| i.name.starts_with("sqlite_autoindex"))
    );
  }

  #[test]
  fn includes_views_and_triggers() {
    let conn = Connection::open_in_memory().expect("conn");
    conn
      .execute_batch(
        "
        CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);
        CREATE VIEW active_users AS SELECT id, email FROM users;
        CREATE TRIGGER users_touch AFTER UPDATE ON users
        BEGIN
          SELECT 1;
        END;
        ",
      )
      .expect("seed schema");

    let live = introspect_schema(&conn).expect("introspect");
    assert!(live.views.iter().any(|v| v.name == "active_users"));
    assert!(live.triggers.iter().any(|t| t.name == "users_touch"));
  }
}
