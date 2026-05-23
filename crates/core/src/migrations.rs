use itertools::Itertools;
use log::*;
use parking_lot::Mutex;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use trailbase_refinery::{Error as RefineryError, Migration};
use trailbase_schema_diff::{
  FINGERPRINT_META_TABLE, PolicyConfig, SchemaCheckPolicy, apply_policy, compute_diff,
  compute_schema_fingerprint,
  types::{LiveIndex, LiveSchema, LiveTable},
};
use walkdir::{DirEntry, WalkDir};

const MIGRATION_TABLE_NAME: &str = "_schema_history";

pub fn new_unique_migration_filename(suffix: &str) -> String {
  let timestamp = {
    // We use the timestamp as a version. We need to debounce it to avoid collisions.
    static PREV_TIMESTAMP: LazyLock<Mutex<i64>> = LazyLock::new(|| Mutex::new(0));

    let now = chrono::Utc::now().timestamp();
    let mut prev = PREV_TIMESTAMP.lock();

    if now > *prev {
      *prev = now;
      now
    } else {
      *prev += 1;
      *prev
    }
  };

  return format!("U{timestamp}__{suffix}.sql");
}

pub(crate) fn new_migration_runner(migrations: &[Migration]) -> trailbase_refinery::Runner {
  // NOTE: divergent migrations are migrations with the same version but a different name. That
  // said, `set_abort_divergent` is not a viable way for us to handle collisions (e.g. in tests),
  // since setting it to false, will prevent the migration from failing but divergent migrations
  // are quietly dropped on the floor and not applied. That's not ok.
  let mut runner = trailbase_refinery::Runner::new(migrations)
    .set_abort_divergent(false)
    .set_grouped(false);
  runner.set_migration_table_name(MIGRATION_TABLE_NAME);
  return runner;
}

/// Apply migrations: embedded and from `user_mgiations_path`.
///
/// Returns true, if V1 was applied, i.e. DB is initialized for the first time,
/// otherwise false.
pub(crate) async fn apply_main_migrations(
  conn: &trailbase_sqlite::Connection,
  base_migrations_path: Option<impl AsRef<Path>>,
) -> Result<bool, RefineryError> {
  #[cfg(feature = "pg")]
  let mut migrations = vec![load_embedded_migrations::<PgMainMigrations>()];

  #[cfg(not(feature = "pg"))]
  let mut migrations = vec![
    load_embedded_migrations::<BaseMigrations>(),
    load_embedded_migrations::<MainMigrations>(),
  ];

  if let Some(path) = base_migrations_path {
    // Ignore when `<traildepot>/migrations/main/` is missing.
    migrations.push(maybe_load_sql_migrations(path.as_ref().join("main"), true)?);

    // Legacy: all *.sql files in migrations.
    migrations.push(load_sql_migrations(path, false)?);
  }

  return apply_migrations_async("main", conn, migrations).await;
}

