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
pub fn apply_policy(
    diff: &SchemaDiff,
    policy: &PolicyConfig,
) -> Result<(), SchemaDiffError> {
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
