import { useState } from "react";
import { Trash2Icon } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { TableCell, TableRow } from "#components/ui/table.tsx";
import { MODE_LABELS, type DevkonEntry, type DevkonMode } from "#components/devkon/api.ts";
import { StatusCell } from "#components/deploy/StatusCell.tsx";

export const BRANCH_LIST_ID = "devkon-branches";

/** Resolve a namespace URL from the configured `{name}` template. Prepend
 * https:// unless the value is already an http(s) URL. Only http(s) is honored as
 * a pre-existing scheme: allowing any `scheme://` let a crafted `javascript:` value
 * (restorable from an untrusted settings backup) run as an href in this
 * csp:null webview. Anything else is forced under https://. */
export function accessUrl(template: string, name: string) {
  const url = template.split("{name}").join(name);
  return /^https?:\/\//i.test(url) ? url : `https://${url}`;
}

/** One editable row. Branch is controlled local state so a mode change reads the
 * currently-typed branch instead of a stale render closure (which would revert a
 * just-typed branch, since devkon_save overwrites name/branch/mode wholesale). */
export function DeployRow({
  entry,
  clusterDomain,
  busy,
  onSave,
  onDeploy,
  onDestroy,
  onRemove,
  removePending,
}: {
  entry: DevkonEntry;
  /** URL template with `{name}`; empty = no link shown. */
  clusterDomain: string;
  busy: boolean;
  onSave: (e: DevkonEntry) => void;
  onDeploy: () => void;
  onDestroy: () => void;
  onRemove: () => void;
  removePending: boolean;
}) {
  const [branch, setBranch] = useState(entry.branch);
  // Local mode too (same reason as branch): the confirm and the backend dispatch
  // must agree on what's about to run. The backend re-reads mode from disk, and the
  // `entry` prop lags the save's refetch, so gating the wipe-confirm on the prop let
  // a fast select-clean-then-Deploy skip the prompt. Local state is set synchronously
  // on change, so the confirm always reflects the mode the user actually picked.
  const [mode, setMode] = useState(entry.mode);
  const saveBranch = () => {
    const b = branch.trim();
    if (b !== entry.branch) onSave({ ...entry, branch: b });
  };
  return (
    <TableRow>
      <TableCell className="font-medium align-top">
        <div className="flex flex-col gap-0.5">
          {entry.name}
          {clusterDomain.trim() && (
            <a
              href={accessUrl(clusterDomain, entry.name)}
              target="_blank"
              rel="noreferrer"
              className="text-xs text-muted-foreground hover:text-foreground truncate max-w-[16rem]"
            >
              {accessUrl(clusterDomain, entry.name).replace(/^https?:\/\//, "")}
            </a>
          )}
        </div>
      </TableCell>

      <TableCell className="align-top">
        <Input
          list={BRANCH_LIST_ID}
          value={branch}
          onChange={(e) => setBranch(e.target.value)}
          onBlur={saveBranch}
          placeholder="branch…"
          className="h-7 w-44"
        />
      </TableCell>

      <TableCell className="align-top">
        <select
          value={mode}
          onChange={(e) => {
            const m = e.target.value as DevkonMode;
            setMode(m);
            onSave({ ...entry, branch: branch.trim(), mode: m });
          }}
          className="h-7 rounded-lg border bg-background px-2 text-sm"
        >
          {(Object.keys(MODE_LABELS) as DevkonMode[]).map((m) => (
            <option key={m} value={m}>
              {MODE_LABELS[m]}
            </option>
          ))}
        </select>
      </TableCell>

      <TableCell className="align-top">
        <StatusCell entry={entry} />
      </TableCell>

      <TableCell className="align-top">
        <div className="flex items-center justify-end gap-1.5">
          <Button
            size="sm"
            disabled={busy || !branch.trim()}
            onClick={() => {
              // Clean modes tear down and recreate the namespace (wiping its data),
              // so confirm - matching Destroy/Remove - instead of a silent one-click.
              if (
                mode.startsWith("clean") &&
                !window.confirm(
                  `Clean redeploy tears down and recreates the "${entry.name}" namespace, wiping its data. Continue?`,
                )
              )
                return;
              onDeploy();
            }}
          >
            {busy ? "Dispatching…" : "Deploy"}
          </Button>
          <Button
            size="sm"
            variant="destructive"
            disabled={busy}
            onClick={() => {
              if (
                window.confirm(
                  `Destroy the "${entry.name}" namespace? This tears down the deployment.`,
                )
              )
                onDestroy();
            }}
          >
            {busy ? "Dispatching…" : "Destroy"}
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            title="Remove from list"
            disabled={busy || removePending}
            onClick={() => {
              if (window.confirm(`Remove "${entry.name}" from the list?`)) onRemove();
            }}
          >
            <Trash2Icon />
          </Button>
        </div>
      </TableCell>
    </TableRow>
  );
}
