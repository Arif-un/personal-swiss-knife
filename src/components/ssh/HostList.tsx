import {
  ArrowRightLeftIcon,
  CopyIcon,
  PencilIcon,
  PlusIcon,
  ServerIcon,
  Trash2Icon,
  WaypointsIcon,
} from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Badge } from "#components/ui/badge.tsx";
import type { Host } from "./types.ts";

interface Props {
  hosts: Host[];
  loading: boolean;
  onConnect: (host: Host) => void;
  onAdd: () => void;
  onEdit: (host: Host) => void;
  onDelete: (host: Host) => void;
  onCopyCommand: (host: Host) => void;
}

export function HostList({ hosts, loading, onConnect, onAdd, onEdit, onDelete, onCopyCommand }: Props) {
  return (
    <div className="flex h-full w-64 shrink-0 flex-col border-r">
      <div className="flex items-center justify-between px-3 py-3">
        <span className="text-sm font-semibold">Hosts</span>
        <Button variant="outline" size="icon-xs" onClick={onAdd} aria-label="Add host">
          <PlusIcon />
        </Button>
      </div>

      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {loading && <p className="px-2 py-3 text-xs text-muted-foreground">Loading…</p>}
        {!loading && hosts.length === 0 && (
          <p className="px-2 py-3 text-xs text-muted-foreground">
            No hosts. Add one or define hosts in <code>~/.ssh/config</code>.
          </p>
        )}

        {hosts.map((host) => (
          <div
            key={host.id}
            className="group mb-1 rounded-lg px-2 py-2 hover:bg-muted"
            onDoubleClick={() => onConnect(host)}
          >
            <div className="flex items-center gap-2">
              <ServerIcon className="size-4 text-muted-foreground" />
              <button
                className="flex-1 truncate text-left text-sm font-medium hover:underline"
                onClick={() => onConnect(host)}
                title="Connect"
              >
                {host.alias}
              </button>
            </div>
            <div className="ml-6 truncate text-xs text-muted-foreground">
              {host.user ? `${host.user}@` : ""}
              {host.hostname || host.alias}
            </div>

            <div className="ml-6 mt-1 flex flex-wrap items-center gap-1">
              <Badge variant="secondary" className="text-[10px]">
                {host.source === "ssh-config" ? "ssh-config" : "app"}
              </Badge>
              {host.proxyJump && (
                <Badge variant="outline" className="gap-0.5 text-[10px]">
                  <WaypointsIcon className="size-2.5" /> jump
                </Badge>
              )}
              {host.forwards.length > 0 && (
                <Badge variant="outline" className="gap-0.5 text-[10px]">
                  <ArrowRightLeftIcon className="size-2.5" /> {host.forwards.length}
                </Badge>
              )}
            </div>

            <div className="ml-5 mt-1 flex gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
              <Button variant="ghost" size="icon-xs" onClick={() => onCopyCommand(host)} aria-label="Copy ssh command">
                <CopyIcon />
              </Button>
              <Button variant="ghost" size="icon-xs" onClick={() => onEdit(host)} aria-label="Edit host">
                <PencilIcon />
              </Button>
              <Button variant="ghost" size="icon-xs" onClick={() => onDelete(host)} aria-label="Delete host">
                <Trash2Icon />
              </Button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
