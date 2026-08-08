import { invoke } from "@tauri-apps/api/core";
import { createRoute } from "@tanstack/react-router";
import { ExternalLinkIcon, MessageCircleIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "#components/ui/button.tsx";
import { rootRoute } from "./__root.tsx";

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
