import { invoke } from "@tauri-apps/api/core";
import { createRoute } from "@tanstack/react-router";
import {
  BellOffIcon,
  ExternalLinkIcon,
  KeyboardIcon,
  MessageCircleIcon,
  TimerIcon,
} from "lucide-react";
import type { ComponentType } from "react";
import { useEffect, useState } from "react";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
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

/** One compact settings row: icon + label + hint on the left, control(s) on the
 *  right. Rows stack inside a single bordered card (divide-y) for density. */
function SettingRow({
  icon: Icon,
  label,
  hint,
  error,
  children,
}: {
  icon: ComponentType<{ className?: string }>;
  label: string;
  hint?: string;
  error?: string | null;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2 text-sm">
      <div className="min-w-0">
        <div className="flex items-center gap-2 font-medium">
          <Icon className="size-4 shrink-0 text-muted-foreground" />
          <span className="truncate">{label}</span>
        </div>
        {hint && <p className="mt-0.5 text-xs text-muted-foreground">{hint}</p>}
        {error && <p className="mt-0.5 text-xs text-destructive">{error}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

function MuteRow() {
  const [muted, setMuted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("messenger_get_muted")
      .then(setMuted)
      .catch((e) => setError(String(e)));
  }, []);

  const toggle = () => {
    const next = !muted;
    setMuted(next);
    invoke("messenger_set_muted", { muted: next }).catch((e) => setError(String(e)));
  };

  return (
    <SettingRow
      icon={BellOffIcon}
      label="Mute badge"
      hint="Hide the unread count on the bubble."
      error={error}
    >
      <Button variant={muted ? "default" : "outline"} size="sm" className="min-w-20" onClick={toggle}>
        {muted ? "Muted" : "Showing"}
      </Button>
    </SettingRow>
  );
}

function AutoCollapseRow() {
  const [secs, setSecs] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<number>("messenger_get_idle_secs")
      .then((s) => setSecs(String(s)))
      .catch((e) => setError(String(e)));
  }, []);

  const save = () => {
    const n = Math.max(0, Math.round(Number(secs) || 0));
    setSecs(String(n));
    invoke("messenger_set_idle_secs", { secs: n })
      .then(() => {
        setError(null);
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1500);
      })
      .catch((e) => setError(String(e)));
  };

  return (
    <SettingRow
      icon={TimerIcon}
      label="Auto-collapse"
      hint="Collapse to bubble after N seconds unfocused. 0 = off."
      error={error}
    >
      <Input
        type="number"
        min={0}
        className="h-7 w-20"
        value={secs}
        onChange={(e) => setSecs(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => e.key === "Enter" && save()}
      />
      <span className="text-xs text-muted-foreground">{saved ? "✓ sec" : "sec"}</span>
    </SettingRow>
  );
}

function ShortcutRow() {
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
    <SettingRow
      icon={KeyboardIcon}
      label="Toggle shortcut"
      hint="Global hotkey to collapse/expand. Plain Esc also collapses when focused."
      error={error}
    >
      {recording && <span className="text-xs text-muted-foreground">Esc cancels</span>}
      <Button
        variant="outline"
        size="sm"
        className="min-w-28 font-mono"
        onClick={() => {
          setError(null);
          setRecording((r) => !r);
        }}
      >
        {recording ? "Press keys…" : accel ? prettyAccel(accel) : "Set shortcut"}
      </Button>
    </SettingRow>
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
    <div className="flex max-w-xl flex-col gap-3">
      <div className="flex items-center gap-2">
        <MessageCircleIcon className="size-5 text-muted-foreground" />
        <h1 className="text-xl font-semibold">Messenger</h1>
      </div>

      <p className="text-sm text-muted-foreground">
        Runs in its own native window to keep RAM low. Closing that window keeps it warm for an
        instant reopen. "Close &amp; free RAM" fully releases it.
      </p>

      <div className="flex gap-2">
        <Button size="sm" onClick={open}>
          Open / Focus
        </Button>
        <Button size="sm" variant="destructive" onClick={closeAndFree}>
          Close &amp; free RAM
        </Button>
      </div>

      <div className="divide-y rounded-lg border">
        <MuteRow />
        <AutoCollapseRow />
        <ShortcutRow />
      </div>

      <div className="rounded-lg border px-3 py-2 text-sm text-muted-foreground">
        <p className="mb-1.5 font-medium text-foreground">Links inside chats</p>
        <ul className="flex flex-col gap-1">
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
