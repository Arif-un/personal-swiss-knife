import { ShieldAlertIcon } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Modal } from "#components/Modal.tsx";
import type { HostKeyPrompt } from "./types.ts";

interface Props {
  prompt: HostKeyPrompt;
  onDecide: (trust: boolean) => void;
}

export function HostKeyDialog({ prompt, onDecide }: Props) {
  return (
    <Modal>
      <div className="w-full max-w-md rounded-xl border bg-background p-5 shadow-lg">
        <div className="mb-3 flex items-center gap-2">
          <ShieldAlertIcon className="size-5 text-amber-500" />
          <h2 className="text-sm font-semibold">Unknown host key</h2>
        </div>
        <p className="mb-3 text-sm text-muted-foreground">
          The server <code className="text-foreground">{prompt.host}</code> is not in{" "}
          <code>~/.ssh/known_hosts</code>. Verify the fingerprint before trusting it.
        </p>
        <div className="mb-4 rounded-md border bg-muted px-3 py-2 font-mono text-xs break-all">
          <div className="text-muted-foreground">{prompt.algorithm}</div>
          {prompt.fingerprint}
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={() => onDecide(false)}>
            Reject
          </Button>
          <Button onClick={() => onDecide(true)}>Trust &amp; connect</Button>
        </div>
      </div>
    </Modal>
  );
}
