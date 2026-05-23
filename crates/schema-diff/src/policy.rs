#![allow(clippy::needless_return)]

//! Policy enforcement: validate that all diff operations are acceptable
//! based on provided flags before writing any migration file.

use crate::types::{DiffOperation, SchemaDiff, SchemaDiffError};

/// Policy configuration for the diff engine.
#[derive(Clone, Debug, Default)]
pub struct PolicyConfig {
  /// Allow DROP TABLE and DROP COLUMN operations.
  pub allow_destructive: bool,

  /// Allow operations that require a full SQLite table rebuild.
  pub allow_table_rebuild: bool,
}

/// Apply policy checks against the diff operations.
///
/// Returns Err if any operation violates the active policy, with a
/// human-readable explanation of the violation.
pub fn apply_policy(diff: &SchemaDiff, policy: &PolicyConfig) -> Result<(), SchemaDiffError> {
  let mut violations: Vec<String> = vec![];

  for op in &diff.operations {
    if !op.is_supported {
      violations.push(format!(
        "Unsupported operation: {} (sql: {})",
        op.description, op.sql
      ));
    }
    if op.is_destructive && !policy.allow_destructive {
      violations.push(format!(
        "Destructive operation requires --allow-destructive: {} (sql: {})",
        op.description, op.sql
      ));
    }
    if op.requires_table_rebuild && !policy.allow_table_rebuild {
      violations.push(format!(
        "Table rebuild required, add --allow-table-rebuild: {} (sql: {})",
        op.description, op.sql
      ));
    }
  }

  if !violations.is_empty() {
    return Err(SchemaDiffError::PolicyViolation(violations.join("\n")));
  }

  return Ok(());
}

/// Filter operations to only include those safe for automatic application.
/// Used in startup declarative mode without explicit flags.
#[allow(dead_code)]
pub fn filter_safe_operations(ops: Vec<DiffOperation>) -> Vec<DiffOperation> {
  return ops
    .into_iter()
    .filter(|op| op.is_supported && !op.is_destructive && !op.requires_table_rebuild)
    .collect();
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::SchemaDiff;

  fn op(
    description: &str,
    is_supported: bool,
    is_destructive: bool,
    requires_table_rebuild: bool,
  ) -> DiffOperation {
    DiffOperation {
      description: description.to_string(),
      is_supported,
      is_destructive,
      requires_table_rebuild,
      sql: format!("-- {description}"),
    }
  }

  #[test]
  fn rejects_unsupported_operation() {
    let diff = SchemaDiff {
      operations: vec![op("unsupported", false, false, false)],
    };
    let err = apply_policy(&diff, &PolicyConfig::default()).expect_err("must fail");
    assert!(err.to_string().contains("Unsupported operation"));
  }

  #[test]
  fn rejects_destructive_without_flag() {
    let diff = SchemaDiff {
      operations: vec![op("drop table", true, true, false)],
    };
    let err = apply_policy(&diff, &PolicyConfig::default()).expect_err("must fail");
    assert!(err.to_string().contains("allow-destructive"));
  }

  #[test]
  fn rejects_table_rebuild_without_flag() {
    let diff = SchemaDiff {
      operations: vec![op("rebuild", true, false, true)],
    };
    let err = apply_policy(&diff, &PolicyConfig::default()).expect_err("must fail");
    assert!(err.to_string().contains("allow-table-rebuild"));
  }

  #[test]
  fn accepts_when_flags_allow_operations() {
    let diff = SchemaDiff {
      operations: vec![
        op("create", true, false, false),
        op("drop", true, true, false),
        op("rebuild", true, false, true),
      ],
    };

    let policy = PolicyConfig {
      allow_destructive: true,
      allow_table_rebuild: true,
    };
    apply_policy(&diff, &policy).expect("policy should pass");
  }

  #[test]
  fn filter_safe_keeps_only_supported_non_destructive_non_rebuild() {
    let filtered = filter_safe_operations(vec![
      op("safe", true, false, false),
      op("unsupported", false, false, false),
      op("destructive", true, true, false),
      op("rebuild", true, false, true),
    ]);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].description, "safe");
  }
}
