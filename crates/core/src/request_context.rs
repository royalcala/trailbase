use std::sync::Arc;

tokio::task_local! {
  static CURRENT_ORG_CONN: Option<Arc<trailbase_sqlite::Connection>>;
}

pub async fn with_current_org_conn<T>(
  conn: Option<Arc<trailbase_sqlite::Connection>>,
  fut: impl std::future::Future<Output = T>,
) -> T {
  return CURRENT_ORG_CONN.scope(conn, fut).await;
}

pub fn current_org_conn() -> Option<Arc<trailbase_sqlite::Connection>> {
  return CURRENT_ORG_CONN.try_with(|conn| conn.clone()).ok().flatten();
}