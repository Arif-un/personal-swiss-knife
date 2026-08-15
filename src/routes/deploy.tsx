import { useRef, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLinkIcon, Trash2Icon } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#components/ui/table.tsx";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { cn } from "#lib/utils.ts";
import {
  devkonApi,
  devkonKeys,
  MODE_LABELS,
  type DevkonEntry,
  type DevkonMode,
  type RunStatus,
} from "#components/devkon/api.ts";

const BRANCH_LIST_ID = "devkon-branches";
// Cap the "awaiting re-attach" poll window to ~5min of wall-clock. A real run
// resolves well within this; the cap stops a never-materializing dispatch (or a
// persistently failing `gh`) from spawning a subprocess every 5s forever.
const AWAIT_WINDOW_MS = 5 * 60_000;

function accessUrl(name: string) {
  return `https://${name}-dev.devkon.shared.netspring.team`;
}

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
function StatusCell({ entry }: { entry: DevkonEntry }) {
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

/** One editable row. Branch is controlled local state so a mode change reads the
 * currently-typed branch instead of a stale render closure (which would revert a
 * just-typed branch, since devkon_save overwrites name/branch/mode wholesale). */
function DeployRow({
  entry,
  busy,
  onSave,
  onDeploy,
  onDestroy,
  onRemove,
  removePending,
}: {
  entry: DevkonEntry;
  busy: boolean;
  onSave: (e: DevkonEntry) => void;
  onDeploy: () => void;
  onDestroy: () => void;
  onRemove: () => void;
  removePending: boolean;
}) {
  const [branch, setBranch] = useState(entry.branch);
  // Local mode too (same reason as branch): the confirm and the backend dispatch
  // must agree on what's about to run. The backend re-reads mode from disk, and the
  // `entry` prop lags the save's refetch, so gating the wipe-confirm on the prop let
  // a fast select-clean-then-Deploy skip the prompt. Local state is set synchronously
  // on change, so the confirm always reflects the mode the user actually picked.
  const [mode, setMode] = useState(entry.mode);
  const saveBranch = () => {
    const b = branch.trim();
    if (b !== entry.branch) onSave({ ...entry, branch: b });
  };
  return (
    <TableRow>
      <TableCell className="font-medium align-top">
        <div className="flex flex-col gap-0.5">
          {entry.name}
          <a
            href={accessUrl(entry.name)}
            target="_blank"
            rel="noreferrer"
            className="text-xs text-muted-foreground hover:text-foreground truncate max-w-[16rem]"
          >
            {entry.name}-dev.devkon…
          </a>
        </div>
      </TableCell>

      <TableCell className="align-top">
        <Input
          list={BRANCH_LIST_ID}
          value={branch}
          onChange={(e) => setBranch(e.target.value)}
          onBlur={saveBranch}
          placeholder="branch…"
          className="h-7 w-44"
        />
      </TableCell>

      <TableCell className="align-top">
        <select
          value={mode}
          onChange={(e) => {
            const m = e.target.value as DevkonMode;
            setMode(m);
            onSave({ ...entry, branch: branch.trim(), mode: m });
          }}
          className="h-7 rounded-lg border bg-background px-2 text-sm"
        >
          {(Object.keys(MODE_LABELS) as DevkonMode[]).map((m) => (
            <option key={m} value={m}>
              {MODE_LABELS[m]}
            </option>
          ))}
        </select>
      </TableCell>

      <TableCell className="align-top">
        <StatusCell entry={entry} />
      </TableCell>

      <TableCell className="align-top">
        <div className="flex items-center justify-end gap-1.5">
          <Button
            size="sm"
            disabled={busy || !branch.trim()}
            onClick={() => {
              // Clean modes tear down and recreate the namespace (wiping its data),
              // so confirm - matching Destroy/Remove - instead of a silent one-click.
              if (
                mode.startsWith("clean") &&
                !window.confirm(
                  `Clean redeploy tears down and recreates the "${entry.name}" namespace, wiping its data. Continue?`,
                )
              )
                return;
              onDeploy();
            }}
          >
            {busy ? "Dispatching…" : "Deploy"}
          </Button>
          <Button
            size="sm"
            variant="destructive"
            disabled={busy}
            onClick={() => {
              if (
                window.confirm(
                  `Destroy the "${entry.name}" namespace? This tears down the deployment.`,
                )
              )
                onDestroy();
            }}
          >
            {busy ? "Dispatching…" : "Destroy"}
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            title="Remove from list"
            disabled={busy || removePending}
            onClick={() => {
              if (window.confirm(`Remove "${entry.name}" from the list?`)) onRemove();
            }}
          >
            <Trash2Icon />
          </Button>
        </div>
      </TableCell>
    </TableRow>
  );
}

function DeployPage() {
  const qc = useQueryClient();
  const [newName, setNewName] = useState("");
  // Ids currently mid-dispatch. A Set (not the shared mutation's `variables`, which
  // only holds the latest arg) so dispatching a second row can't re-enable the first.
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const markBusy = (id: string, busy: boolean) =>
    setBusyIds((prev) => {
      const next = new Set(prev);
      if (busy) next.add(id);
      else next.delete(id);
      return next;
    });
  const dispatch = (
    m: { mutate: (id: string, opts: { onSettled: () => void }) => void },
    id: string,
  ) => {
    markBusy(id, true);
    m.mutate(id, { onSettled: () => markBusy(id, false) });
  };

  const { data } = useQuery({
    queryKey: devkonKeys.list(),
    queryFn: () => devkonApi.list(),
  });
  const { data: branches } = useQuery({
    queryKey: devkonKeys.branches(),
    queryFn: () => devkonApi.branches(),
    staleTime: 5 * 60_000,
  });

  const entries = data?.entries ?? [];
  const invalidateList = () => qc.invalidateQueries({ queryKey: devkonKeys.list() });

  const save = useMutation({
    mutationFn: devkonApi.save,
    onSuccess: invalidateList,
  });
  const add = useMutation({
    mutationFn: (name: string) => devkonApi.save({ name }),
    onSuccess: () => {
      setNewName("");
      invalidateList();
    },
  });
  const remove = useMutation({
    mutationFn: devkonApi.remove,
    onSuccess: invalidateList,
  });
  const deploy = useMutation({
    mutationFn: devkonApi.deploy,
    onSuccess: (e) => {
      invalidateList();
      qc.invalidateQueries({ queryKey: devkonKeys.status(e.id) });
    },
  });
  const destroy = useMutation({
    mutationFn: devkonApi.destroy,
    onSuccess: (e) => {
      invalidateList();
      qc.invalidateQueries({ queryKey: devkonKeys.status(e.id) });
    },
  });

  return (
    <div className="flex flex-col gap-6">
      <p className="text-sm text-muted-foreground">
        Deploy and destroy isolated devkon namespaces. Each name maps to{" "}
        <code className="text-xs">{"{name}"}-dev.devkon.shared.netspring.team</code> and dispatches
        the <code className="text-xs">deploy-dev-cluster.yml</code> workflow.
      </p>

      <form
        className="flex items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          const n = newName.trim();
          if (n) add.mutate(n);
        }}
      >
        <Input
          placeholder="New name (namespace)…"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          className="max-w-xs"
        />
        <Button type="submit" disabled={!newName.trim() || add.isPending}>
          Add
        </Button>
      </form>

      {(add.isError || save.isError || remove.isError) && (
        <ErrorBox
          error={add.error ?? save.error ?? remove.error}
          fallback="Failed to update the list"
        />
      )}
      {(deploy.isError || destroy.isError) && (
        <ErrorBox
          error={deploy.error ?? destroy.error}
          fallback="Failed to dispatch the workflow"
        />
      )}

      {/* Shared branch options for every row's <input list>. */}
      <datalist id={BRANCH_LIST_ID}>
        {(branches ?? []).map((b) => (
          <option key={b} value={b} />
        ))}
      </datalist>

      {entries.length === 0 ? (
        <p className="text-sm text-muted-foreground">No names yet. Add one above.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Branch</TableHead>
              <TableHead>Mode</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {entries.map((entry) => (
              <DeployRow
                key={entry.id}
                entry={entry}
                busy={busyIds.has(entry.id)}
                onSave={(e) => save.mutate(e)}
                onDeploy={() => dispatch(deploy, entry.id)}
                onDestroy={() => dispatch(destroy, entry.id)}
                onRemove={() => remove.mutate(entry.id)}
                removePending={remove.isPending}
              />
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

export const deployRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/deploy",
  component: DeployPage,
});
