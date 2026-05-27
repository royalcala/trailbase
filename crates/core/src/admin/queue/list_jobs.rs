use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::admin::AdminError as Error;
use crate::app_state::AppState;

#[derive(Debug, Deserialize)]
struct DbQueueJob {
  id: Vec<u8>,
  queue: String,
  job_type: String,
  status: String,
  priority: i64,
  attempts: i64,
  max_attempts: i64,
  run_at: f64,
  lease_until: Option<f64>,
  worker_id: Option<String>,
  org_id: Option<Vec<u8>>,
  last_error: Option<String>,
  created: f64,
  updated: f64,
}

#[derive(Debug, Serialize, TS, ToSchema)]
pub struct QueueJob {
  pub id: String,
  pub queue: String,
  pub job_type: String,
  pub status: String,
  pub priority: i64,
  pub attempts: i64,
  pub max_attempts: i64,
  pub run_at: f64,
  pub lease_until: Option<f64>,
  pub worker_id: Option<String>,
  pub org_id: Option<String>,
  pub last_error: Option<String>,
  pub created: f64,
  pub updated: f64,
}

#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct ListQueueJobsResponse {
  pub total_row_count: i64,
  pub jobs: Vec<QueueJob>,
}

fn encode_uuid_blob(bytes: &[u8]) -> String {
  if let Ok(arr) = <[u8; 16]>::try_from(bytes)
    && let Ok(uuid) = Uuid::from_slice(&arr)
  {
    return uuid.to_string();
  }

  let mut out = String::with_capacity(bytes.len() * 2);
  for b in bytes {
    use std::fmt::Write as _;
    let _ = write!(&mut out, "{b:02x}");
  }
  return out;
}

#[utoipa::path(
  get,
  path = "/queue/jobs",
  responses(
    (status = 200, description = "List queue jobs", body = ListQueueJobsResponse),
  ),
  tag = "admin-queue",
)]
pub async fn list_queue_jobs_handler(
  State(state): State<AppState>,
) -> Result<Json<ListQueueJobsResponse>, Error> {
  let conn = state.queue_conn();

  let total_row_count: i64 = conn
    .read_query_row_get("SELECT COUNT(*) FROM _queue_job", (), 0)
    .await?
    .unwrap_or(0);

  let jobs = conn
    .read_query_values::<DbQueueJob>(
      r#"
      SELECT
        id,
        queue,
        job_type,
        status,
        priority,
        attempts,
        max_attempts,
        run_at,
        lease_until,
        worker_id,
        org_id,
        last_error,
        created,
        updated
      FROM _queue_job
      ORDER BY created DESC
      LIMIT 200
      "#,
      (),
    )
    .await?
    .into_iter()
    .map(|row| QueueJob {
      id: encode_uuid_blob(&row.id),
      queue: row.queue,
      job_type: row.job_type,
      status: row.status,
      priority: row.priority,
      attempts: row.attempts,
      max_attempts: row.max_attempts,
      run_at: row.run_at,
      lease_until: row.lease_until,
      worker_id: row.worker_id,
      org_id: row.org_id.map(|b| encode_uuid_blob(&b)),
      last_error: row.last_error,
      created: row.created,
      updated: row.updated,
    })
    .collect::<Vec<_>>();

  return Ok(Json(ListQueueJobsResponse {
    total_row_count,
    jobs,
  }));
}