/// Declarative schema mode: compare desired schema file against live DB,
/// materialize diff as a versioned migration, and apply it through the
/// existing migration pipeline.
///
/// Returns true if any migration was applied.
pub async fn apply_declarative_schema(
  conn: &trailbase_sqlite::Connection,
  schema_path: impl AsRef<Path>,
  migrations_dir: impl AsRef<Path>,
  policy: &PolicyConfig,
  check_policy: &SchemaCheckPolicy,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
  let schema_path = schema_path.as_ref();
  let migrations_dir = migrations_dir.as_ref();

  if !schema_path.exists() {
    debug!(
      "Declarative schema file not found at {:?}, skipping.",
      schema_path
    );
    return Ok(false);
  }

  let desired_sql = std::fs::read_to_string(schema_path)?;
  let fingerprint = compute_schema_fingerprint(&desired_sql);

  conn
    .execute(
      format!(
        "CREATE TABLE IF NOT EXISTS {FINGERPRINT_META_TABLE}(\
          key TEXT PRIMARY KEY,\
          value TEXT NOT NULL\
        ) STRICT"
      ),
      (),
    )
    .await?;

  // Fast path: skip diff if schema has not changed.
  if *check_policy != SchemaCheckPolicy::Off {
    let saved_fp = conn
      .read_query_row_get::<String>(
        format!(
          "SELECT value FROM {meta_table} WHERE key = 'schema_fingerprint'",
          meta_table = FINGERPRINT_META_TABLE
        ),
        (),
        0,
      )
      .await;

    let saved_fp: Option<String> = match saved_fp {
      Ok(v) => v,
      Err(_) => None,
    };

    if saved_fp.as_deref() == Some(&fingerprint) {
      debug!("Schema fingerprint unchanged, skipping declarative diff.");
      return Ok(false);
    }
  }

  #[derive(serde::Deserialize)]
  struct TableRow {
    name: String,
    sql: Option<String>,
  }

  #[derive(serde::Deserialize)]
  struct IndexRow {
    name: String,
    tbl_name: String,
    sql: String,
  }

  let table_rows: Vec<TableRow> = conn
    .read_query_values(
      "SELECT name, sql FROM sqlite_schema \
       WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
       ORDER BY name",
      (),
    )
    .await?;

  let index_rows: Vec<IndexRow> = conn
    .read_query_values(
      "SELECT name, tbl_name, sql FROM sqlite_schema \
       WHERE type = 'index' AND sql IS NOT NULL \
       ORDER BY name",
      (),
    )
    .await?;

  let live = LiveSchema {
    tables: table_rows
      .into_iter()
      .filter(|t| {
        !t.name.starts_with("__")
          && t.name != "_schema_history"
          && t.name != FINGERPRINT_META_TABLE
          && !t.name.starts_with("sqlite_")
      })
      .map(|t| LiveTable {
        name: t.name,
        sql: t.sql.unwrap_or_default(),
      })
      .collect(),
    indexes: index_rows
      .into_iter()
      .map(|i| LiveIndex {
        name: i.name,
        table_name: i.tbl_name,
        sql: i.sql,
      })
      .collect(),
  };

  let diff_result = compute_diff(&desired_sql, &live)?;

  if diff_result.is_empty() {
    info!("Declarative schema: no changes detected.");
    // Update fingerprint even when no changes, so we skip on next startup.
    conn
      .execute(
        format!(
          "INSERT INTO {meta_table}(key, value) VALUES('schema_fingerprint', ?1) \
           ON CONFLICT(key) DO UPDATE SET value = excluded.value",
          meta_table = FINGERPRINT_META_TABLE
        ),
        (fingerprint.clone(),),
      )
      .await?;
    return Ok(false);
  }

  // Apply policy.
  if let Err(e) = apply_policy(&diff_result, policy) {
    if *check_policy == SchemaCheckPolicy::Strict {
      return Err(format!("Declarative schema policy violation: {e}").into());
    }
    warn!("Declarative schema policy violation (non-strict): {e}");
  }

  let sql = diff_result.to_sql();
  if sql.trim().is_empty() {
    return Ok(false);
  }

  // Write migration file.
  let filename = new_unique_migration_filename("schema_sync");
  let stem = Path::new(&filename)
    .file_stem()
    .ok_or("bad filename")?
    .to_string_lossy()
    .to_string();

  let migration_path = migrations_dir.join("main");
  std::fs::create_dir_all(&migration_path)?;
  let file_path = migration_path.join(&filename);

  {
    let mut file = std::fs::File::create_new(&file_path)?;
    file.write_all(sql.as_bytes())?;
  }
  info!("Declarative schema: wrote migration {:?}", file_path);

  // Apply the new migration.
  let migration = Migration::unapplied(&stem, &sql)?;
  let runner = new_migration_runner(&[migration]).set_abort_missing(false);
  let mut conn_clone = conn.clone();
  runner.run_async(&mut conn_clone).await?;

  // Store updated fingerprint.
  conn
    .execute(
      format!(
        "INSERT INTO {meta_table}(key, value) VALUES('schema_fingerprint', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        meta_table = FINGERPRINT_META_TABLE
      ),
      (fingerprint.clone(),),
    )
    .await?;

  info!("Declarative schema: applied migration '{filename}'.");
  return Ok(true);
}

// Base migrations contains things like file deletions table shared across main and user DBs.
pub(crate) fn apply_base_migrations(
  conn: &mut rusqlite::Connection,
  base_migrations_path: Option<impl AsRef<Path>>,
  db: &str,
) -> Result<bool, RefineryError> {
  let mut migrations = vec![load_embedded_migrations::<BaseMigrations>()];
  // TODO: Should we handle load_sql_migrations error?
  if let Some(path) = base_migrations_path {
    // Ignore when `<traildepot>/migrations/main/` is missing.
    migrations.push(maybe_load_sql_migrations(path.as_ref().join(db), true)?);
  }
  return apply_migrations(db, conn, migrations);
}

