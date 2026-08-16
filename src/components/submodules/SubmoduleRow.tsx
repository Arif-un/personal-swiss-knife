import { useMemo, useState } from "react";
import { Check, ChevronsUpDown, GitBranch, Terminal } from "lucide-react";
import { FaGithub } from "react-icons/fa";
import { VscVscode } from "react-icons/vsc";
import { Input } from "#components/ui/input.tsx";
import { Button } from "#components/ui/button.tsx";
import { Badge } from "#components/ui/badge.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "#components/ui/tooltip.tsx";
import { TableCell, TableRow } from "#components/ui/table.tsx";
import { type DirtyAction, type RepoRow } from "#components/submodules/api.ts";

/** One repo row. The branch cell is a searchable dropdown that confirms the switch inline. */
export function SubmoduleRow({
  row,
  rootName,
  onSwitch,
  onOpenApp,
  switching,
}: {
  row: RepoRow;
  rootName: string;
  onSwitch: (sub: string, branch: string, action: DirtyAction) => void;
  onOpenApp: (sub: string, app: "github" | "vscode" | "terminal") => void;
  switching: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  // The branch picked in the list, awaiting confirmation. `null` = list step.
  const [pending, setPending] = useState<string | null>(null);
  // Checked = carry uncommitted changes; unchecked = stash them.
  const [bring, setBring] = useState(false);

  const sub = row.isParent ? "" : row.name;

  const matches = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q ? row.branches.filter((b) => b.toLowerCase().includes(q)) : row.branches;
  }, [row.branches, filter]);

  // Reset the popover's transient state whenever it closes.
  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      setFilter("");
      setPending(null);
      setBring(false);
    }
  }

  function pick(b: string) {
    if (b === row.branch) return; // already here, nothing to switch
    setPending(b);
  }

  function confirm() {
    if (!pending) return;
    onOpenChange(false);
    const action: DirtyAction = row.dirty ? (bring ? "carry" : "stash") : "none";
    onSwitch(sub, pending, action);
  }

  return (
    <TableRow>
      <TableCell className="font-medium">
        <div className="flex flex-col gap-1">
          <span className="flex items-center gap-1.5">
            {row.isParent ? (
              <>
                <GitBranch className="size-3.5 text-muted-foreground" />
                {rootName}
                <Badge variant="secondary">parent</Badge>
              </>
            ) : (
              row.name
            )}
          </span>
          <span className="flex items-center gap-2">
            {row.dirty ? (
              <span className="rounded-full bg-destructive/15 px-1.5 py-0.5 text-[10px] font-medium text-destructive">
                dirty
              </span>
            ) : (
              <span className="rounded-full bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                clean
              </span>
            )}
            {row.ahead !== null && row.behind !== null && (
              <span className="font-mono text-[10px] text-muted-foreground">
                ↑{row.ahead} ↓{row.behind}
              </span>
            )}
          </span>
        </div>
      </TableCell>

      <TableCell>
        <div className="flex items-center gap-1.5">
          {row.error ? (
            <span className="text-sm text-destructive" title={row.error}>
              {row.error}
            </span>
          ) : (
            <Popover open={open} onOpenChange={onOpenChange}>
              <PopoverTrigger
                disabled={switching}
                render={
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 max-w-64 justify-between gap-1.5 px-2 font-mono text-xs"
                  >
                    <span
                      className="min-w-0 truncate"
                      title={row.detached ? row.headDesc : row.branch}
                    >
                      {row.detached ? `detached · ${row.headDesc || "?"}` : row.branch || "—"}
                    </span>
                    <ChevronsUpDown className="size-3.5 shrink-0 opacity-50" />
                  </Button>
                }
              />
              <PopoverContent align="start" className="w-72 p-0">
                {pending === null ? (
                  <div className="flex flex-col">
                    <Input
                      autoFocus
                      value={filter}
                      onChange={(e) => setFilter(e.target.value)}
                      placeholder="Search branches…"
                      className="m-1 h-8 border-0 shadow-none focus-visible:ring-0"
                    />
                    <div className="max-h-64 overflow-y-auto border-t p-1">
                      {matches.length === 0 ? (
                        <p className="px-2 py-1.5 text-sm text-muted-foreground">No branches.</p>
                      ) : (
                        matches.map((b) => (
                          <button
                            key={b}
                            type="button"
                            onClick={() => pick(b)}
                            className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left font-mono text-sm hover:bg-accent"
                          >
                            <Check
                              className={b === row.branch ? "size-3.5" : "size-3.5 opacity-0"}
                            />
                            <span className="truncate">{b}</span>
                          </button>
                        ))
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="flex flex-col gap-3 p-3">
                    <p className="text-sm">
                      Switch to <b className="font-mono">{pending}</b>?
                    </p>
                    {row.dirty && (
                      <label className="flex items-start gap-2 text-sm">
                        <input
                          type="checkbox"
                          checked={bring}
                          onChange={(e) => setBring(e.target.checked)}
                          className="mt-0.5"
                        />
                        <span>
                          Bring uncommitted changes along
                          <span className="block text-xs text-muted-foreground">
                            {bring
                              ? "Carried to the new branch."
                              : "Otherwise stashed on this branch."}
                          </span>
                        </span>
                      </label>
                    )}
                    <div className="flex justify-end gap-2">
                      <Button size="sm" variant="outline" onClick={() => setPending(null)}>
                        Cancel
                      </Button>
                      <Button size="sm" onClick={confirm} disabled={switching}>
                        Switch
                      </Button>
                    </div>
                  </div>
                )}
              </PopoverContent>
            </Popover>
          )}
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onOpenApp(sub, "github")}
                  aria-label="Open in GitHub Desktop"
                >
                  <FaGithub />
                </Button>
              }
            />
            <TooltipContent>Open in GitHub Desktop</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onOpenApp(sub, "vscode")}
                  aria-label="Open in VS Code"
                >
                  <VscVscode />
                </Button>
              }
            />
            <TooltipContent>Open in VS Code</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onOpenApp(sub, "terminal")}
                  aria-label="Open in Terminal"
                >
                  <Terminal />
                </Button>
              }
            />
            <TooltipContent>Open in Terminal</TooltipContent>
          </Tooltip>
        </div>
      </TableCell>
    </TableRow>
  );
}
