import { useState } from "react";
import {
  Check,
  Eye,
  EyeOff,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Settings,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { cn } from "#lib/utils.ts";
import type { PrView } from "./types.ts";

interface ViewsMenuProps {
  views: PrView[];
  activeViewId: string | null;
  /** Whether a repo has been fetched, so "save current" makes sense. */
  canSaveCurrent: boolean;
  busy?: boolean;
  /** Whether the repo input + Fetch + Filters row is currently shown. */
  topBarVisible: boolean;
  onToggleTopBar: () => void;
  onApply: (view: PrView) => void;
  onSaveCurrent: (name: string) => void;
  onUpdate: (view: PrView) => void;
  onRename: (view: PrView, name: string) => void;
  onDelete: (view: PrView) => void;
}

export function ViewsMenu({
  views,
  activeViewId,
  canSaveCurrent,
  busy,
  topBarVisible,
  onToggleTopBar,
  onApply,
  onSaveCurrent,
  onUpdate,
  onRename,
  onDelete,
}: ViewsMenuProps) {
  const [open, setOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  function startRename(view: PrView) {
    setRenamingId(view.id);
    setRenameValue(view.name);
  }

  function commitRename(view: PrView) {
    const name = renameValue.trim();
    if (name && name !== view.name) onRename(view, name);
    setRenamingId(null);
  }

  function saveCurrent() {
    const name = newName.trim();
    if (!name) return;
    onSaveCurrent(name);
    setNewName("");
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={<Button variant="ghost" size="icon-sm" aria-label="Pull request views" />}
      >
        {busy ? <Loader2 className="animate-spin" /> : <Settings />}
      </PopoverTrigger>
      <PopoverContent className="w-80 p-0">
        <div className="flex items-center justify-between border-b px-3 py-2">
          <span className="text-sm font-medium">Views</span>
          {busy && <Loader2 className="size-3.5 animate-spin text-muted-foreground" />}
        </div>

        <button
          type="button"
          onClick={onToggleTopBar}
          className="flex w-full items-center gap-2 border-b px-3 py-2 text-left text-sm hover:bg-accent"
        >
          {topBarVisible ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
          {topBarVisible ? "Hide search bar" : "Show search bar"}
        </button>

        <div className="max-h-72 overflow-y-auto py-1">
          {views.length === 0 && (
            <p className="px-3 py-3 text-center text-xs text-muted-foreground">
              No saved views yet. Fetch a repo, then save the current repo and filters below.
            </p>
          )}
          {views.map((view) =>
            renamingId === view.id ? (
              <div key={view.id} className="flex items-center gap-1 px-2 py-1">
                <Input
                  autoFocus
                  value={renameValue}
                  onChange={(e) => setRenameValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitRename(view);
                    if (e.key === "Escape") setRenamingId(null);
                  }}
                  className="h-7"
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Confirm rename"
                  onClick={() => commitRename(view)}
                >
                  <Check />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Cancel rename"
                  onClick={() => setRenamingId(null)}
                >
                  <X />
                </Button>
              </div>
            ) : (
              <div key={view.id} className="group/view flex items-center gap-1 px-2">
                <button
                  type="button"
                  onClick={() => onApply(view)}
                  className={cn(
                    "flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent",
                    view.id === activeViewId && "font-medium",
                  )}
                >
                  <Check
                    className={cn(
                      "size-3.5 shrink-0",
                      view.id === activeViewId ? "opacity-100" : "opacity-0",
                    )}
                  />
                  <span className="min-w-0 flex-1 truncate">{view.name}</span>
                  <span className="shrink-0 truncate text-xs text-muted-foreground">
                    {view.repo}
                  </span>
                </button>
                <div className="flex shrink-0 items-center opacity-0 transition-opacity group-hover/view:opacity-100">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Update view to current filters"
                    title="Update to current repo & filters"
                    disabled={!canSaveCurrent}
                    onClick={() => onUpdate(view)}
                  >
                    <RefreshCw />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Rename view"
                    title="Rename"
                    onClick={() => startRename(view)}
                  >
                    <Pencil />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Delete view"
                    title="Delete"
                    onClick={() => onDelete(view)}
                  >
                    <Trash2 />
                  </Button>
                </div>
              </div>
            ),
          )}
        </div>

        <div className="flex items-center gap-2 border-t p-2">
          <Input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") saveCurrent();
            }}
            placeholder={canSaveCurrent ? "New view name" : "Fetch a repo first"}
            disabled={!canSaveCurrent}
            className="h-7"
          />
          <Button size="sm" disabled={!canSaveCurrent || !newName.trim()} onClick={saveCurrent}>
            <Plus />
            Save
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
