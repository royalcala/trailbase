#![allow(clippy::needless_return)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Operational mode for schema management.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SchemaMode {
  /// Classic append-only migrations (default, no change in behavior).
  #[default]
  Append,
  /// Declarative mode: compute diff from schema file and materialize as migration.
  Declarative,
}

/// Startup check policy for declarative mode.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SchemaCheckPolicy {
  /// Skip schema drift checking entirely.
  Off,
  /// Check and apply when fingerprint changed (default for declarative mode).
  #[default]
  On,
  /// Fail startup on any unsupported or unsafe schema diff operation.
  Strict,
}

/// A single SQL operation produced by the diff engine.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffOperation {
  /// Human-readable description of the operation.
  pub description: String,

  /// Whether this operation is destructive (e.g. DROP TABLE, DROP COLUMN).
  pub is_destructive: bool,

  /// Whether this operation requires a full table rebuild (SQLite limitation).
  pub requires_table_rebuild: bool,

  /// Whether the diff engine considers this safe to emit without explicit flags.
  pub is_supported: bool,

  /// The SQL statement to execute.
  pub sql: String,
}

/// The full diff result between desired and actual schema.
#[derive(Clone, Debug, Default)]
pub struct SchemaDiff {
  pub operations: Vec<DiffOperation>,
}

impl SchemaDiff {
  pub fn is_empty(&self) -> bool {
    return self.operations.is_empty();
  }

  pub fn has_destructive(&self) -> bool {
    return self.operations.iter().any(|op| op.is_destructive);
  }

  pub fn has_unsupported(&self) -> bool {
    return self.operations.iter().any(|op| !op.is_supported);
  }

  pub fn has_table_rebuild(&self) -> bool {
    return self.operations.iter().any(|op| op.requires_table_rebuild);
  }

  /// Return only the supported safe operations.
  pub fn safe_operations(&self) -> Vec<&DiffOperation> {
    return self
      .operations
      .iter()
      .filter(|op| op.is_supported && !op.is_destructive && !op.requires_table_rebuild)
      .collect();
  }

  /// Build final SQL from all operations.
  pub fn to_sql(&self) -> String {
    return self
      .operations
      .iter()
      .map(|op| {
        let sql = op.sql.trim();
        if sql.ends_with(';') {
          sql.to_string()
        } else {
          format!("{sql};")
        }
      })
      .collect::<Vec<_>>()
      .join("\n");
  }
}

/// Errors produced by the schema diff pipeline.
#[derive(Debug, Error)]
pub enum SchemaDiffError {
  #[error("Schema parse error: {0}")]
  Parse(String),

  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),

  #[error("Policy violation: {0}")]
  PolicyViolation(String),

  #[error("Unsupported operation: {0}")]
  Unsupported(String),
}

/// Represents a live table read from the database.
#[derive(Clone, Debug)]
pub struct LiveTable {
  pub name: String,
  pub sql: String,
}

/// Represents a live index read from the database.
#[derive(Clone, Debug)]
pub struct LiveIndex {
  pub name: String,
  pub table_name: String,
  pub sql: String,
}

/// Represents a live view read from the database.
#[derive(Clone, Debug)]
pub struct LiveView {
  pub name: String,
  pub sql: String,
}

/// Represents a live trigger read from the database.
#[derive(Clone, Debug)]
pub struct LiveTrigger {
  pub name: String,
  pub table_name: String,
  pub sql: String,
}

/// Live schema as introspected from a running SQLite database.
#[derive(Clone, Debug, Default)]
pub struct LiveSchema {
  pub tables: Vec<LiveTable>,
  pub indexes: Vec<LiveIndex>,
  pub views: Vec<LiveView>,
  pub triggers: Vec<LiveTrigger>,
}
