import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  ChevronsUpDown,
  FolderOpen,
  GitBranch,
  RefreshCw,
  Settings,
  Terminal,
} from "lucide-react";
import { FaGithub } from "react-icons/fa";
import { VscVscode } from "react-icons/vsc";
import { rootRoute } from "./__root.tsx";
import { pickDirectory } from "#lib/pick-directory.ts";
import { Input } from "#components/ui/input.tsx";
import { Button } from "#components/ui/button.tsx";
import { Badge } from "#components/ui/badge.tsx";
import { Skeleton } from "#components/ui/skeleton.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "#components/ui/tooltip.tsx";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#components/ui/table.tsx";
import {
  gitmodApi,
  gitmodKeys,
  type DirtyAction,
  type RepoRow,
} from "#components/submodules/api.ts";

function errMsg(e: unknown, fallback: string) {
  return typeof e === "string" ? e : e instanceof Error ? e.message : fallback;
}

/** One repo row. The branch cell is a searchable dropdown that confirms the switch inline. */
function SubmoduleRow({
  row,
  rootName,
  onSwitch,
  onOpenApp,
  switching,
}: {
  row: RepoRow;
  rootName: string;
  onSwitch: (sub: string, branch: string, action: DirtyAction) => void;
  onOpenApp: (sub: string, app: "github" | "vscode" | "terminal") => void;
  switching: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  // The branch picked in the list, awaiting confirmation. `null` = list step.
  const [pending, setPending] = useState<string | null>(null);
  // Checked = carry uncommitted changes; unchecked = stash them.
  const [bring, setBring] = useState(false);

  const sub = row.isParent ? "" : row.name;

  const matches = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q ? row.branches.filter((b) => b.toLowerCase().includes(q)) : row.branches;
  }, [row.branches, filter]);

  // Reset the popover's transient state whenever it closes.
  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      setFilter("");
      setPending(null);
      setBring(false);
    }
  }

  function pick(b: string) {
    if (b === row.branch) return; // already here, nothing to switch
    setPending(b);
  }

  function confirm() {
    if (!pending) return;
    onOpenChange(false);
    const action: DirtyAction = row.dirty ? (bring ? "carry" : "stash") : "none";
    onSwitch(sub, pending, action);
  }

  return (
    <TableRow>
      <TableCell className="font-medium">
        <div className="flex flex-col gap-1">
          <span className="flex items-center gap-1.5">
            {row.isParent ? (
              <>
                <GitBranch className="size-3.5 text-muted-foreground" />
                {rootName}
                <Badge variant="secondary">parent</Badge>
              </>
            ) : (
              row.name
            )}
          </span>
          <span className="flex items-center gap-2">
            {row.dirty ? (
              <span className="rounded-full bg-destructive/15 px-1.5 py-0.5 text-[10px] font-medium text-destructive">
                dirty
              </span>
            ) : (
              <span className="rounded-full bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                clean
              </span>
            )}
            {row.ahead !== null && row.behind !== null && (
              <span className="font-mono text-[10px] text-muted-foreground">
                ↑{row.ahead} ↓{row.behind}
              </span>
            )}
          </span>
        </div>
      </TableCell>

      <TableCell>
        <div className="flex items-center gap-1.5">
          {row.error ? (
            <span className="text-sm text-destructive" title={row.error}>
              {row.error}
            </span>
          ) : (
            <Popover open={open} onOpenChange={onOpenChange}>
              <PopoverTrigger
                disabled={switching}
                render={
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 max-w-64 justify-between gap-1.5 px-2 font-mono text-xs"
                  >
                    <span
                      className="min-w-0 truncate"
                      title={row.detached ? row.headDesc : row.branch}
                    >
                      {row.detached ? `detached · ${row.headDesc || "?"}` : row.branch || "—"}
                    </span>
                    <ChevronsUpDown className="size-3.5 shrink-0 opacity-50" />
                  </Button>
                }
              />
              <PopoverContent align="start" className="w-72 p-0">
                {pending === null ? (
                  <div className="flex flex-col">
                    <Input
                      autoFocus
                      value={filter}
                      onChange={(e) => setFilter(e.target.value)}
                      placeholder="Search branches…"
                      className="m-1 h-8 border-0 shadow-none focus-visible:ring-0"
                    />
                    <div className="max-h-64 overflow-y-auto border-t p-1">
                      {matches.length === 0 ? (
                        <p className="px-2 py-1.5 text-sm text-muted-foreground">No branches.</p>
                      ) : (
                        matches.map((b) => (
                          <button
                            key={b}
                            type="button"
                            onClick={() => pick(b)}
                            className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left font-mono text-sm hover:bg-accent"
                          >
                            <Check
                              className={b === row.branch ? "size-3.5" : "size-3.5 opacity-0"}
                            />
                            <span className="truncate">{b}</span>
                          </button>
                        ))
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="flex flex-col gap-3 p-3">
                    <p className="text-sm">
                      Switch to <b className="font-mono">{pending}</b>?
                    </p>
                    {row.dirty && (
                      <label className="flex items-start gap-2 text-sm">
                        <input
                          type="checkbox"
                          checked={bring}
                          onChange={(e) => setBring(e.target.checked)}
                          className="mt-0.5"
                        />
                        <span>
                          Bring uncommitted changes along
                          <span className="block text-xs text-muted-foreground">
                            {bring
                              ? "Carried to the new branch."
                              : "Otherwise stashed on this branch."}
                          </span>
                        </span>
                      </label>
                    )}
                    <div className="flex justify-end gap-2">
                      <Button size="sm" variant="outline" onClick={() => setPending(null)}>
                        Cancel
                      </Button>
                      <Button size="sm" onClick={confirm} disabled={switching}>
                        Switch
                      </Button>
                    </div>
                  </div>
                )}
              </PopoverContent>
            </Popover>
          )}
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onOpenApp(sub, "github")}
                  aria-label="Open in GitHub Desktop"
                >
                  <FaGithub />
                </Button>
              }
            />
            <TooltipContent>Open in GitHub Desktop</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onOpenApp(sub, "vscode")}
                  aria-label="Open in VS Code"
                >
                  <VscVscode />
                </Button>
              }
            />
            <TooltipContent>Open in VS Code</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onOpenApp(sub, "terminal")}
                  aria-label="Open in Terminal"
                >
                  <Terminal />
                </Button>
              }
            />
            <TooltipContent>Open in Terminal</TooltipContent>
          </Tooltip>
        </div>
      </TableCell>
    </TableRow>
  );
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

      {saveConfig.isError && (
        <p className="text-sm text-destructive">
          {errMsg(saveConfig.error, "Failed to save path")}
        </p>
      )}
      {switchBranch.isError && (
        <p className="text-sm text-destructive">
          {errMsg(switchBranch.error, "Failed to switch branch")}
        </p>
      )}
      {refreshPull.isError && (
        <p className="text-sm text-destructive">{errMsg(refreshPull.error, "Failed to refresh")}</p>
      )}
      {switchAll.isError && (
        <p className="text-sm text-destructive">
          {errMsg(switchAll.error, "Failed to switch all")}
        </p>
      )}
      {openApp.isError && (
        <p className="text-sm text-destructive">{errMsg(openApp.error, "Failed to open app")}</p>
      )}
      {error && <p className="text-sm text-destructive">{errMsg(error, "Failed to read repo")}</p>}

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
              />
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

export const submodulesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/submodules",
  component: SubmodulesPage,
});