pub(crate) fn apply_logs_migrations(
  logs_conn: &mut rusqlite::Connection,
) -> Result<(), RefineryError> {
  apply_migrations(
    "logs",
    logs_conn,
    vec![load_embedded_migrations::<LogsMigrations>()],
  )?;
  return Ok(());
}

pub(crate) fn apply_session_migrations(
  logs_conn: &mut rusqlite::Connection,
) -> Result<(), RefineryError> {
  apply_migrations(
    "session",
    logs_conn,
    vec![load_embedded_migrations::<SessionMigrations>()],
  )?;
  return Ok(());
}

pub(crate) fn apply_migrations(
  name: &str,
  conn: &mut rusqlite::Connection,
  migrations: Vec<Vec<Migration>>,
) -> Result<bool, RefineryError> {
  let migrations: Vec<Migration> = migrations.into_iter().flatten().sorted().collect();

  let runner = new_migration_runner(&migrations);
  let report = runner.run(conn).map_err(|err| {
    error!("Migration error for '{name}' DB: {err}");
    return err;
  })?;

  let applied_migrations = report.applied_migrations();
  log_migrations(name, applied_migrations);

  // If we applied migration v1 we can be sure this is a fresh database.
  let new_db = applied_migrations.iter().any(|m| m.version() == 1);

  return Ok(new_db);
}

pub(crate) async fn apply_migrations_async(
  name: &str,
  conn: &trailbase_sqlite::Connection,
  migrations: Vec<Vec<Migration>>,
) -> Result<bool, RefineryError> {
  let migrations: Vec<Migration> = migrations.into_iter().flatten().sorted().collect();

  let mut conn = conn.clone();
  let runner = new_migration_runner(&migrations);
  let report = runner.run_async(&mut conn).await.map_err(|err| {
    error!("Migration error for '{name}' DB: {err}");
    return err;
  })?;

  let applied_migrations = report.applied_migrations();
  log_migrations(name, applied_migrations);

  // If we applied migration v1 we can be sure this is a fresh database.
  let new_db = applied_migrations.iter().any(|m| m.version() == 1);

  return Ok(new_db);
}

fn log_migrations(db_name: &str, migrations: &[Migration]) {
  fn name(migration: &Migration) -> String {
    return format!(
      "{prefix}{version}__{name}",
      prefix = migration.prefix(),
      version = migration.version(),
      name = migration.name(),
    );
  }

  if !migrations.is_empty() {
    if !cfg!(test) {
      info!(
        "Successfully applied migrations for '{db_name}' DB: {names}",
        names = migrations
          .iter()
          .map(|m| format!("'{}'", name(m)))
          .join(", ")
      )
    }

    for migration in migrations {
      trace!(
        "Migration details for '{name}':\n{sql}",
        name = name(migration),
        sql = migration.sql().unwrap_or("<EMPTY>"),
      );
    }
  }
}

// Just like `load_sql_migrations` but ignores missing paths.
fn maybe_load_sql_migrations(
  location: impl AsRef<Path>,
  recursive: bool,
) -> Result<Vec<Migration>, RefineryError> {
  return match load_sql_migrations(location, recursive) {
    Err(err)
      if matches!(
        err.kind(),
        trailbase_refinery::error::Kind::InvalidMigrationPath(_, _)
      ) =>
    {
      return Ok(vec![]);
    }
    resp => resp,
  };
}

/// Loads SQL migrations from a path. This enables dynamic migration discovery, as opposed to
/// embedding. The resulting collection is ordered by version.
fn load_sql_migrations(
  location: impl AsRef<Path>,
  recursive: bool,
) -> Result<Vec<Migration>, RefineryError> {
  use trailbase_refinery::{Error, error::Kind};

  let mut migrations = find_migration_files(location, recursive)?
    .map(|path| -> Result<Migration, Error> {
      let sql = std::fs::read_to_string(path.as_path()).map_err(|e| {
        let path = path.to_owned();
        let kind = match e.kind() {
          std::io::ErrorKind::NotFound => Kind::InvalidMigrationPath(path, e),
          _ => Kind::InvalidMigrationFile(path, e),
        };

        Error::new(kind, None)
      })?;

      let filename = path
        .file_stem()
        .and_then(|file| file.to_os_string().into_string().ok())
        .ok_or_else(|| RefineryError::new(Kind::InvalidName, None))?;

      return Migration::unapplied(&filename, &sql);
    })
    .collect::<Result<Vec<Migration>, Error>>()?;

  migrations.sort();

  return Ok(migrations);
}

