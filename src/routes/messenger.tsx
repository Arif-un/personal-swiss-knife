import { invoke } from "@tauri-apps/api/core";
import { createRoute } from "@tanstack/react-router";
import { MessageCircleIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "#components/ui/button.tsx";
import { rootRoute } from "./__root.tsx";
import { AutoCollapseRow, MuteRow, ShortcutRow } from "#components/messenger/SettingRows.tsx";
import { LinkRoutingSection } from "#components/messenger/LinkRouting.tsx";

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

      <LinkRoutingSection />

      {error && <p className="text-sm text-destructive">{error}</p>}
    </div>
  );
}

export const messengerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/messenger",
  component: MessengerPage,
});
