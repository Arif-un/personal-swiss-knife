import { invoke } from "@tauri-apps/api/core";
import { createRoute } from "@tanstack/react-router";
import { ExternalLinkIcon, KeyboardIcon, MessageCircleIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "#components/ui/button.tsx";
import { rootRoute } from "./__root.tsx";

const ACCEL_SYM: Record<string, string> = { CmdOrCtrl: "⌘", Cmd: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };
const ARROW: Record<string, string> = { ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→" };

/** Render a Tauri accelerator string (e.g. "CmdOrCtrl+Shift+KeyM") as symbols. */
function prettyAccel(accel: string): string {
  return accel
    .split("+")
    .map(
      (p) =>
        ACCEL_SYM[p] ??
        ARROW[p] ??
        p.replace(/^(Key|Digit)/, "").replace("Escape", "Esc"),
    )
    .join(" ");
}

/** Build a Tauri accelerator from a keydown, or null if it isn't a bindable
 *  global combo. Needs at least one modifier (bare keys would swallow that key
 *  system-wide). Plain Escape cancels recording. */
function toAccel(e: KeyboardEvent): string | null {
  const code = e.code;
  if (/^(Meta|Control|Alt|Shift)(Left|Right)$/.test(code)) return null; // lone modifier
  const mods: string[] = [];
  if (e.metaKey) mods.push("Cmd");
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null; // require a modifier
  return [...mods, code].join("+");
}

function ShortcutRecorder() {
  const [accel, setAccel] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("messenger_get_shortcut")
      .then(setAccel)
      .catch((e) => setError(String(e)));
  }, []);

  // Capture keys at the document level while recording: macOS WKWebView doesn't
  // move focus to a <button> on click, so a button-scoped keydown handler would
  // never fire. Capture phase so we win before the page's own handlers.
  useEffect(() => {
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      // Plain Escape cancels without rebinding.
      if (e.code === "Escape" && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        setRecording(false);
        return;
      }
      const next = toAccel(e);
      if (!next) return; // wait for a full modifier+key combo
      setRecording(false);
      invoke("messenger_set_shortcut", { accelerator: next })
        .then(() => {
          setAccel(next);
          setError(null);
        })
        .catch((err) => setError(String(err)));
    };
    document.addEventListener("keydown", handler, true);
    return () => document.removeEventListener("keydown", handler, true);
  }, [recording]);

  return (
    <div className="rounded-lg border p-3 text-sm">
      <div className="mb-1 flex items-center gap-2 font-medium">
        <KeyboardIcon className="size-4 text-muted-foreground" />
        Collapse / expand shortcut
      </div>
      <p className="mb-3 text-muted-foreground">
        Works from anywhere to toggle the window between full and bubble. Plain Esc also collapses
        while the window is focused.
      </p>
      <div className="flex items-center gap-3">
        <Button
          variant="outline"
          className="min-w-32 font-mono"
          onClick={() => {
            setError(null);
            setRecording((r) => !r);
          }}
        >
          {recording ? "Press keys…" : accel ? prettyAccel(accel) : "Set shortcut"}
        </Button>
        {recording && <span className="text-xs text-muted-foreground">Esc to cancel</span>}
      </div>
      {error && <p className="mt-2 text-xs text-destructive">{error}</p>}
    </div>
  );
}

/** Launcher/control panel for the Messenger webview. The chat itself lives in a
 *  separate native window (lighter than a browser tab); this page just opens,
 *  focuses, and frees it. */
function MessengerPage() {
  const [error, setError] = useState<string | null>(null);

  const open = () => {
    setError(null);
    invoke("messenger_open").catch((e) => setError(String(e)));
  };
  const closeAndFree = () => {
    setError(null);
    invoke("messenger_close").catch((e) => setError(String(e)));
  };

  // Open (or focus) the window as soon as the page is visited.
  useEffect(() => {
    open();
  }, []);

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <MessageCircleIcon className="size-5 text-muted-foreground" />
        <h1 className="text-xl font-semibold">Messenger</h1>
      </div>

      <p className="text-sm text-muted-foreground">
        Messenger runs in its own native window to keep RAM low. Closing that window keeps it warm
        for an instant reopen. Use "Close & free RAM" to fully release it.
      </p>

      <div className="flex gap-2">
        <Button onClick={open}>Open / Focus</Button>
        <Button variant="destructive" onClick={closeAndFree}>
          Close &amp; free RAM
        </Button>
      </div>

      <ShortcutRecorder />

      <div className="rounded-lg border p-3 text-sm text-muted-foreground">
        <p className="mb-2 font-medium text-foreground">Links inside chats</p>
        <ul className="flex flex-col gap-1.5">
          <li className="flex items-center gap-2">
            <ExternalLinkIcon className="size-3.5 shrink-0" />
            Click a link &rarr; opens in a reusable preview window.
          </li>
          <li className="flex items-center gap-2">
            <ExternalLinkIcon className="size-3.5 shrink-0" />
            Shift-click a link &rarr; opens in your default browser.
          </li>
        </ul>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}
    </div>
  );
}

export const messengerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/messenger",
  component: MessengerPage,
});
