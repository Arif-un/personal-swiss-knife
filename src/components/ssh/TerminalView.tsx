import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import { sshApi } from "./api.ts";
import type { Host, SshClosedEvent, SshDataEvent } from "./types.ts";

interface Props {
  host: Host;
  active: boolean;
  onSession: (sessionId: string | null) => void;
  onClosed: () => void;
  onError: (msg: string) => void;
}

const TERM_THEME = {
  background: "#181825",
  foreground: "#cdd6f4",
  cursor: "#cdd6f4",
  selectionBackground: "#414458",
};

export function TerminalView({ host, active, onSession, onClosed, onError }: Props) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new XTerm({
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
      fontSize: 13,
      cursorBlink: true,
      theme: TERM_THEME,
      allowProposedApi: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(container);
    try {
      fit.fit();
    } catch { /* container not laid out yet */ }
    termRef.current = term;
    fitRef.current = fit;

    let unlistenData: (() => void) | undefined;
    let unlistenClosed: (() => void) | undefined;
    let disposed = false;

    (async () => {
      unlistenData = await listen<SshDataEvent>("ssh://data", (e) => {
        if (e.payload.sessionId === sessionIdRef.current) {
          term.write(new Uint8Array(e.payload.bytes));
        }
      });
      unlistenClosed = await listen<SshClosedEvent>("ssh://closed", (e) => {
        if (e.payload.sessionId === sessionIdRef.current) {
          term.writeln("\r\n\x1b[33m[connection closed]\x1b[0m");
          onClosed();
        }
      });

      try {
        const sessionId = await sshApi.connect(host.id, term.cols, term.rows);
        if (disposed) {
          void sshApi.disconnect(sessionId);
          return;
        }
        sessionIdRef.current = sessionId;
        onSession(sessionId);
        void sshApi.resize(sessionId, term.cols, term.rows);
        term.focus();
      } catch (err) {
        term.writeln(`\x1b[31m${String(err)}\x1b[0m`);
        onError(String(err));
      }
    })();

    const dataSub = term.onData((d) => {
      const id = sessionIdRef.current;
      if (id) void sshApi.write(id, d);
    });

    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch { /* ignore */ }
      const id = sessionIdRef.current;
      if (id) void sshApi.resize(id, term.cols, term.rows);
    });
    ro.observe(container);

    return () => {
      disposed = true;
      ro.disconnect();
      dataSub.dispose();
      unlistenData?.();
      unlistenClosed?.();
      const id = sessionIdRef.current;
      if (id) void sshApi.disconnect(id);
      term.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host.id]);

  useEffect(() => {
    if (!active) return;
    const term = termRef.current;
    const fit = fitRef.current;
    if (!term || !fit) return;
    const raf = requestAnimationFrame(() => {
      try {
        fit.fit();
        const id = sessionIdRef.current;
        if (id) void sshApi.resize(id, term.cols, term.rows);
        term.focus();
      } catch { /* ignore */ }
    });
    return () => cancelAnimationFrame(raf);
  }, [active]);

  return <div ref={containerRef} className="h-full w-full overflow-hidden" />;
}