const STEM_RE: &str = r"^([U|V])(\d+(?:\.\d+)?)__(\w+)";
static SQL_FILE_RE: LazyLock<regex::Regex> =
  LazyLock::new(|| regex::Regex::new(&format!(r"{STEM_RE}\.sql$")).expect("const"));

/// find migrations on file system recursively across directories given a location and
/// [MigrationType]
fn find_migration_files(
  location: impl AsRef<Path>,
  recursive: bool,
) -> Result<impl Iterator<Item = PathBuf>, RefineryError> {
  use trailbase_refinery::error::Kind;

  let location: &Path = location.as_ref();
  let location = location.canonicalize().map_err(|err| {
    RefineryError::new(
      Kind::InvalidMigrationPath(location.to_path_buf(), err),
      None,
    )
  })?;

  let max_depth = if recursive {
    usize::MAX
  } else {
    // Don't load recursively.
    1
  };

  let file_paths = WalkDir::new(location)
    .max_depth(max_depth)
    .into_iter()
    .filter_map(Result::ok)
    .map(DirEntry::into_path)
    // filter by migration file regex
    .filter(|path|-> bool {
    return match path.file_name().and_then(OsStr::to_str) {
      Some(_) if path.is_dir() => false,
      Some(file_name) if SQL_FILE_RE.is_match(file_name) => true,
      Some(file_name) => {
        log::warn!(
          "File \"{file_name}\" does not adhere to the migration naming convention. Migrations must be named in the format [U|V]{{1}}__{{2}}.sql or [U|V]{{1}}__{{2}}.rs, where {{1}} represents the migration version and {{2}} the name."
        );
        false
      }
      None => false,
    };
  });

  Ok(file_paths)
}

fn load_embedded_migrations<T: rust_embed::RustEmbed>() -> Vec<Migration> {
  return T::iter()
    .map(|filename| {
      return Migration::unapplied(
        &filename,
        &String::from_utf8_lossy(&T::get(&filename).expect("startup").data),
      )
      .expect("startup");
    })
    .collect();
}

// Base migrations contains things like file deletions table shared across main and user DBs.
#[derive(Clone, rust_embed::RustEmbed)]
#[folder = "migrations/base"]
struct BaseMigrations;

#[derive(Clone, rust_embed::RustEmbed)]
#[folder = "migrations/main"]
struct MainMigrations;

#[cfg(feature = "pg")]
#[derive(Clone, rust_embed::RustEmbed)]
#[folder = "migrations/pg_main"]
struct PgMainMigrations;

#[derive(Clone, rust_embed::RustEmbed)]
#[folder = "migrations/logs"]
struct LogsMigrations;

#[derive(Clone, rust_embed::RustEmbed)]
#[folder = "migrations/session"]
struct SessionMigrations;

#[cfg(test)]
mod tests {
  use super::*;

  use std::path::Path;
  use trailbase_sqlite::Connection;

  #[test]
  fn test_load_sql_migrations() {
    assert!(load_sql_migrations("__non-existent-path__", true).is_err());
    assert!(
      maybe_load_sql_migrations("__non-existent-path__", true)
        .unwrap()
        .is_empty()
    );
  }

  #[tokio::test]
  async fn test_schema_invariants_still_hold_after_migrations() {
    let state = crate::app_state::test_state(None).await.unwrap();

    async fn exists(conn: &Connection, name: &str, schema_type: &str) -> bool {
      return conn.read_query_row_get(
          format!(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = '{schema_type}' AND name = '{name}')"
          ),
          (),
          0,
        )
        .await.unwrap().unwrap();
    }

    async fn index_exists(conn: &Connection, name: &str) -> bool {
      return exists(conn, name, "index").await;
    }

    async fn trigger_exists(conn: &Connection, name: &str) -> bool {
      return exists(conn, name, "trigger").await;
    }

    let conn = state.conn();

    // QUESTION: Should we push something like this down into startup to assert that
    // user-provided migrations don't break TB's expectations, e.g. they modified the
    // `_user` TABLE and forgot to put an index back into place.
    assert!(index_exists(conn, "__user__email_index").await);
    assert!(index_exists(conn, "__user__provider_ids_index").await);

