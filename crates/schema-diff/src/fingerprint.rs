#![allow(clippy::needless_return)]

//! Schema fingerprinting: compute and store a stable content hash of the desired schema.

use sha2::{Digest, Sha256};

use crate::types::SchemaDiffError;

pub const FINGERPRINT_META_TABLE: &str = "_schema_diff_meta";
const FINGERPRINT_KEY: &str = "schema_fingerprint";

/// Compute a stable hex SHA-256 fingerprint of the desired schema content.
pub fn compute_schema_fingerprint(content: &str) -> String {
  let mut hasher = Sha256::new();
  // Normalize whitespace to make fingerprint stable against formatting.
  let normalized = content
    .lines()
    .map(str::trim)
    .filter(|l| !l.is_empty() && !l.starts_with("--"))
    .collect::<Vec<_>>()
    .join("\n");
  hasher.update(normalized.as_bytes());
  let result = hasher.finalize();
  return hex::encode(result);
}

/// Ensure meta table exists and load saved fingerprint (if any).
pub fn load_fingerprint(conn: &rusqlite::Connection) -> Result<Option<String>, SchemaDiffError> {
  conn
    .execute_batch(&format!(
      "CREATE TABLE IF NOT EXISTS {FINGERPRINT_META_TABLE} \
         (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT"
    ))
    .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

  let result: rusqlite::Result<Option<String>> = conn.query_row(
    &format!("SELECT value FROM {FINGERPRINT_META_TABLE} WHERE key = ?1"),
    rusqlite::params![FINGERPRINT_KEY],
    |row| row.get(0),
  );

  return match result {
    Ok(v) => Ok(v),
    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
    Err(e) => Err(SchemaDiffError::Parse(e.to_string())),
  };
}

/// Persist the current fingerprint after a successful apply.
pub fn store_fingerprint(
  conn: &rusqlite::Connection,
  fingerprint: &str,
) -> Result<(), SchemaDiffError> {
  conn
    .execute_batch(&format!(
      "CREATE TABLE IF NOT EXISTS {FINGERPRINT_META_TABLE} \
         (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT"
    ))
    .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

  conn
    .execute(
      &format!(
        "INSERT INTO {FINGERPRINT_META_TABLE} (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value"
      ),
      rusqlite::params![FINGERPRINT_KEY, fingerprint],
    )
    .map_err(|e| SchemaDiffError::Parse(e.to_string()))?;

  return Ok(());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fingerprint_ignores_comments_and_blank_lines() {
    let a = "-- comment\nCREATE TABLE users(id INTEGER PRIMARY KEY);\n";
    let b = "\nCREATE TABLE users(id INTEGER PRIMARY KEY);\n-- trailing\n";

    assert_eq!(compute_schema_fingerprint(a), compute_schema_fingerprint(b));
  }

  #[test]
  fn roundtrip_store_and_load() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");
    let fp = compute_schema_fingerprint("CREATE TABLE users(id INTEGER);");

    store_fingerprint(&conn, &fp).expect("store");
    let loaded = load_fingerprint(&conn).expect("load");
    assert_eq!(loaded.as_deref(), Some(fp.as_str()));
  }
}
