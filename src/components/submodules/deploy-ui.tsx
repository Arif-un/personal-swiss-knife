import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, ChevronsUpDown, FolderOpen, Loader2 } from "lucide-react";
import { pickDirectory } from "#lib/pick-directory.ts";
import { sshApi, sshKeys } from "#components/ssh/api.ts";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "#components/ui/sheet.tsx";
import {
  wpDeployApi,
  wpDeployEvents,
  wpDeployKeys,
  type DoneEvent,
  type LogLine,
} from "#components/submodules/deployApi.ts";

/** Cap on retained log lines. A build (esp. "Build assets first") can stream
 *  thousands of lines; keeping only a tail bounds the per-line array copy and the
 *  full re-grouping/scroll the log sheet does on every update. */
const MAX_LOG_LINES = 5000;

/** Deploy orchestration: owns the running deploy's id, streamed logs, and the
 *  log-sheet open state. Subscribes once to the backend event channels. */
export function useDeploy(enviraDev: string) {
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<DoneEvent | null>(null);
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const idRef = useRef<string | null>(null);

  useEffect(() => {
    const un1 = wpDeployEvents.onLog((e) => {
      if (e.payload.deployId === idRef.current)
        setLogs((l) => {
          if (l.length < MAX_LOG_LINES) return [...l, e.payload];
          // Over cap: evict the oldest body ("out"/"err") line, keeping "step"/
          // "time" markers so the sheet's section headers survive long streams.
          const i = l.findIndex((x) => x.stream === "out" || x.stream === "err");
          const base = i >= 0 ? [...l.slice(0, i), ...l.slice(i + 1)] : l.slice(1);
          return [...base, e.payload];
        });
    });
    const un2 = wpDeployEvents.onDone((e) => {
      if (e.payload.deployId === idRef.current) {
        setResult(e.payload);
        setRunning(false);
      }
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, []);

  function start(id: string, label: string) {
    idRef.current = id;
    setLogs([]);
    setResult(null);
    setRunning(true);
    setTitle(label);
    setOpen(true);
  }

  function deploy(slug: string, build: boolean) {
    const id = crypto.randomUUID();
    start(id, `Deploying ${slug}`);
    wpDeployApi.deploy(enviraDev, slug, build, id).catch((e) => {
      setResult({ deployId: id, ok: false, message: String(e), version: null });
      setRunning(false);
    });
  }

  function rollback(slug: string) {
    const id = crypto.randomUUID();
    start(id, `Rolling back ${slug}`);
    wpDeployApi.rollback(slug, id).catch((e) => {
      setResult({ deployId: id, ok: false, message: String(e), version: null });
      setRunning(false);
    });
  }

  return { deploy, rollback, running, logs, result, open, setOpen, title };
}

/** Slide-over log panel for the active deploy/rollback. */
export function DeployLogSheet({
  open,
  setOpen,
  title,
  logs,
  running,
  result,
}: {
  open: boolean;
  setOpen: (v: boolean) => void;
  title: string;
  logs: LogLine[];
  running: boolean;
  result: DoneEvent | null;
}) {
  const endRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [logs]);

  // Group the flat log stream into collapsible step sections. A "step" line
  // starts a section, "time" tags its duration, everything else is body output.
  const sections: { title: string; duration?: string; lines: LogLine[] }[] = [];
  for (const l of logs) {
    if (l.stream === "step") {
      sections.push({ title: l.line, lines: [] });
    } else if (l.stream === "time") {
      if (sections.length > 0) sections[sections.length - 1].duration = l.line;
    } else {
      if (sections.length === 0) sections.push({ title: "Output", lines: [] });
      sections[sections.length - 1].lines.push(l);
    }
  }

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetContent side="right" className="w-full sm:max-w-xl">
        <SheetHeader>
          <SheetTitle className="flex items-center gap-2">
            {running && <Loader2 className="size-4 animate-spin" />}
            {title || "Deploy"}
          </SheetTitle>
        </SheetHeader>
        <div className="flex-1 overflow-y-auto px-4">
          <div className="flex flex-col gap-1">
            {sections.map((s, i) => {
              const isLast = i === sections.length - 1;
              // A step only "failed" when the whole run failed and this is the
              // last step reached. Tools write normal output to stderr on
              // success, so stderr alone is not an error.
              const failed = isLast && !!result && !result.ok;
              return (
                <details key={i} open={isLast} className="rounded-md border">
                  <summary className="flex cursor-pointer items-center gap-2 px-2 py-1.5 text-sm font-medium marker:text-muted-foreground">
                    {running && isLast && <Loader2 className="size-3.5 animate-spin" />}
                    <span className="min-w-0 flex-1 truncate">{s.title}</span>
                    {s.duration && (
                      <span className="font-mono text-xs text-muted-foreground">{s.duration}</span>
                    )}
                    {failed && <span className="text-xs text-destructive">failed</span>}
                  </summary>
                  {s.lines.length > 0 && (
                    <pre className="max-h-64 overflow-y-auto whitespace-pre-wrap break-words border-t px-2 py-1.5 font-mono text-xs leading-relaxed text-muted-foreground">
                      {s.lines.map((l, j) => (
                        <div key={j}>{l.line}</div>
                      ))}
                    </pre>
                  )}
                </details>
              );
            })}
            <div ref={endRef} />
          </div>
        </div>
        {result && (
          <div className="px-4 pb-4">
            <p className={result.ok ? "text-sm text-emerald-600" : "text-sm text-destructive"}>
              {result.ok
                ? `✓ ${result.message}${result.version ? ` (version ${result.version})` : ""}`
                : `✗ ${result.message}`}
            </p>
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}

/** Popover content for the global deploy settings: target host, docroot, zip
 *  base. Rendered by the page inside a header Popover. */
export function DeploySettings() {
  const qc = useQueryClient();
  const { data: config } = useQuery({
    queryKey: wpDeployKeys.config(),
    queryFn: wpDeployApi.configGet,
  });
  const { data: hosts } = useQuery({
    queryKey: sshKeys.hosts(),
    queryFn: sshApi.hostsList,
  });

  const [hostId, setHostId] = useState("");
  const [zipBase, setZipBase] = useState("");
  const [docroot, setDocroot] = useState("");
  const [hostOpen, setHostOpen] = useState(false);
  const didInit = useRef(false);
  useEffect(() => {
    if (didInit.current || !config) return;
    didInit.current = true;
    setHostId(config.targetHostId);
    setZipBase(config.zipBase);
    setDocroot(config.docroots[config.targetHostId] ?? "");
  }, [config]);

  const save = useMutation({
    mutationFn: async () => {
      await wpDeployApi.configSave(hostId, zipBase.trim());
      const cfg = await wpDeployApi.setDocroot(hostId, docroot.trim());
      return cfg;
    },
    onSuccess: (cfg) => qc.setQueryData(wpDeployKeys.config(), cfg),
    // If the second write failed after the first persisted, the cache is stale
    // vs what's on disk — refetch so configured-state reflects reality.
    onError: () => qc.invalidateQueries({ queryKey: wpDeployKeys.config() }),
  });

  const detect = useMutation({
    mutationFn: (id: string) => wpDeployApi.detectDocroot(id),
    // Ignore a slow scan that resolves after the user picked a different host,
    // so its docroot can't overwrite the now-selected host's field.
    onSuccess: (cands, id) => {
      if (id === hostId && cands.length > 0) setDocroot(cands[0]);
    },
  });

  const hostList = hosts ?? [];
  const currentHost = hostList.find((h) => h.id === hostId);
  const hostLabel = currentHost ? currentHost.alias || currentHost.hostname : "— select host —";

  // Selecting a host loads its saved docroot, or auto-detects when none is set.
  function selectHost(id: string) {
    setHostId(id);
    setHostOpen(false);
    const saved = config?.docroots[id] ?? "";
    setDocroot(saved);
    if (id && !saved) detect.mutate(id);
  }

  const reset = useMutation({
    mutationFn: () => wpDeployApi.configReset(),
    onSuccess: (cfg) => {
      qc.setQueryData(wpDeployKeys.config(), cfg);
      setHostId("");
      setZipBase("");
      setDocroot("");
    },
  });

  const canSave = hostId.trim().length > 0 && docroot.trim().length > 0;

  async function browse() {
    const dir = await pickDirectory();
    if (dir) setZipBase(dir);
  }

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm font-medium">Deploy settings</p>

      <div className="flex flex-col gap-1 text-xs text-muted-foreground">
        Target host
        <Popover open={hostOpen} onOpenChange={setHostOpen}>
          <PopoverTrigger
            render={
              <Button
                variant="outline"
                size="sm"
                className="h-8 justify-between gap-1.5 px-2 text-sm font-normal text-foreground"
              >
                <span className="min-w-0 truncate">{hostLabel}</span>
                <ChevronsUpDown className="size-3.5 shrink-0 opacity-50" />
              </Button>
            }
          />
          <PopoverContent align="start" className="w-64 p-1">
            {hostList.length === 0 ? (
              <p className="px-2 py-1.5 text-sm text-muted-foreground">
                No SSH hosts. Add one on the SSH page.
              </p>
            ) : (
              <div className="max-h-64 overflow-y-auto">
                {hostList.map((h) => (
                  <button
                    key={h.id}
                    type="button"
                    onClick={() => selectHost(h.id)}
                    className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-accent"
                  >
                    <Check className={h.id === hostId ? "size-3.5" : "size-3.5 opacity-0"} />
                    <span className="min-w-0 truncate">
                      {h.alias || h.hostname}
                      {h.hostname && h.alias ? (
                        <span className="text-muted-foreground"> ({h.hostname})</span>
                      ) : null}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </PopoverContent>
        </Popover>
      </div>

      <label className="flex flex-col gap-1 text-xs text-muted-foreground">
        WordPress docroot
        <div className="flex gap-1.5">
          <Input
            value={docroot}
            onChange={(e) => setDocroot(e.target.value)}
            placeholder={
              !hostId
                ? "Select a host first"
                : detect.isPending
                  ? "Detecting…"
                  : "/home/user/web/site/public_html"
            }
            disabled={!hostId || detect.isPending}
            className="h-8 text-sm"
          />
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={!hostId || detect.isPending}
            onClick={() => detect.mutate(hostId)}
            title="Auto-detect (scan ~/web/*)"
          >
            {detect.isPending ? <Loader2 className="size-3.5 animate-spin" /> : "Detect"}
          </Button>
        </div>
      </label>

      <label className="flex flex-col gap-1 text-xs text-muted-foreground">
        Zip base dir
        <div className="flex gap-1.5">
          <Input
            value={zipBase}
            onChange={(e) => setZipBase(e.target.value)}
            placeholder="~/wp-deploy-zips"
            className="h-8 text-sm"
          />
          <Button
            type="button"
            size="icon-sm"
            variant="outline"
            onClick={browse}
            aria-label="Browse"
          >
            <FolderOpen />
          </Button>
        </div>
      </label>

      {!canSave && (
        <p className="text-xs text-muted-foreground">
          Target host and docroot are required to deploy.
        </p>
      )}
      {detect.isError && <p className="text-xs text-destructive">Detect failed.</p>}
      {save.isError && <p className="text-xs text-destructive">Save failed.</p>}

      <div className="flex gap-2">
        <Button
          size="sm"
          className="flex-1"
          disabled={!canSave || save.isPending}
          onClick={() => save.mutate()}
        >
          {save.isPending ? "Saving…" : save.isSuccess ? "Saved" : "Save settings"}
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={reset.isPending}
          onClick={() => reset.mutate()}
          title="Clear all deploy settings"
        >
          Reset
        </Button>
      </div>
    </div>
  );
}

/** Whether deploy is fully configured (target host + zip base + that host's docroot). */
export function useDeployConfigured() {
  const { data: config } = useQuery({
    queryKey: wpDeployKeys.config(),
    queryFn: wpDeployApi.configGet,
  });
  return !!(
    config &&
    config.targetHostId &&
    config.zipBase &&
    config.docroots[config.targetHostId]
  );
}
