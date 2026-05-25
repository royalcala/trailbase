use const_format::formatcp;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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