#![allow(clippy::needless_return)]

//! Declarative SQLite schema diff engine for TrailBase.
//!
//! This crate compares a desired schema (from a `.sql` file) against the live
//! database state and produces an ordered list of SQL statements needed to
//! reconcile them.  The generated statements are meant to be persisted as a
//! regular `U<timestamp>__schema_sync.sql` migration and applied through
//! TrailBase's existing migration pipeline.

mod diff;
mod fingerprint;
mod introspect;
mod policy;
pub mod types;

pub use diff::compute_diff;
pub use fingerprint::FINGERPRINT_META_TABLE;
pub use fingerprint::{compute_schema_fingerprint, load_fingerprint, store_fingerprint};
pub use introspect::introspect_schema;
pub use policy::{PolicyConfig, apply_policy};
pub use types::{DiffOperation, SchemaCheckPolicy, SchemaDiff, SchemaMode};
