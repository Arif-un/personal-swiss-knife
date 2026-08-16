import { useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLinkIcon } from "lucide-react";
import { cn } from "#lib/utils.ts";
import { devkonApi, devkonKeys, type DevkonEntry, type RunStatus } from "#components/devkon/api.ts";

// Cap the "awaiting re-attach" poll window to ~5min of wall-clock. A real run
// resolves well within this; the cap stops a never-materializing dispatch (or a
// persistently failing `gh`) from spawning a subprocess every 5s forever.
const AWAIT_WINDOW_MS = 5 * 60_000;

function statusLabel(s: RunStatus | undefined, entry: DevkonEntry): { text: string; dot: string } {
  const kind = s?.kind ?? entry.lastRunKind;
  const runId = s?.runId ?? entry.lastRunId;
  // Dispatched (kind set) but the run id wasn't captured within the watch window;
  // a status poll re-attaches it, so show it as in-progress meanwhile.
  if (runId == null && entry.lastRunKind != null && (s?.state ?? "none") === "none")
    return { text: kind === "destroy" ? "Destroying…" : "Deploying…", dot: "bg-amber-500" };
  const state = s?.state ?? (runId ? "unknown" : "none");
  if (state === "none") return { text: "Not deployed", dot: "bg-muted-foreground" };
  if (state === "queued" || state === "in_progress")
    return {
      text: kind === "destroy" ? "Destroying…" : "Deploying…",
      dot: "bg-amber-500",
    };
  if (state === "completed") {
    if (s?.conclusion === "success")
      return kind === "destroy"
        ? { text: "Destroyed", dot: "bg-muted-foreground" }
        : { text: "Deployed", dot: "bg-green-500" };
    return { text: `Failed (${s?.conclusion ?? "?"})`, dot: "bg-red-500" };
  }
  return { text: state, dot: "bg-muted-foreground" };
}

/** Per-row status cell: polls while a run is queued/in-progress. */
export function StatusCell({ entry }: { entry: DevkonEntry }) {
  // When this row entered "awaiting re-attach", so the window is bounded per-dispatch.
  // Not q.state.dataUpdateCount: that counts every successful poll for the row's
  // whole lifetime, so a prior long deploy exhausts the cap and a later awaiting
  // dispatch would never poll to re-attach (stuck on "Deploying…").
  const awaitingSince = useRef<number | null>(null);
  const { data } = useQuery({
    queryKey: devkonKeys.status(entry.id),
    queryFn: () => devkonApi.status(entry.id),
    enabled: entry.lastRunId !== null || entry.lastRunKind !== null,
    refetchInterval: (q) => {
      const d = q.state.data;
      // Keep polling while a dispatched run is unresolved (awaiting re-attach) or running.
      const awaiting = (d?.runId ?? entry.lastRunId) == null && entry.lastRunKind != null;
      if (awaiting) {
        if (awaitingSince.current == null) awaitingSince.current = Date.now();
        return Date.now() - awaitingSince.current < AWAIT_WINDOW_MS ? 5_000 : false;
      }
      awaitingSince.current = null;
      if (d?.state === "queued" || d?.state === "in_progress") return 5_000;
      return false;
    },
  });

  const s = statusLabel(data, entry);
  const kind = data?.kind ?? entry.lastRunKind;
  // last_deployed_at is only written on a successful apply and never cleared on
  // destroy, so a torn-down row would otherwise still show its old "deployed <time>".
  const deployedAt = kind === "destroy" ? null : (data?.lastDeployedAt ?? entry.lastDeployedAt);
  const runUrl = entry.lastRunUrl;

  return (
    <div className="flex flex-col gap-0.5">
      <span className="flex items-center gap-1.5 text-sm">
        <span className={cn("size-2 rounded-full", s.dot)} />
        {s.text}
        {runUrl && (
          <a
            href={runUrl}
            target="_blank"
            rel="noreferrer"
            className="text-muted-foreground hover:text-foreground"
            title="Open GitHub Actions run"
          >
            <ExternalLinkIcon className="size-3" />
          </a>
        )}
      </span>
      {deployedAt && (
        <span className="text-xs text-muted-foreground">
          deployed {new Date(deployedAt).toLocaleString()}
        </span>
      )}
    </div>
  );
}
