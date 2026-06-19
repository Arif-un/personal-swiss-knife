import { useEffect, useState } from "react";
import { PlayIcon, SquareIcon } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { sshApi } from "./api.ts";
import type { ForwardSpec, Host } from "./types.ts";

interface Props {
  sessionId: string | null;
  host: Host;
}

export function ForwardsPanel({ sessionId, host }: Props) {
  // Map "bindPort:destHost:destPort" -> active forwardId
  const [active, setActive] = useState<Record<string, string>>({});

  useEffect(() => {
    setActive({});
    if (!sessionId) return;
    sshApi.forwardsList(sessionId).then((list) => {
      const next: Record<string, string> = {};
      for (const f of list) next[keyOf(f.spec)] = f.id;
      setActive(next);
    }).catch(() => {});
  }, [sessionId]);

  if (host.forwards.length === 0) return null;

  async function toggle(spec: ForwardSpec) {
    if (!sessionId) return;
    const k = keyOf(spec);
    const existing = active[k];
    if (existing) {
      await sshApi.forwardStop(sessionId, existing).catch(() => {});
      setActive((p) => {
        const n = { ...p };
        delete n[k];
        return n;
      });
    } else {
      try {
        const id = await sshApi.forwardStart(sessionId, spec);
        setActive((p) => ({ ...p, [k]: id }));
      } catch { /* bind may fail (port in use) */ }
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-2 border-t px-3 py-2">
      <span className="text-xs font-medium text-muted-foreground">Tunnels</span>
      {host.forwards.map((f, i) => {
        const on = Boolean(active[keyOf(f)]);
        return (
          <Button
            key={i}
            variant={on ? "default" : "outline"}
            size="xs"
            disabled={!sessionId}
            onClick={() => toggle(f)}
          >
            {on ? <SquareIcon /> : <PlayIcon />}
            -L {f.bindPort}→{f.destHost}:{f.destPort}
          </Button>
        );
      })}
    </div>
  );
}

function keyOf(f: ForwardSpec): string {
  return `${f.bindAddr}:${f.bindPort}:${f.destHost}:${f.destPort}`;
}
