import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderOpen, GitBranch, RefreshCw, Rocket, Settings } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { pickDirectory } from "#lib/pick-directory.ts";
import { Input } from "#components/ui/input.tsx";
import { Button } from "#components/ui/button.tsx";
import { Skeleton } from "#components/ui/skeleton.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { Table, TableBody, TableHead, TableHeader, TableRow } from "#components/ui/table.tsx";
import {
  gitmodApi,
  gitmodKeys,
  type DirtyAction,
  type RepoRow,
} from "#components/submodules/api.ts";
import { SubmoduleRow } from "#components/submodules/SubmoduleRow.tsx";
import {
  DeployLogSheet,
  DeploySettings,
  useDeploy,
  useDeployConfigured,
} from "#components/submodules/deploy-ui.tsx";

function errMsg(e: unknown, fallback: string) {
  return typeof e === "string" ? e : e instanceof Error ? e.message : fallback;
}

function SubmodulesPage() {
  const qc = useQueryClient();
  const { data: config } = useQuery({
    queryKey: gitmodKeys.config(),
    queryFn: gitmodApi.getConfig,
  });

  // Committed path drives the status query; `path` is the editable field.
  const [path, setPath] = useState("");
  const savedPath = config?.path ?? "";
  const didInit = useRef(false);
  useEffect(() => {
    if (didInit.current || config === undefined) return;
    didInit.current = true;
    setPath(config.path);
  }, [config]);

  const saveConfig = useMutation({
    mutationFn: (p: string) => gitmodApi.setConfig(p),
    onSuccess: (cfg) => qc.setQueryData(gitmodKeys.config(), cfg),
  });

  const {
    data: rows,
    isFetching,
    error,
    refetch,
  } = useQuery({
    queryKey: gitmodKeys.status(savedPath),
    queryFn: () => gitmodApi.status(savedPath),
    enabled: savedPath.trim().length > 0,
    // Fetching remotes is slow; don't auto-refetch, let the user drive it.
    staleTime: Infinity,
  });

  const switchBranch = useMutation({
    mutationFn: ({ sub, branch, action }: { sub: string; branch: string; action: DirtyAction }) =>
      gitmodApi.switch(savedPath, sub, branch, action),
    onSuccess: (updated) => {
      // Replace just the switched row in place; no full re-fetch needed.
      qc.setQueryData<RepoRow[]>(gitmodKeys.status(savedPath), (prev) =>
        prev?.map((r) => (r.name === updated.name ? updated : r)),
      );
    },
  });

  // Fetch + fast-forward every repo, then replace all rows.
  const refreshPull = useMutation({
    mutationFn: () => gitmodApi.refreshPull(savedPath),
    onSuccess: (updated) => qc.setQueryData(gitmodKeys.status(savedPath), updated),
  });

  // Switch every repo to develop (fallback master/main). Confirm + dirty choice
  // live in the toolbar popover below.
  const [switchAllOpen, setSwitchAllOpen] = useState(false);
  const [bringAll, setBringAll] = useState(false);
  const [switchAllNotes, setSwitchAllNotes] = useState<string[]>([]);
  const anyDirty = rows?.some((r) => r.dirty) ?? false;
  const switchAll = useMutation({
    mutationFn: (action: DirtyAction) => gitmodApi.switchAll(savedPath, action),
    onSuccess: (res) => {
      qc.setQueryData(gitmodKeys.status(savedPath), res.rows);
      setSwitchAllNotes(res.notes);
    },
  });

  const openApp = useMutation({
    mutationFn: ({ sub, app }: { sub: string; app: "github" | "vscode" | "terminal" }) =>
      gitmodApi.openApp(savedPath, sub, app),
  });

  const deployConfigured = useDeployConfigured();
  const deploy = useDeploy(savedPath);
  const [settingsOpen, setSettingsOpen] = useState(false);

  function confirmSwitchAll() {
    setSwitchAllOpen(false);
    setSwitchAllNotes([]);
    switchAll.mutate(anyDirty ? (bringAll ? "carry" : "stash") : "none");
  }

  const busy = isFetching || refreshPull.isPending || switchAll.isPending || switchBranch.isPending;

  function savePath(e: React.FormEvent) {
    e.preventDefault();
    saveConfig.mutate(path.trim());
  }

  async function browsePath() {
    const dir = await pickDirectory();
    if (dir) setPath(dir);
  }

  const rootName = savedPath.replace(/\/+$/, "").split("/").filter(Boolean).pop() ?? "repo";

  const [headerSlot, setHeaderSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    setHeaderSlot(document.getElementById("header-actions"));
  }, []);

  // Directory bar visibility, persisted across reloads.
  const [showPathBar, setShowPathBar] = useState(
    () => localStorage.getItem("gitmod:showPathBar") !== "false",
  );
  useEffect(() => {
    localStorage.setItem("gitmod:showPathBar", String(showPathBar));
  }, [showPathBar]);

  return (
    <div className="flex flex-col gap-6">
      {headerSlot &&
        createPortal(
          <>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => refetch()}
              disabled={!savedPath.trim() || isFetching}
              aria-label="Refresh"
              title="Refresh (fetches all remotes)"
            >
              <RefreshCw className={isFetching ? "animate-spin" : undefined} />
            </Button>
            <Popover open={settingsOpen} onOpenChange={setSettingsOpen}>
              <PopoverTrigger
                render={
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Deploy settings"
                    title="Deploy settings"
                  >
                    <Rocket />
                  </Button>
                }
              />
              <PopoverContent align="end" className="w-72">
                <DeploySettings />
              </PopoverContent>
            </Popover>
            <Popover>
              <PopoverTrigger
                render={
                  <Button variant="ghost" size="icon-sm" aria-label="Settings" title="Settings">
                    <Settings />
                  </Button>
                }
              />
              <PopoverContent align="end" className="w-56">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={showPathBar}
                    onChange={(e) => setShowPathBar(e.target.checked)}
                  />
                  Show directory bar
                </label>
              </PopoverContent>
            </Popover>
          </>,
          headerSlot,
        )}

      {showPathBar && (
        <form onSubmit={savePath} className="flex gap-2">
          <Input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="/path/to/superproject"
            className="max-w-lg"
          />
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={browsePath}
            aria-label="Browse for directory"
            title="Browse"
          >
            <FolderOpen />
          </Button>
          <Button type="submit" disabled={saveConfig.isPending || path.trim() === savedPath}>
            Save
          </Button>
        </form>
      )}

      {(
        [
          [saveConfig.isError, saveConfig.error, "Failed to save path"],
          [switchBranch.isError, switchBranch.error, "Failed to switch branch"],
          [refreshPull.isError, refreshPull.error, "Failed to refresh"],
          [switchAll.isError, switchAll.error, "Failed to switch all"],
          [openApp.isError, openApp.error, "Failed to open app"],
          [!!error, error, "Failed to read repo"],
        ] as const
      )
        .filter(([shown]) => shown)
        .map(([, err, fallback]) => (
          <p key={fallback} className="text-sm text-destructive">
            {errMsg(err, fallback)}
          </p>
        ))}

      {switchAllNotes.length > 0 && (
        <ul className="rounded-md border bg-muted/40 p-3 text-sm text-muted-foreground">
          {switchAllNotes.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}

      {!savedPath.trim() && (
        <p className="text-sm text-muted-foreground">
          Set a superproject path above to list its submodules.
        </p>
      )}

      {savedPath.trim() && isFetching && !rows && (
        <div className="flex flex-col gap-2">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-11 w-full" />
          ))}
        </div>
      )}

      {rows && (
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => refreshPull.mutate()} disabled={busy}>
            <RefreshCw className={refreshPull.isPending ? "animate-spin" : undefined} />
            Refresh &amp; pull
          </Button>
          <Popover open={switchAllOpen} onOpenChange={setSwitchAllOpen}>
            <PopoverTrigger
              disabled={busy}
              render={
                <Button variant="outline" size="sm">
                  <GitBranch />
                  Switch all to develop
                </Button>
              }
            />
            <PopoverContent align="start" className="w-80">
              <p className="mb-2 text-sm">
                Switch <b>every repo</b> to <span className="font-mono">develop</span> (or{" "}
                <span className="font-mono">master</span>/<span className="font-mono">main</span> if
                absent)?
              </p>
              {anyDirty && (
                <label className="mb-3 flex items-start gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={bringAll}
                    onChange={(e) => setBringAll(e.target.checked)}
                    className="mt-0.5"
                  />
                  <span>
                    Bring uncommitted changes along
                    <span className="block text-xs text-muted-foreground">
                      {bringAll
                        ? "Carried to the new branch."
                        : "Otherwise stashed on each dirty repo."}
                    </span>
                  </span>
                </label>
              )}
              <div className="flex justify-end gap-2">
                <Button size="sm" variant="outline" onClick={() => setSwitchAllOpen(false)}>
                  Cancel
                </Button>
                <Button size="sm" onClick={confirmSwitchAll}>
                  Switch all
                </Button>
              </div>
            </PopoverContent>
          </Popover>
        </div>
      )}

      {rows && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Repository</TableHead>
              <TableHead>Branch</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <SubmoduleRow
                key={row.name}
                row={row}
                rootName={rootName}
                switching={busy}
                onSwitch={(sub, branch, action) => switchBranch.mutate({ sub, branch, action })}
                onOpenApp={(sub, app) => openApp.mutate({ sub, app })}
                enviraDev={savedPath}
                deployConfigured={deployConfigured}
                deployBusy={deploy.running}
                onDeploy={deploy.deploy}
                onRollback={deploy.rollback}
                onOpenSettings={() => setSettingsOpen(true)}
              />
            ))}
          </TableBody>
        </Table>
      )}

      <DeployLogSheet
        open={deploy.open}
        setOpen={deploy.setOpen}
        title={deploy.title}
        logs={deploy.logs}
        running={deploy.running}
        result={deploy.result}
      />
    </div>
  );
}

export const submodulesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/submodules",
  component: SubmodulesPage,
});
