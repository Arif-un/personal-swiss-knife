import { useState } from "react";
import { HistoryIcon, XIcon } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Modal } from "#components/Modal.tsx";
import { DEFAULT_SSH_PORT } from "./constants.ts";
import type { Host } from "./types.ts";

interface Props {
  found: Host[];
  onImport: (hosts: Host[]) => void;
  onClose: () => void;
}

/** Stable identity for a discovered host (selection is keyed by this, not by
 *  array index). */
function hostKey(h: Host): string {
  return `${h.user}@${h.hostname}:${h.port}`;
}

export function ImportDialog({ found, onImport, onClose }: Props) {
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(found.map(hostKey)),
  );

  function toggle(key: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  return (
    <Modal>
      <div className="flex max-h-[85vh] w-full max-w-lg flex-col overflow-hidden rounded-xl border bg-background shadow-lg">
        <div className="flex items-center justify-between border-b px-4 py-3">
          <div className="flex items-center gap-2">
            <HistoryIcon className="size-4" />
            <h2 className="text-sm font-semibold">Import from shell history</h2>
          </div>
          <Button variant="ghost" size="icon-sm" onClick={onClose} aria-label="Close">
            <XIcon />
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto px-4 py-3">
          {found.length === 0 ? (
            <p className="py-6 text-center text-sm text-muted-foreground">
              No new <code>ssh user@host</code> commands found in your shell history.
            </p>
          ) : (
            <div className="flex flex-col gap-1">
              {found.map((h) => {
                const key = hostKey(h);
                return (
                  <label
                    key={key}
                    className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-2 hover:bg-muted"
                  >
                    <input
                      type="checkbox"
                      checked={selected.has(key)}
                      onChange={() => toggle(key)}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium">
                        {h.user ? `${h.user}@` : ""}
                        {h.hostname}
                        {h.port !== DEFAULT_SSH_PORT ? `:${h.port}` : ""}
                      </div>
                      <div className="truncate text-xs text-muted-foreground">
                        alias: {h.alias}
                        {h.identityFile ? ` · key: ${h.identityFile}` : ""}
                        {h.proxyJump ? ` · jump: ${h.proxyJump}` : ""}
                      </div>
                    </div>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex items-center justify-between border-t px-4 py-3">
          <span className="text-xs text-muted-foreground">
            {found.length > 0 ? `${selected.size} of ${found.length} selected` : ""}
          </span>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onClose}>Cancel</Button>
            <Button
              disabled={selected.size === 0}
              onClick={() => onImport(found.filter((h) => selected.has(hostKey(h))))}
            >
              Import {selected.size || ""}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
