import { adminFetch } from "@/lib/fetch";

import type { ListQueueJobsResponse } from "@bindings/ListQueueJobsResponse";
import type { QueueStatsResponse } from "@bindings/QueueStatsResponse";

export async function fetchQueueJobs(): Promise<ListQueueJobsResponse> {
  const response = await adminFetch("/queue/jobs");
  return await response.json();
}

export async function fetchQueueStats(): Promise<QueueStatsResponse> {
  const response = await adminFetch("/queue/stats");
  return await response.json();
}