    assert!(trigger_exists(conn, "__user__updated_trigger").await);
    assert!(trigger_exists(conn, "__user_avatar__updated_trigger").await);
  }

  fn make_temp_test_dir(suffix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .expect("clock")
      .as_nanos();

    let dir = std::env::temp_dir().join(format!(
      "trailbase-declarative-{suffix}-{}-{nanos}",
      std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
  }

  async fn table_exists(conn: &Connection, table: &str) -> bool {
    conn
      .read_query_row_get::<bool>(
        format!(
          "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = '{table}')"
        ),
        (),
        0,
      )
      .await
      .expect("query")
      .expect("value")
  }

  fn schema_sync_migration_count(migrations_dir: &Path) -> usize {
    let main_dir = migrations_dir.join("main");
    if !main_dir.exists() {
      return 0;
    }

    std::fs::read_dir(main_dir)
      .expect("read dir")
      .flatten()
      .filter(|entry| {
        entry
          .file_name()
          .to_string_lossy()
          .ends_with("__schema_sync.sql")
      })
      .count()
  }

  #[tokio::test]
  async fn declarative_apply_is_idempotent_with_fingerprint() {
    let conn = Connection::open_in_memory().expect("conn");
    let temp_dir = make_temp_test_dir("idempotent");
    let schema_path = temp_dir.join("main.sql");

    std::fs::write(
      &schema_path,
      "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);",
    )
    .expect("write schema");

    let policy = trailbase_schema_diff::PolicyConfig {
      allow_destructive: false,
      allow_table_rebuild: false,
    };

    let first = apply_declarative_schema(
      &conn,
      &schema_path,
      &temp_dir,
      &policy,
      &trailbase_schema_diff::SchemaCheckPolicy::On,
    )
    .await
    .expect("first apply");
    assert!(first);
    assert!(table_exists(&conn, "users").await);
    assert_eq!(schema_sync_migration_count(&temp_dir), 1);

    let second = apply_declarative_schema(
      &conn,
      &schema_path,
      &temp_dir,
      &policy,
      &trailbase_schema_diff::SchemaCheckPolicy::On,
    )
    .await
    .expect("second apply");
    assert!(!second);
    assert_eq!(schema_sync_migration_count(&temp_dir), 1);

    let _ = std::fs::remove_dir_all(temp_dir);
  }

  #[tokio::test]
  async fn declarative_strict_blocks_destructive_without_policy_flag() {
    let conn = Connection::open_in_memory().expect("conn");
    let temp_dir = make_temp_test_dir("strict-blocks");
    let schema_path = temp_dir.join("main.sql");

    conn
      .execute(
        "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);",
        (),
      )
      .await
      .expect("create users");
    conn
      .execute("CREATE TABLE posts(id INTEGER PRIMARY KEY, title TEXT);", ())
      .await
      .expect("create posts");

    std::fs::write(
      &schema_path,
      "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);",
    )
    .expect("write schema");

    let policy = trailbase_schema_diff::PolicyConfig {
      allow_destructive: false,
      allow_table_rebuild: false,
    };

    let result = apply_declarative_schema(
      &conn,
      &schema_path,
      &temp_dir,
      &policy,
      &trailbase_schema_diff::SchemaCheckPolicy::Strict,
    )
    .await;

    assert!(result.is_err());
    assert!(table_exists(&conn, "posts").await);
    assert_eq!(schema_sync_migration_count(&temp_dir), 0);

    let _ = std::fs::remove_dir_all(temp_dir);
  }

  #[tokio::test]
  async fn declarative_strict_allows_destructive_when_enabled() {
    let conn = Connection::open_in_memory().expect("conn");
    let temp_dir = make_temp_test_dir("strict-allowed");
    let schema_path = temp_dir.join("main.sql");

    conn
      .execute(
        "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);",
        (),
      )
      .await
      .expect("create users");
    conn
      .execute("CREATE TABLE posts(id INTEGER PRIMARY KEY, title TEXT);", ())
      .await
      .expect("create posts");

    std::fs::write(
      &schema_path,
      "CREATE TABLE users(id INTEGER PRIMARY KEY, email TEXT);",
    )
    .expect("write schema");

    let policy = trailbase_schema_diff::PolicyConfig {
      allow_destructive: true,
      allow_table_rebuild: false,
    };

    let applied = apply_declarative_schema(
      &conn,
      &schema_path,
      &temp_dir,
      &policy,
      &trailbase_schema_diff::SchemaCheckPolicy::Strict,
    )
    .await
    .expect("apply");

    assert!(applied);
    assert!(!table_exists(&conn, "posts").await);
    assert_eq!(schema_sync_migration_count(&temp_dir), 1);

    let _ = std::fs::remove_dir_all(temp_dir);
  }
}
