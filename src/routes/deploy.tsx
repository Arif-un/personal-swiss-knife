import { useEffect, useRef, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLinkIcon, FolderOpen, Trash2Icon } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { pickDirectory } from "#lib/pick-directory.ts";
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
import { Tooltip, TooltipContent, TooltipTrigger } from "#components/ui/tooltip.tsx";
import { cn } from "#lib/utils.ts";
import {
  devkonApi,
  devkonKeys,
  MODE_LABELS,
  type DevkonEntry,
  type DevkonMode,
  type RunStatus,
} from "#components/devkon/api.ts";
import { awsauthApi, awsauthKeys, type AwsAuthConfig } from "#components/awsauth/api.ts";

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

/** AWS SAML login helper: opens Brave to the login link (two tabs), waits for the
 * manual `credentials` download, then runs `tools/awsauth` (starting Docker if
 * needed). Profile and repo dir are persisted. */
function AwsLoginPanel() {
  const qc = useQueryClient();
  const { data: config } = useQuery({
    queryKey: awsauthKeys.config(),
    queryFn: () => awsauthApi.getConfig(),
  });
  // Local edits so typing isn't clobbered by the query; seeded once config loads.
  const [profile, setProfile] = useState("");
  const [repoDir, setRepoDir] = useState("");
  useEffect(() => {
    if (config) {
      setProfile(config.braveProfile);
      setRepoDir(config.repoDir);
    }
  }, [config]);

  const saveConfig = useMutation({
    mutationFn: (c: AwsAuthConfig) => awsauthApi.setConfig(c),
    onSuccess: () => qc.invalidateQueries({ queryKey: awsauthKeys.config() }),
  });
  const persist = () => {
    const next = { braveProfile: profile.trim(), repoDir: repoDir.trim() };
    if (config && (next.braveProfile !== config.braveProfile || next.repoDir !== config.repoDir))
      saveConfig.mutate(next);
  };

  async function browseRepoDir() {
    const dir = await pickDirectory();
    if (!dir) return;
    setRepoDir(dir);
    saveConfig.mutate({ braveProfile: profile.trim(), repoDir: dir });
  }

  // Credentials-download wait: the countdown/cancel loop lives here (frontend) so
  // it's cancellable and shows a live timer. `finish` is the Docker + awsauth tail.
  const finish = useMutation({ mutationFn: () => awsauthApi.finish() });
  const [waiting, setWaiting] = useState(false);
  const [remaining, setRemaining] = useState(0);
  const [durationSec, setDurationSec] = useState(30); // session-only, not persisted
  const [waitError, setWaitError] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);
  // True only while a wait loop is live. cancel()/unmount flips it false so an
  // already-in-flight checkFresh() can't fire finish.mutate() after the fact.
  const activeRef = useRef(false);

  const stopPoll = () => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  };
  // Disarm the loop and clear the timer on unmount.
  useEffect(
    () => () => {
      activeRef.current = false;
      stopPoll();
    },
    [],
  );

  const cancel = () => {
    activeRef.current = false;
    stopPoll();
    setWaiting(false);
    setRemaining(0);
  };

  async function start() {
    persist();
    finish.reset();
    setWaitError(null);
    const dur = Math.min(120, Math.max(5, Math.round(durationSec) || 30));
    let baseline: number | null;
    try {
      baseline = await awsauthApi.openBrave();
    } catch (e) {
      setWaitError(String(e));
      return;
    }
    const deadline = Date.now() + dur * 1000;
    activeRef.current = true;
    setWaiting(true);
    setRemaining(dur);
    let busy = false; // guard against overlapping async polls
    pollRef.current = window.setInterval(async () => {
      if (busy) return;
      busy = true;
      try {
        setRemaining(Math.max(0, Math.ceil((deadline - Date.now()) / 1000)));
        const fresh = await awsauthApi.checkFresh(baseline);
        // Cancelled/unmounted while the IPC round-trip was in flight: drop the
        // result so a stale "fresh" can't kick off Docker + awsauth after cancel.
        if (!activeRef.current) return;
        if (fresh) {
          cancel();
          finish.mutate();
        } else if (Date.now() >= deadline) {
          cancel();
          setWaitError(`credentials file was not downloaded within ${dur}s`);
        }
      } finally {
        busy = false;
      }
    }, 500);
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="mr-1 text-sm font-medium">AWS login</span>
        <Tooltip>
          <TooltipTrigger
            render={
              <Input
                value={profile}
                onChange={(e) => setProfile(e.target.value)}
                onBlur={persist}
                placeholder="Brave profile"
                aria-label="Brave profile"
                className="h-7 w-32"
              />
            }
          />
          <TooltipContent>Brave profile</TooltipContent>
        </Tooltip>
        <span className="flex gap-1">
          <Tooltip>
            <TooltipTrigger
              render={
                <Input
                  value={repoDir}
                  onChange={(e) => setRepoDir(e.target.value)}
                  onBlur={persist}
                  placeholder="/Volumes/workspace/netspring"
                  aria-label="Repo directory"
                  className="h-7 w-72"
                />
              }
            />
            <TooltipContent>Repo directory</TooltipContent>
          </Tooltip>
          <Button
            type="button"
            variant="outline"
            size="icon-sm"
            className="h-7"
            onClick={browseRepoDir}
            aria-label="Browse for directory"
            title="Browse"
          >
            <FolderOpen />
          </Button>
        </span>
        <Tooltip>
          <TooltipTrigger
            render={
              <Input
                type="number"
                min={5}
                max={120}
                value={durationSec}
                onChange={(e) => setDurationSec(Number(e.target.value))}
                disabled={waiting || finish.isPending}
                aria-label="Wait (seconds)"
                className="h-7 w-16"
              />
            }
          />
          <TooltipContent>Wait (seconds)</TooltipContent>
        </Tooltip>
        {waiting ? (
          <Button size="sm" variant="destructive" onClick={cancel}>
            Cancel ({remaining}s)
          </Button>
        ) : (
          <Button size="sm" disabled={finish.isPending} onClick={start}>
            {finish.isPending ? "Authenticating…" : "Login"}
          </Button>
        )}
      </div>

      <span className="text-xs text-muted-foreground">
        Opens the SAML link in Brave (two tabs). Download{" "}
        <code className="text-xs">credentials</code> to{" "}
        <code className="text-xs">~/Downloads/AWS</code> before the countdown, then runs{" "}
        <code className="text-xs">tools/awsauth</code>. Click again to cancel.
      </span>

      {finish.isSuccess && (
        <span className="flex items-center gap-1.5 text-sm">
          <span className="size-2 rounded-full bg-green-500" />
          Login succeeded.
        </span>
      )}
      {finish.isError && (
        <pre className="max-h-64 overflow-auto rounded-md border border-destructive bg-destructive/5 p-3 text-xs whitespace-pre-wrap text-destructive">
          {/* Tauri rejects with the Rust Err string (the combined awsauth log), not an Error. */}
          {finish.error instanceof Error ? finish.error.message : String(finish.error)}
        </pre>
      )}
      {waitError && <ErrorBox error={waitError} fallback="AWS login failed" />}
    </div>
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

      <AwsLoginPanel />

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
