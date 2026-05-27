use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

use crate::admin::AdminError as Error;
use crate::app_state::AppState;

#[derive(Debug, Deserialize)]
struct DbStatusCount {
  status: String,
  count: i64,
}

#[derive(Debug, Serialize, TS, ToSchema)]
pub struct QueueStatusCount {
  pub status: String,
  pub count: i64,
}

#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct QueueStatsResponse {
  pub total_jobs: i64,
  pub by_status: Vec<QueueStatusCount>,
}

#[utoipa::path(
  get,
  path = "/queue/stats",
  responses(
    (status = 200, description = "Queue statistics by status", body = QueueStatsResponse),
  ),
  tag = "admin-queue",
)]
pub async fn queue_stats_handler(
  State(state): State<AppState>,
) -> Result<Json<QueueStatsResponse>, Error> {
  let conn = state.queue_conn();

  let total_jobs: i64 = conn
    .read_query_row_get("SELECT COUNT(*) FROM _queue_job", (), 0)
    .await?
    .unwrap_or(0);

  let by_status = conn
    .read_query_values::<DbStatusCount>(
      "SELECT status, COUNT(*) as count FROM _queue_job GROUP BY status ORDER BY status",
      (),
    )
    .await?
    .into_iter()
    .map(|row| QueueStatusCount {
      status: row.status,
      count: row.count,
    })
    .collect::<Vec<_>>();

  return Ok(Json(QueueStatsResponse {
    total_jobs,
    by_status,
  }));
}
