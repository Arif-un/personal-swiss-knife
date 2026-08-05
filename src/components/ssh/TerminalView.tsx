import { useSshTerminal } from "./useSshTerminal.ts";
import type { Host } from "./types.ts";

interface Props {
  host: Host;
  active: boolean;
  onSession: (sessionId: string | null) => void;
  onClosed: () => void;
  onError: (msg: string) => void;
}

export function TerminalView({ host, active, onSession, onClosed, onError }: Props) {
  const containerRef = useSshTerminal({
    hostId: host.id,
    active,
    onSession,
    onClosed,
    onError,
  });

  return <div ref={containerRef} className="h-full w-full overflow-hidden" />;
}
