import { useEffect, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { PlusIcon, XIcon } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { cn } from "#lib/utils.ts";
import { sshApi } from "#components/ssh/api.ts";
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

function SshPage() {
  const qc = useQueryClient();
  const { data: hosts = [], isLoading } = useQuery<Host[]>({
    queryKey: ["ssh-hosts"],
    queryFn: () => sshApi.hostsList(),
  });

  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [editing, setEditing] = useState<Host | null>(null);
  const [importing, setImporting] = useState<Host[] | null>(null);
  const [prompt, setPrompt] = useState<HostKeyPrompt | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<HostKeyPrompt>("ssh://hostkey", (e) => setPrompt(e.payload)).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, []);

  function flash(msg: string) {
    setToast(msg);
    setTimeout(() => setToast(null), 2000);
  }

  function connect(host: Host) {
    const key = crypto.randomUUID();
    setTabs((t) => [...t, { key, host, sessionId: null, closed: false }]);
    setActiveKey(key);
  }

  function closeTab(key: string) {
    setTabs((t) => t.filter((tab) => tab.key !== key));
    setActiveKey((cur) => {
      if (cur !== key) return cur;
      const remaining = tabs.filter((tab) => tab.key !== key);
      return remaining.length ? remaining[remaining.length - 1].key : null;
    });
  }

  async function saveHost(host: Host) {
    try {
      await sshApi.hostSave(host);
      setEditing(null);
      qc.invalidateQueries({ queryKey: ["ssh-hosts"] });
    } catch (err) {
      flash(String(err));
    }
  }

  async function deleteHost(host: Host) {
    try {
      await sshApi.hostDelete(host);
      qc.invalidateQueries({ queryKey: ["ssh-hosts"] });
    } catch (err) {
      flash(String(err));
    }
  }

  async function openImport() {
    try {
      const found = await sshApi.discoverHistory();
      setImporting(found);
    } catch (err) {
      flash(String(err));
    }
  }

  async function importHosts(hosts: Host[]) {
    try {
      for (const h of hosts) await sshApi.hostSave(h);
      setImporting(null);
      qc.invalidateQueries({ queryKey: ["ssh-hosts"] });
      flash(`Imported ${hosts.length} host${hosts.length === 1 ? "" : "s"}`);
    } catch (err) {
      flash(String(err));
    }
  }

  async function copyCommand(host: Host) {
    try {
      const cmd = await sshApi.buildCommand(host.id);
      await navigator.clipboard.writeText(cmd);
      flash("Copied: " + cmd);
    } catch (err) {
      flash(String(err));
    }
  }

  async function decideHostkey(trust: boolean) {
    if (!prompt) return;
    await sshApi.trustHostkey(prompt.promptId, trust).catch(() => {});
    setPrompt(null);
  }

  const activeTab = tabs.find((t) => t.key === activeKey) ?? null;

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border">
      <div className="flex min-h-0 flex-1">
        <HostList
          hosts={hosts}
          loading={isLoading}
          onConnect={connect}
          onAdd={() => setEditing(emptyHost())}
          onEdit={(h) => setEditing(h)}
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
                      tab.closed ? "bg-muted-foreground" : tab.sessionId ? "bg-green-500" : "bg-amber-500",
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
          <div className="relative min-h-0 flex-1 bg-[#181825]">
            {tabs.length === 0 ? (
              <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                <Button variant="outline" size="sm" onClick={() => setEditing(emptyHost())}>
                  <PlusIcon /> Add a host to begin
                </Button>
              </div>
            ) : (
              tabs.map((tab) => (
                <div key={tab.key} className={cn("absolute inset-0 p-2", tab.key === activeKey ? "block" : "hidden")}>
                  <TerminalView
                    host={tab.host}
                    active={tab.key === activeKey}
                    onSession={(sid) =>
                      setTabs((t) => t.map((x) => (x.key === tab.key ? { ...x, sessionId: sid } : x)))
                    }
                    onClosed={() =>
                      setTabs((t) => t.map((x) => (x.key === tab.key ? { ...x, closed: true } : x)))
                    }
                    onError={(msg) => flash(msg)}
                  />
                </div>
              ))
            )}
          </div>

          {activeTab && <ForwardsPanel sessionId={activeTab.sessionId} host={activeTab.host} />}
        </div>
      </div>

      {editing && (
        <HostForm initial={editing} onSave={saveHost} onClose={() => setEditing(null)} />
      )}
      {importing && (
        <ImportDialog found={importing} onImport={importHosts} onClose={() => setImporting(null)} />
      )}
      {prompt && <HostKeyDialog prompt={prompt} onDecide={decideHostkey} />}
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
