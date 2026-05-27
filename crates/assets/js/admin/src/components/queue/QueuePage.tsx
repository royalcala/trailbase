import { createMemo, For, Show } from "solid-js";
import type { ColumnDef } from "@tanstack/solid-table";
import { useQuery } from "@tanstack/solid-query";

import { Header } from "@/components/Header";
import { Table, buildTable } from "@/components/Table";

import { fetchQueueJobs, fetchQueueStats } from "@/lib/api/queue";

import type { QueueJob } from "@bindings/QueueJob";

const columns: ColumnDef<QueueJob>[] = [
  {
    accessorKey: "id",
    size: 200,
  },
  {
    accessorKey: "queue",
    size: 120,
  },
  {
    accessorKey: "job_type",
    size: 140,
  },
  {
    accessorKey: "status",
    size: 80,
  },
  {
    accessorKey: "priority",
    size: 60,
  },
  {
    accessorKey: "attempts",
    size: 60,
  },
  {
    header: "created",
    accessorKey: "created",
    size: 120,
    cell: (ctx) => {
      const ts = new Date(ctx.row.original.created * 1000);
      return ts.toISOString().replace(/T/, " ").replace(/\.\d{3}Z$/, "Z");
    },
  },
  {
    accessorKey: "last_error",
    size: 200,
  },
];

export default function QueuePage() {
  const jobsQuery = useQuery(() => ({
    queryKey: ["queue-jobs"],
    queryFn: fetchQueueJobs,
  }));

  const statsQuery = useQuery(() => ({
    queryKey: ["queue-stats"],
    queryFn: fetchQueueStats,
  }));

  const tableData = createMemo(() => jobsQuery.data?.jobs ?? []);

  const table = buildTable({
    columns,
    data: tableData,
  });

  return (
    <div class="flex h-full flex-col gap-4 p-4">
      <Header title="Queue" />

      <Show when={statsQuery.data}>
        {(stats) => (
          <div class="flex flex-wrap gap-3">
            <div class="rounded border px-4 py-2 text-sm">
              <span class="font-semibold">Total:</span> {stats().total_jobs}
            </div>
            <For each={stats().by_status}>
              {(s) => (
                <div class="rounded border px-4 py-2 text-sm">
                  <span class="font-semibold">{s.status}:</span> {s.count}
                </div>
              )}
            </For>
          </div>
        )}
      </Show>

      <Table table={table} />
    </div>
  );
}
