#![allow(clippy::needless_return)]

//! Core diff engine: compares desired schema (parsed SQL) vs live database state.
//!
//! Supported operations (MVP):
//!   - CREATE TABLE (new table in desired, not in live)
//!   - ADD COLUMN   (new nullable or defaulted column in existing table)
//!   - CREATE INDEX (new index in desired, not in live)
//!   - DROP INDEX   (index in live, not in desired)
//!
//! Gated operations (require policy flags):
//!   - DROP TABLE   (table in live, not in desired) → requires allow_destructive
//!   - DROP COLUMN  → not supported in MVP (SQLite limitation; requires table rebuild)
//!   - NOT NULL column without DEFAULT → unsupported
//!
//! Unsupported operations will be flagged with is_supported=false for diagnostic output.

use std::collections::HashMap;

use trailbase_schema::parse::parse_into_statements;
use trailbase_schema::sqlite::{Table, TableIndex};

use crate::types::{DiffOperation, LiveSchema, SchemaDiff, SchemaDiffError};

/// Parse desired schema SQL into tables and indexes.
fn parse_desired(sql: &str) -> Result<(Vec<Table>, Vec<TableIndex>), SchemaDiffError> {
  let stmts =
    parse_into_statements(sql).map_err(|e| SchemaDiffError::Parse(format!("Parse error: {e}")))?;

  let mut tables: Vec<Table> = vec![];
  let mut indexes: Vec<TableIndex> = vec![];

  for stmt in stmts {
    use sqlite3_parser::ast::Stmt;
    match &stmt {
      Stmt::CreateTable { .. } | Stmt::CreateVirtualTable { .. } => {
        let table: Table =
          stmt
            .try_into()
            .map_err(|e: trailbase_schema::sqlite::SchemaError| {
              SchemaDiffError::Parse(e.to_string())
            })?;
        tables.push(table);
      }
      Stmt::CreateIndex { .. } => {
        let index: TableIndex =
          stmt
            .try_into()
            .map_err(|e: trailbase_schema::sqlite::SchemaError| {
              SchemaDiffError::Parse(e.to_string())
            })?;
        indexes.push(index);
      }
      // Silently skip comments, views, triggers etc. (not in MVP scope).
      _ => {}
    }
  }

  return Ok((tables, indexes));
}

/// Normalize a CREATE TABLE or CREATE INDEX SQL string for comparison.
/// Strips extra whitespace and converts to uppercase for stable comparison.
fn normalize_sql(sql: &str) -> String {
  return sql
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .to_uppercase();
}

/// Compare desired table definition against live and produce ADD COLUMN operations.
fn diff_table_columns(
  desired: &Table,
  live_sql: &str,
) -> Result<Vec<DiffOperation>, SchemaDiffError> {
  // Parse the live CREATE TABLE to extract existing column names.
  let live_stmts = parse_into_statements(live_sql)
    .map_err(|e| SchemaDiffError::Parse(format!("Parse live table: {e}")))?;

  let live_table: Table = live_stmts
    .into_iter()
    .next()
    .ok_or_else(|| SchemaDiffError::Parse("Empty live schema for table".into()))?
    .try_into()
    .map_err(|e: trailbase_schema::sqlite::SchemaError| SchemaDiffError::Parse(e.to_string()))?;

  let live_cols: HashMap<String, ()> = live_table
    .columns
    .iter()
    .map(|c| (c.name.to_lowercase(), ()))
    .collect();

  let mut ops: Vec<DiffOperation> = vec![];

  for col in &desired.columns {
    if live_cols.contains_key(&col.name.to_lowercase()) {
      continue;
    }

    let table_name = &desired.name.name;

    // NOT NULL without DEFAULT on new column requires all rows to have a value:
    // unsupported in MVP because it can't safely be applied to existing data.
    if col.is_not_null() && !col.has_default() && !col.is_primary() {
      ops.push(DiffOperation {
        description: format!(
          "ADD COLUMN '{}.{}' NOT NULL without DEFAULT (unsupported)",
          table_name, col.name
        ),
        is_destructive: false,
        requires_table_rebuild: false,
        is_supported: false,
        sql: format!(
          "-- UNSUPPORTED: ALTER TABLE \"{table_name}\" ADD COLUMN {} \
                     (NOT NULL without DEFAULT requires manual migration)",
          col.name
        ),
      });
      continue;
    }

    // Build the ADD COLUMN SQL: reuse the column's fragment. We need the type
    // string; leverage create_table_statement fragments via the column's Display.
    let desired_col_sql = desired.create_table_statement();
    // Extract the column definition fragment by re-parsing desired and finding the column.
    // Simplest safe approach: emit standard ADD COLUMN with type and options.
    let col_fragment = format!("\"{}\" {}{}", col.name, col.type_name, {
      let options: Vec<String> = col
        .options
        .iter()
        .filter_map(|opt| {
          use trailbase_schema::sqlite::ColumnOption;
          match opt {
            ColumnOption::Default(d) => Some(format!("DEFAULT {d}")),
            ColumnOption::NotNull => Some("NOT NULL".to_string()),
            _ => None,
          }
        })
        .collect();
      if options.is_empty() {
        String::new()
      } else {
        format!(" {}", options.join(" "))
      }
    });

    // Suppress the unused variable warning from the desired_col_sql computed above.
    let _ = desired_col_sql;

    ops.push(DiffOperation {
      description: format!("ADD COLUMN '{}.{}'", table_name, col.name),
      is_destructive: false,
      requires_table_rebuild: false,
      is_supported: true,
      sql: format!("ALTER TABLE \"{table_name}\" ADD COLUMN {col_fragment}"),
    });
  }

  return Ok(ops);
}

