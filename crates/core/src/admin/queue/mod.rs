pub mod list_jobs;
pub mod stats;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
  tags(
    (name = "admin-queue", description = "Admin queue management APIs"),
  ),
  paths(
    list_jobs::list_queue_jobs_handler,
    stats::queue_stats_handler,
  ),
  components(
    schemas(
      list_jobs::QueueJob,
      list_jobs::ListQueueJobsResponse,
      stats::QueueStatusCount,
      stats::QueueStatsResponse,
    ),
  ),
)]
pub(crate) struct AdminQueueApi;
