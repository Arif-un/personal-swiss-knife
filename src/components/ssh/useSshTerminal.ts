import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { sshApi, sshEvents } from "./api.ts";
import { TERM_THEME } from "./constants.ts";

interface Options {
  hostId: string;
  active: boolean;
  onSession: (sessionId: string | null) => void;
  onClosed: () => void;
  onError: (msg: string) => void;
}

/** Owns the xterm instance and its SSH session for one terminal tab: creates
 *  the terminal, wires data/close events, connects, and keeps size in sync.
 *  Returns the container ref to attach to a div. */
export function useSshTerminal({ hostId, active, onSession, onClosed, onError }: Options) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);

  // Keep the latest callbacks in refs so the connect effect can depend only on
  // hostId without capturing stale closures from the parent's re-renders.
  const onSessionRef = useRef(onSession);
  const onClosedRef = useRef(onClosed);
  const onErrorRef = useRef(onError);
  onSessionRef.current = onSession;
  onClosedRef.current = onClosed;
  onErrorRef.current = onError;

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
    } catch {
      /* container not laid out yet */
    }
    termRef.current = term;
    fitRef.current = fit;

    let disposed = false;
    let unlistenData: (() => void) | undefined;
    let unlistenClosed: (() => void) | undefined;

    (async () => {
      const ud = await sshEvents.onData((e) => {
        if (e.payload.sessionId === sessionIdRef.current) {
          term.write(new Uint8Array(e.payload.bytes));
        }
      });
      // If we unmounted while awaiting, unlisten immediately (no leak).
      if (disposed) ud();
      else unlistenData = ud;

      const uc = await sshEvents.onClosed((e) => {
        if (e.payload.sessionId === sessionIdRef.current) {
          term.writeln("\r\n\x1b[33m[connection closed]\x1b[0m");
          onClosedRef.current();
        }
      });
      if (disposed) uc();
      else unlistenClosed = uc;

      try {
        const sessionId = await sshApi.connect(hostId, term.cols, term.rows);
        if (disposed) {
          void sshApi.disconnect(sessionId);
          return;
        }
        sessionIdRef.current = sessionId;
        onSessionRef.current(sessionId);
        void sshApi.resize(sessionId, term.cols, term.rows);
        term.focus();
      } catch (err) {
        term.writeln(`\x1b[31m${String(err)}\x1b[0m`);
        onErrorRef.current(String(err));
      }
    })();

    const dataSub = term.onData((d) => {
      const id = sessionIdRef.current;
      if (id) void sshApi.write(id, d);
    });

    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch {
        /* ignore */
      }
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
  }, [hostId]);

  // Refit and focus when this tab becomes the active one.
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
      } catch {
        /* ignore */
      }
    });
    return () => cancelAnimationFrame(raf);
  }, [active]);

  return containerRef;
}
