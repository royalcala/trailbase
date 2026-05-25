use axum::{
  extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
  http::request::Parts,
};

use crate::app_state::AppState;
use crate::auth::AuthError;
use crate::auth::user::User;
use crate::constants::HEADER_ORG_ID;
use crate::org::{OrgContext, resolve_org_context};

impl<S> FromRequestParts<S> for OrgContext
where
  AppState: FromRef<S>,
  S: Send + Sync,
{
  type Rejection = AuthError;

  async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
    let app_state = AppState::from_ref(state);
    let user = <User as OptionalFromRequestParts<S>>::from_request_parts(parts, state)
      .await?
      .ok_or(AuthError::Unauthorized)?;

    let org_id = parts.headers.get(HEADER_ORG_ID).and_then(|header| header.to_str().ok());

    resolve_org_context(&app_state, Some(&user), org_id)
      .await?
      .ok_or(AuthError::Unauthorized)
  }
}