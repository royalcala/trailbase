use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

use crate::app_state::AppState;
use crate::auth::{AuthError, User};
use crate::org::{OrgSummary, create_org_for_user, list_orgs_for_user};

#[derive(Debug, Clone, Deserialize, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateOrgRequest {
  pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateOrgResponse {
  pub org: OrgSummary,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct ListOrgsResponse {
  pub orgs: Vec<OrgSummary>,
}

#[utoipa::path(
  get,
  path = "/orgs",
  tag = "auth",
  responses((status = 200, body = ListOrgsResponse))
)]
pub async fn list_orgs_handler(
  State(state): State<AppState>,
  user: User,
) -> Result<Json<ListOrgsResponse>, AuthError> {
  let orgs = list_orgs_for_user(&state, &user.uuid).await?;
  return Ok(Json(ListOrgsResponse { orgs }));
}

#[utoipa::path(
  post,
  path = "/orgs",
  tag = "auth",
  request_body = CreateOrgRequest,
  responses((status = 200, body = CreateOrgResponse))
)]
pub async fn create_org_handler(
  State(state): State<AppState>,
  user: User,
  Json(request): Json<CreateOrgRequest>,
) -> Result<Json<CreateOrgResponse>, AuthError> {
  let org = create_org_for_user(&state, &user, &request.name).await?;
  return Ok(Json(CreateOrgResponse { org }));
}
