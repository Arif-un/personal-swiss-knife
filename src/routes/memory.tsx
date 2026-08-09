import { lazy, Suspense, useMemo, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CameraIcon } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#components/ui/table.tsx";
import { cn } from "#lib/utils.ts";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { memoryApi, memoryKeys } from "#components/memory/api.ts";
import { formatBytes, formatStamp } from "#components/memory/format.ts";
import {
  RANGES,
  type RangeKey,
  type Snapshot,
  type SnapshotSummary,
} from "#components/memory/types.ts";

// Background sampler writes every 15 min; poll a bit more often so a fresh
// snapshot shows up without a manual reload.
const REFETCH_MS = 60_000;

// Lazy-loaded so recharts (and its redux/d3 transitive tree) is code-split into
// the /memory chunk instead of the main bundle.
const MemoryChart = lazy(() =>
  import("#components/memory/MemoryChart.tsx").then((m) => ({ default: m.MemoryChart })),
);

function StatTile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="flex flex-col gap-1 rounded-lg border p-4">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-2xl font-semibold tabular-nums">{value}</span>
      {sub && <span className="text-xs text-muted-foreground">{sub}</span>}
    </div>
  );
}

function MemoryPage() {
  const qc = useQueryClient();
  const [range, setRange] = useState<RangeKey>("24h");

  const { data: history = [] } = useQuery<SnapshotSummary[]>({
    queryKey: memoryKeys.history(),
    queryFn: () => memoryApi.history(),
    refetchInterval: REFETCH_MS,
  });
  const { data: latest } = useQuery<Snapshot | null>({
    queryKey: memoryKeys.latest(),
    queryFn: () => memoryApi.latest(),
    refetchInterval: REFETCH_MS,
  });

  const snapshotNow = useMutation({
    mutationFn: () => memoryApi.snapshotNow(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: memoryKeys.history() });
      qc.invalidateQueries({ queryKey: memoryKeys.latest() });
    },
  });

  const rangeSeconds = RANGES.find((r) => r.key === range)!.seconds;
  const windowed = useMemo(() => {
    if (history.length === 0) return [];
    const newest = history[history.length - 1].ts;
    const cutoff = newest - rangeSeconds;
    return history.filter((p) => p.ts >= cutoff);
  }, [history, rangeSeconds]);

  const peak = useMemo(() => windowed.reduce((max, p) => Math.max(max, p.totalRss), 0), [windowed]);

  const procs = latest?.processes ?? [];
  const maxProc = procs.reduce((max, p) => Math.max(max, p.rssBytes), 0);

  return (
    <div className="flex flex-col gap-6">
      {/* summary + controls */}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatTile
          label="Current total"
          value={latest ? formatBytes(latest.totalRss) : "—"}
          sub={latest ? `updated ${formatStamp(latest.ts)}` : "no snapshots yet"}
        />
        <StatTile label={`Peak (${range})`} value={peak ? formatBytes(peak) : "—"} />
        <StatTile
          label="Processes"
          value={latest ? String(latest.processes.length) : "—"}
          sub="app + spawned"
        />
        <StatTile label="Snapshots kept" value={String(history.length)} sub="last 30 days" />
      </div>

      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1 rounded-md border p-0.5">
          {RANGES.map((r) => (
            <button
              key={r.key}
              onClick={() => setRange(r.key)}
              className={cn(
                "rounded px-2.5 py-1 text-xs font-medium transition-colors",
                range === r.key
                  ? "bg-foreground text-background"
                  : "text-muted-foreground hover:bg-accent",
              )}
            >
              {r.label}
            </button>
          ))}
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => snapshotNow.mutate()}
          disabled={snapshotNow.isPending}
        >
          <CameraIcon /> {snapshotNow.isPending ? "Sampling…" : "Snapshot now"}
        </Button>
      </div>

      {snapshotNow.isError && (
        <ErrorBox error={snapshotNow.error} fallback="Failed to take snapshot" />
      )}

      <Suspense
        fallback={<div className="h-72 w-full rounded-lg border" />}
      >
        <MemoryChart data={windowed} rangeSeconds={rangeSeconds} />
      </Suspense>

      {/* latest per-process breakdown */}
      <div>
        <h2 className="mb-2 text-sm font-medium text-muted-foreground">
          Latest breakdown{latest ? ` — ${formatStamp(latest.ts)}` : ""}
        </h2>
        <div className="rounded-lg border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Process</TableHead>
                <TableHead className="w-20 text-right">PID</TableHead>
                <TableHead className="w-40 text-right">RSS</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {procs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} className="text-center text-sm text-muted-foreground">
                    No snapshot recorded yet.
                  </TableCell>
                </TableRow>
              ) : (
                procs.map((p) => (
                  <TableRow key={p.pid}>
                    <TableCell className="max-w-0 truncate font-medium" title={p.name}>
                      {p.name}
                    </TableCell>
                    <TableCell className="text-right tabular-nums text-muted-foreground">
                      {p.pid}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-2">
                        <div className="h-1.5 w-24 overflow-hidden rounded-full bg-muted">
                          <div
                            className="h-full rounded-full"
                            style={{
                              width: `${maxProc ? (p.rssBytes / maxProc) * 100 : 0}%`,
                              backgroundColor: "var(--mem-series)",
                            }}
                          />
                        </div>
                        <span className="tabular-nums">{formatBytes(p.rssBytes)}</span>
                      </div>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>
    </div>
  );
}

export const memoryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/memory",
  component: MemoryPage,
});