/// Compute a full schema diff between desired SQL and live database state.
///
/// Returns a `SchemaDiff` containing all proposed operations ordered by
/// dependency safety (CREATE TABLE before CREATE INDEX).
pub fn compute_diff(desired_sql: &str, live: &LiveSchema) -> Result<SchemaDiff, SchemaDiffError> {
  let (desired_tables, desired_indexes) = parse_desired(desired_sql)?;

  let live_tables: HashMap<String, &str> = live
    .tables
    .iter()
    .map(|t| (t.name.to_lowercase(), t.sql.as_str()))
    .collect();

  let live_indexes: HashMap<String, &str> = live
    .indexes
    .iter()
    .map(|i| (i.name.to_lowercase(), i.sql.as_str()))
    .collect();

  let desired_table_names: HashMap<String, ()> = desired_tables
    .iter()
    .map(|t| (t.name.name.to_lowercase(), ()))
    .collect();

  let desired_index_names: HashMap<String, ()> = desired_indexes
    .iter()
    .map(|i| (i.name.name.to_lowercase(), ()))
    .collect();

  let mut operations: Vec<DiffOperation> = vec![];

  // --- Tables: new tables ---
  for table in &desired_tables {
    let key = table.name.name.to_lowercase();
    if !live_tables.contains_key(&key) {
      operations.push(DiffOperation {
        description: format!("CREATE TABLE '{}'", table.name.name),
        is_destructive: false,
        requires_table_rebuild: false,
        is_supported: true,
        sql: table.create_table_statement(),
      });
    } else {
      // Table exists: check for new columns (ADD COLUMN).
      let live_sql = live_tables[&key];
      let col_ops = diff_table_columns(table, live_sql)?;
      operations.extend(col_ops);
    }
  }

  // --- Tables: dropped tables (destructive, gated) ---
  for live_table in &live.tables {
    let key = live_table.name.to_lowercase();
    if !desired_table_names.contains_key(&key) {
      operations.push(DiffOperation {
        description: format!(
          "DROP TABLE '{}' (destructive, requires --allow-destructive)",
          live_table.name
        ),
        is_destructive: true,
        requires_table_rebuild: false,
        is_supported: true,
        sql: format!("DROP TABLE \"{}\"", live_table.name),
      });
    }
  }

  // --- Indexes: new indexes ---
  for index in &desired_indexes {
    let key = index.name.name.to_lowercase();
    if !live_indexes.contains_key(&key) {
      operations.push(DiffOperation {
        description: format!(
          "CREATE INDEX '{}' ON '{}'",
          index.name.name, index.table_name
        ),
        is_destructive: false,
        requires_table_rebuild: false,
        is_supported: true,
        sql: index.create_index_statement(),
      });
    } else {
      // If index SQL changed, flag it as needing rebuild (drop + recreate).
      let live_sql = live_indexes[&key];
      if normalize_sql(live_sql) != normalize_sql(&index.create_index_statement()) {
        operations.push(DiffOperation {
          description: format!("RECREATE INDEX '{}' (definition changed)", index.name.name),
          is_destructive: false,
          requires_table_rebuild: false,
          is_supported: true,
          sql: format!(
            "DROP INDEX IF EXISTS \"{}\";\n{}",
            index.name.name,
            index.create_index_statement()
          ),
        });
      }
    }
  }

  // --- Indexes: dropped indexes (gated) ---
  for live_index in &live.indexes {
    let key = live_index.name.to_lowercase();
    if !desired_index_names.contains_key(&key) {
      operations.push(DiffOperation {
        description: format!(
          "DROP INDEX '{}' (requires --allow-destructive)",
          live_index.name
        ),
        is_destructive: true,
        requires_table_rebuild: false,
        is_supported: true,
        sql: format!("DROP INDEX IF EXISTS \"{}\"", live_index.name),
      });
    }
  }

  return Ok(SchemaDiff { operations });
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::introspect::introspect_schema;
  use indoc::indoc;

  fn live_schema(sql: &str) -> LiveSchema {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");
    conn.execute_batch(sql).expect("seed schema");
    introspect_schema(&conn).expect("introspect")
  }

  #[test]
  fn detects_create_table() {
    let live = LiveSchema::default();
    let desired = "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);";

    let diff = compute_diff(desired, &live).expect("diff");
    assert_eq!(diff.operations.len(), 1);
    assert!(
      diff.operations[0]
        .description
        .contains("CREATE TABLE 'users'")
    );
    assert!(!diff.operations[0].is_destructive);
  }

  #[test]
  fn detects_add_column() {
    let live = live_schema("CREATE TABLE users(id INTEGER PRIMARY KEY);");
    let desired = "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);";

    let diff = compute_diff(desired, &live).expect("diff");
    assert!(
      diff
        .operations
        .iter()
        .any(|op| op.description.contains("ADD COLUMN 'users.email'"))
    );
  }

  #[test]
  fn marks_drop_table_destructive() {
    let live = live_schema(indoc! {
        "
            CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);
            CREATE TABLE posts(id INTEGER PRIMARY KEY, title TEXT);
            "
    });

    let desired = "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);";
    let diff = compute_diff(desired, &live).expect("diff");

    let drop_posts = diff
      .operations
      .iter()
      .find(|op| op.sql.contains("DROP TABLE \"posts\""))
      .expect("drop posts operation");
    assert!(drop_posts.is_destructive);
  }
}
