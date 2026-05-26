use const_format::formatcp;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task::JoinSet;
use trailbase_sqlite::params;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::auth::AuthError;
use crate::auth::user::User;
use crate::constants::{ORG_MEMBERSHIP_TABLE, ORG_TABLE};
use crate::connection::BuildOptions;
use crate::util::{b64_to_uuid, uuid_to_b64};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OrgContext {
  pub org_id: String,
  pub role: Option<String>,
  pub conn: Arc<trailbase_sqlite::Connection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[ts(export)]
pub struct OrgSummary {
  pub id: String,
  pub name: String,
  pub slug: String,
  pub role: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DbOrgSummary {
  pub id: [u8; 16],
  pub name: String,
  pub slug: String,
  pub role: String,
}

fn validate_org_id(org_id: &str) -> Result<(), AuthError> {
  if org_id.is_empty()
    || org_id.len() > 80
    || !org_id
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '=')
  {
    return Err(AuthError::BadRequest("invalid org id"));
  }

  return Ok(());
}

pub fn org_db_name(org_id: &str) -> Result<String, AuthError> {
  validate_org_id(org_id)?;
  return Ok(format!("org_{org_id}"));
}

async fn get_org_membership_role(
  state: &AppState,
  user_id: &Uuid,
  org_id: &str,
) -> Result<Option<String>, AuthError> {
  let org_uuid = b64_to_uuid(org_id).map_err(|_| AuthError::BadRequest("invalid org id"))?;
  const QUERY: &str = formatcp!(
    r#"SELECT role FROM "{ORG_MEMBERSHIP_TABLE}" WHERE org = $1 AND user = $2"#
  );

  return state
    .conn()
    .read_query_row_get::<String>(QUERY, params!(org_uuid.into_bytes(), user_id.into_bytes()), 0)
    .await
    .map_err(|_| AuthError::Internal("failed to read membership".into()));
}

pub async fn default_org_id_for_user(state: &AppState, user_id: &Uuid) -> Result<Option<String>, AuthError> {
  const QUERY: &str = formatcp!(
    r#"SELECT o.slug FROM "{ORG_TABLE}" o
       INNER JOIN "{ORG_MEMBERSHIP_TABLE}" m ON m.org = o.id
       WHERE m.user = $1
       ORDER BY o.created ASC
       LIMIT 1"#
  );

  return state
    .conn()
    .read_query_row_get::<String>(QUERY, params!(user_id.into_bytes()), 0)
    .await
    .map_err(|_| AuthError::Internal("failed to resolve default org".into()));
}

pub async fn list_orgs_for_user(state: &AppState, user_id: &Uuid) -> Result<Vec<OrgSummary>, AuthError> {
  const QUERY: &str = formatcp!(
    r#"SELECT o.id, o.name, o.slug, m.role
       FROM "{ORG_TABLE}" o
       INNER JOIN "{ORG_MEMBERSHIP_TABLE}" m ON m.org = o.id
       WHERE m.user = $1
       ORDER BY o.created ASC"#
  );

  let rows: Vec<DbOrgSummary> = state
    .conn()
    .read_query_values(QUERY, params!(user_id.into_bytes()))
    .await
    .map_err(|_| AuthError::Internal("failed to list orgs".into()))?;

  return Ok(rows
    .into_iter()
    .map(|row| OrgSummary {
      id: uuid_to_b64(&Uuid::from_bytes(row.id)),
      name: row.name,
      slug: row.slug,
      role: row.role,
    })
    .collect());
}

pub async fn create_org_for_user(
  state: &AppState,
  user: &User,
  name: &str,
) -> Result<OrgSummary, AuthError> {
  let name = name.trim();
  if name.is_empty() {
    return Err(AuthError::BadRequest("org name required"));
  }

  let org_id = Uuid::now_v7();
  let slug = uuid_to_b64(&org_id);
  let user_id = user.uuid;

  const INSERT_ORG: &str = formatcp!(
    r#"INSERT INTO "{ORG_TABLE}" (id, name, slug) VALUES ($1, $2, $3)"#
  );
  state
    .conn()
    .execute(INSERT_ORG, params!(org_id.into_bytes(), name.to_string(), slug.clone()))
    .await
    .map_err(|_| AuthError::Internal("failed to create org".into()))?;

  const INSERT_MEMBERSHIP: &str = formatcp!(
    r#"INSERT INTO "{ORG_MEMBERSHIP_TABLE}" (org, user, role) VALUES ($1, $2, $3)"#
  );
  state
    .conn()
    .execute(
      INSERT_MEMBERSHIP,
      params!(org_id.into_bytes(), user_id.into_bytes(), "owner".to_string()),
    )
    .await
    .map_err(|_| AuthError::Internal("failed to create org membership".into()))?;

  return Ok(OrgSummary {
    id: slug.clone(),
    name: name.to_string(),
    slug,
    role: "owner".to_string(),
  });
}

pub async fn resolve_org_context(
  state: &AppState,
  user: Option<&User>,
  org_id_hint: Option<&str>,
) -> Result<Option<OrgContext>, AuthError> {
  let Some(user) = user else {
    return Ok(None);
  };

  let org_id = if let Some(org_id) = org_id_hint {
    Some(org_id.to_string())
  } else {
    user.org_id.clone()
  };

  let Some(org_id) = org_id else {
    return Ok(None);
  };

  validate_org_id(&org_id)?;

  let role = get_org_membership_role(state, &user.uuid, &org_id).await?;
  let Some(role) = role else {
    return Err(AuthError::Forbidden);
  };

  let conn = state
    .connection_manager()
    .get_entry(BuildOptions {
      is_main: true,
      attached_databases: Some([org_db_name(&org_id)?].into()),
      ..Default::default()
    })
    .await
    .map_err(|_| AuthError::Internal("failed to open org database".into()))?
    .connection;

  return Ok(Some(OrgContext {
    org_id,
    role: Some(role),
    conn,
  }));
}

/// On server startup, iterate all known orgs and apply any pending migrations to their
/// individual DBs. This ensures that after a deploy all org DBs are in sync before any
/// traffic is served. The lazy path in `get_entry` (via `resolve_org_context`) acts as
/// the safety net for orgs created after startup.
///
/// Migrations are parallelized with tokio tasks. The fingerprint check inside
/// `apply_declarative_schema` makes this O(1) per org when nothing changed.
pub async fn migrate_all_org_dbs(state: &AppState) -> Result<(), AuthError> {
  const QUERY: &str = formatcp!(r#"SELECT slug FROM "{ORG_TABLE}" ORDER BY created ASC"#);

  let slugs: Vec<String> = state
    .conn()
    .read_query_values::<String>(QUERY, ())
    .await
    .map_err(|_| AuthError::Internal("failed to list org slugs".into()))?;

  if slugs.is_empty() {
    return Ok(());
  }

  info!("Running startup migration sweep for {} org(s)...", slugs.len());

  let connection_manager = state.connection_manager();
  let mut tasks = JoinSet::new();

  for slug in slugs {
    let cm = connection_manager.clone();
    tasks.spawn(async move {
      let db_name = format!("org_{slug}");
      let result = cm
        .get_entry(BuildOptions {
          is_main: true,
          attached_databases: Some([db_name].into()),
          ..Default::default()
        })
        .await;

      if let Err(err) = result {
        warn!("Startup migration failed for org '{slug}': {err}");
        return Err(slug);
      }

      return Ok(());
    });
  }

  let mut failed = 0usize;
  while let Some(result) = tasks.join_next().await {
    if result.unwrap_or(Err(String::new())).is_err() {
      failed += 1;
    }
  }

  if failed > 0 {
    warn!("Startup org migration sweep: {failed} org(s) failed. Check logs above.");
  } else {
    info!("Startup org migration sweep complete.");
  }

  return Ok(());
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app_state::test_state;
  use crate::migrations::apply_declarative_schema;
  use trailbase_schema_diff::{PolicyConfig, SchemaCheckPolicy};

  fn schema_sync_migration_count(migrations_dir: &std::path::Path) -> usize {
    let orgs_dir = migrations_dir.join("orgs");
    if !orgs_dir.exists() {
      return 0;
    }

    std::fs::read_dir(orgs_dir)
      .expect("read dir")
      .flatten()
      .map(|entry| entry.path())
      .filter(|path| path.is_dir())
      .map(|dir| {
        std::fs::read_dir(dir)
          .expect("read dir")
          .flatten()
          .filter(|entry| {
            entry
              .file_name()
              .to_string_lossy()
              .ends_with("__schema_sync.sql")
          })
          .count()
      })
      .sum()
  }

  #[tokio::test]
  async fn org_schema_sync_applies_on_first_attach() {
    let state = test_state(None).await.expect("state");
    let root = state.data_dir().root().to_path_buf();
    std::fs::create_dir_all(root.join("data")).expect("data dir");
    std::fs::create_dir_all(root.join("migrations")).expect("migrations dir");
    let schema_dir = root.join("schema");
    let schema_path = schema_dir.join("org.sql");
    std::fs::create_dir_all(&schema_dir).expect("schema dir");
    std::fs::write(
      &schema_path,
      r#"
CREATE TABLE _file_deletions (
  id                           INTEGER PRIMARY KEY NOT NULL,
  deleted                      INTEGER NOT NULL DEFAULT (UNIXEPOCH()),
  attempts                     INTEGER NOT NULL DEFAULT 0,
  errors                       TEXT,
  table_name                   TEXT NOT NULL,
  record_rowid                 INTEGER NOT NULL,
  column_name                  TEXT NOT NULL,
  json                         TEXT NOT NULL
) STRICT;

CREATE TABLE org_items(id INTEGER PRIMARY KEY, name TEXT);
"#,
    )
    .expect("write schema");

    let org_id = Uuid::now_v7();
    let slug = uuid_to_b64(&org_id);
    let schema_name = org_db_name(&slug).expect("org db name");
    let org_db_path = root.join("data").join(format!("{schema_name}.db"));
    let conn = trailbase_sqlite::Connection::with_opts(
      || rusqlite::Connection::open(&org_db_path),
      Default::default(),
    )
    .expect("open org db");

    let policy = PolicyConfig {
      allow_destructive: false,
      allow_table_rebuild: false,
    };
    let migration_output_dir = root.join("migrations").join("orgs").join(&schema_name);
    let applied = apply_declarative_schema(
      &conn,
      &schema_path,
      &migration_output_dir,
      &policy,
      &SchemaCheckPolicy::On,
    )
    .await
    .expect("apply schema");

    assert!(applied);

    let exists = conn
      .read_query_row_get::<bool>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='org_items')",
        (),
        0,
      )
      .await
      .expect("query")
      .expect("value");

    assert!(exists);

    let migrations_dir = root.join("migrations");
    assert!(schema_sync_migration_count(&migrations_dir) >= 1);

    let _ = std::fs::remove_dir_all(root);
  }

  #[tokio::test]
  async fn startup_sweep_applies_existing_org_dbs() {
    let state = test_state(None).await.expect("state");
    let root = state.data_dir().root().to_path_buf();
    std::fs::create_dir_all(root.join("data")).expect("data dir");
    std::fs::create_dir_all(root.join("migrations")).expect("migrations dir");
    let schema_dir = root.join("schema");
    let schema_path = schema_dir.join("org.sql");
    std::fs::create_dir_all(&schema_dir).expect("schema dir");
    std::fs::write(
      &schema_path,
      r#"
CREATE TABLE _file_deletions (
  id                           INTEGER PRIMARY KEY NOT NULL,
  deleted                      INTEGER NOT NULL DEFAULT (UNIXEPOCH()),
  attempts                     INTEGER NOT NULL DEFAULT 0,
  errors                       TEXT,
  table_name                   TEXT NOT NULL,
  record_rowid                 INTEGER NOT NULL,
  column_name                  TEXT NOT NULL,
  json                         TEXT NOT NULL
) STRICT;

CREATE TABLE org_events(id INTEGER PRIMARY KEY, name TEXT);
"#,
    )
    .expect("write schema");

    let org_id = Uuid::now_v7();
    let slug = uuid_to_b64(&org_id);
    state
      .conn()
      .execute(
        format!(r#"INSERT INTO "{ORG_TABLE}" (id, name, slug) VALUES ($1, $2, $3)"#),
        params!(org_id.into_bytes(), "Org".to_string(), slug.clone()),
      )
      .await
      .expect("insert org");

    migrate_all_org_dbs(&state).await.expect("startup sweep");

    let schema_name = org_db_name(&slug).expect("org db name");
    let org_db_path = root.join("data").join(format!("{schema_name}.db"));
    assert!(org_db_path.exists());

    let migration_dir = root.join("migrations").join("orgs").join(&schema_name);
    assert!(migration_dir.exists());
    assert!(schema_sync_migration_count(&root.join("migrations")) >= 1);

    let _ = std::fs::remove_dir_all(root);
  }
}