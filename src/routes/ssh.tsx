import { useCallback, useEffect, useRef, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { PlusIcon, XIcon } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { cn } from "#lib/utils.ts";
import { sshApi, sshEvents, sshKeys } from "#components/ssh/api.ts";
import { TERM_BACKGROUND, TOAST_DURATION_MS } from "#components/ssh/constants.ts";
import { HostList } from "#components/ssh/HostList.tsx";
import { HostForm } from "#components/ssh/HostForm.tsx";
import { HostKeyDialog } from "#components/ssh/HostKeyDialog.tsx";
import { ImportDialog } from "#components/ssh/ImportDialog.tsx";
import { TerminalView } from "#components/ssh/TerminalView.tsx";
import { ForwardsPanel } from "#components/ssh/ForwardsPanel.tsx";
import { emptyHost, type Host, type HostKeyPrompt } from "#components/ssh/types.ts";

interface Tab {
  key: string;
  host: Host;
  sessionId: string | null;
  closed: boolean;
}

// Ignore repeat connect requests for the same host within this window, so a
// double-click (two click events + a dblclick) opens a single tab.
const CONNECT_DEBOUNCE_MS = 500;

function plural(n: number, word: string) {
  return `${word}${n === 1 ? "" : "s"}`;
}

function SshPage() {
  const qc = useQueryClient();
  const { data: hosts = [], isLoading } = useQuery<Host[]>({
    queryKey: sshKeys.hosts(),
    queryFn: () => sshApi.hostsList(),
  });
  const invalidateHosts = () => qc.invalidateQueries({ queryKey: sshKeys.hosts() });

  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [editing, setEditing] = useState<Host | null>(null);
  const [importing, setImporting] = useState<Host[] | null>(null);
  // Queue of pending host-key prompts. Concurrent connects to distinct unknown
  // hosts each emit a prompt; we show them one at a time (front of queue) so a
  // later prompt never clobbers an unanswered earlier one (which would leave
  // that connect hanging until its backend timeout).
  const [prompts, setPrompts] = useState<HostKeyPrompt[]>([]);
  const [toast, setToast] = useState<string | null>(null);

  const toastTimer = useRef<number | null>(null);
  const flash = useCallback((msg: string) => {
    setToast(msg);
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), TOAST_DURATION_MS);
  }, []);
  useEffect(
    () => () => {
      if (toastTimer.current) clearTimeout(toastTimer.current);
    },
    [],
  );

  // Host-key prompts arrive as backend events.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    sshEvents
      .onHostkey((e) =>
        setPrompts((q) =>
          q.some((p) => p.promptId === e.payload.promptId) ? q : [...q, e.payload],
        ),
      )
      .then((u) => {
        // If we unmounted before the listener registered, drop it now.
        if (cancelled) u();
        else unlisten = u;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const saveMutation = useMutation({
    mutationFn: (host: Host) => sshApi.hostSave(host),
    onSuccess: invalidateHosts,
    onError: (err) => flash(String(err)),
  });
  const deleteMutation = useMutation({
    mutationFn: (host: Host) => sshApi.hostDelete(host),
    onSuccess: invalidateHosts,
    onError: (err) => flash(String(err)),
  });
  const importMutation = useMutation({
    mutationFn: async (toImport: Host[]) => {
      const results = await Promise.allSettled(toImport.map((h) => sshApi.hostSave(h)));
      const failed = results.filter((r) => r.status === "rejected").length;
      return { total: toImport.length, failed };
    },
    onSuccess: ({ total, failed }) => {
      invalidateHosts();
      setImporting(null);
      const ok = total - failed;
      flash(
        failed > 0
          ? `Imported ${ok}/${total} ${plural(total, "host")} (${failed} failed)`
          : `Imported ${ok} ${plural(ok, "host")}`,
      );
    },
    onError: (err) => flash(String(err)),
  });

  const lastConnectRef = useRef<{ id: string; t: number } | null>(null);
  const connect = useCallback((host: Host) => {
    const now = Date.now();
    const last = lastConnectRef.current;
    if (last && last.id === host.id && now - last.t < CONNECT_DEBOUNCE_MS) return;
    lastConnectRef.current = { id: host.id, t: now };
    const key = crypto.randomUUID();
    setTabs((t) => [...t, { key, host, sessionId: null, closed: false }]);
    setActiveKey(key);
  }, []);

  function closeTab(key: string) {
    setTabs((t) => t.filter((tab) => tab.key !== key));
    setActiveKey((cur) => {
      if (cur !== key) return cur;
      const remaining = tabs.filter((tab) => tab.key !== key);
      return remaining.length ? remaining[remaining.length - 1].key : null;
    });
  }

  const saveHost = useCallback(
    (host: Host) => {
      saveMutation.mutate(host, { onSuccess: () => setEditing(null) });
    },
    [saveMutation],
  );
  const deleteHost = useCallback((host: Host) => deleteMutation.mutate(host), [deleteMutation]);

  const openImport = useCallback(async () => {
    try {
      const found = await sshApi.discoverHistory();
      setImporting(found);
    } catch (err) {
      flash(String(err));
    }
  }, [flash]);

  const copyCommand = useCallback(
    async (host: Host) => {
      try {
        const cmd = await sshApi.buildCommand(host.id);
        await navigator.clipboard.writeText(cmd);
        flash("Copied: " + cmd);
      } catch (err) {
        flash(String(err));
      }
    },
    [flash],
  );

  const onAddHost = useCallback(() => setEditing(emptyHost()), []);
  const onEditHost = useCallback((h: Host) => setEditing(h), []);

  async function decideHostkey(trust: boolean) {
    const current = prompts[0];
    if (!current) return;
    try {
      await sshApi.trustHostkey(current.promptId, trust);
    } catch (err) {
      flash(String(err));
    }
    setPrompts((q) => q.filter((p) => p.promptId !== current.promptId));
  }

  const activeTab = tabs.find((t) => t.key === activeKey) ?? null;

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border">
      <div className="flex min-h-0 flex-1">
        <HostList
          hosts={hosts}
          loading={isLoading}
          onConnect={connect}
          onAdd={onAddHost}
          onEdit={onEditHost}
          onDelete={deleteHost}
          onCopyCommand={copyCommand}
          onImport={openImport}
        />

        <div className="flex min-w-0 flex-1 flex-col">
          {/* tab bar */}
          <div className="flex items-center gap-1 border-b bg-muted/40 px-2 py-1.5">
            {tabs.length === 0 && (
              <span className="px-2 text-xs text-muted-foreground">
                Double-click or click a host to open a session.
              </span>
            )}
            {tabs.map((tab) => (
              <div
                key={tab.key}
                className={cn(
                  "flex items-center gap-2 rounded-md px-2.5 py-1 text-xs",
                  tab.key === activeKey ? "bg-background shadow-sm" : "hover:bg-background/60",
                )}
              >
                <button onClick={() => setActiveKey(tab.key)} className="flex items-center gap-1.5">
                  <span
                    className={cn(
                      "size-1.5 rounded-full",
                      tab.closed
                        ? "bg-muted-foreground"
                        : tab.sessionId
                          ? "bg-green-500"
                          : "bg-amber-500",
                    )}
                  />
                  {tab.host.alias}
                </button>
                <button onClick={() => closeTab(tab.key)} aria-label="Close tab">
                  <XIcon className="size-3 text-muted-foreground hover:text-foreground" />
                </button>
              </div>
            ))}
          </div>

          {/* terminals */}
          <div className="relative min-h-0 flex-1" style={{ backgroundColor: TERM_BACKGROUND }}>
            {tabs.length === 0 ? (
              <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                <Button variant="outline" size="sm" onClick={onAddHost}>
                  <PlusIcon /> Add a host to begin
                </Button>
              </div>
            ) : (
              tabs.map((tab) => (
                <div
                  key={tab.key}
                  className={cn("absolute inset-0 p-2", tab.key === activeKey ? "block" : "hidden")}
                >
                  <TerminalView
                    host={tab.host}
                    active={tab.key === activeKey}
                    onSession={(sid) =>
                      setTabs((t) =>
                        t.map((x) => (x.key === tab.key ? { ...x, sessionId: sid } : x)),
                      )
                    }
                    onClosed={() =>
                      setTabs((t) => t.map((x) => (x.key === tab.key ? { ...x, closed: true } : x)))
                    }
                    onError={flash}
                  />
                </div>
              ))
            )}
          </div>

          {activeTab && (
            <ForwardsPanel sessionId={activeTab.sessionId} host={activeTab.host} onError={flash} />
          )}
        </div>
      </div>

      {editing && <HostForm initial={editing} onSave={saveHost} onClose={() => setEditing(null)} />}
      {importing && (
        <ImportDialog
          found={importing}
          onImport={(hostsToImport) => importMutation.mutate(hostsToImport)}
          onClose={() => setImporting(null)}
        />
      )}
      {prompts.length > 0 && <HostKeyDialog prompt={prompts[0]} onDecide={decideHostkey} />}
      {toast && (
        <div className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-md bg-foreground px-3 py-1.5 text-xs text-background shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
}

export const sshRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/ssh",
  component: SshPage,
});
