import { useState } from "react";
import { PlusIcon, Trash2Icon, XIcon } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { Modal } from "#components/Modal.tsx";
import { DEFAULT_BIND_ADDR, DEFAULT_SSH_PORT } from "./constants.ts";
import type { ForwardSpec, Host } from "./types.ts";

interface Props {
  initial: Host;
  onSave: (host: Host) => void;
  onClose: () => void;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

export function HostForm({ initial, onSave, onClose }: Props) {
  const [h, setH] = useState<Host>({ ...initial, forwards: [...initial.forwards] });
  // Stable per-row keys so editing/removing a forward can't rebind inputs to
  // the wrong row (an index key would).
  const [forwardKeys, setForwardKeys] = useState<string[]>(() =>
    initial.forwards.map(() => crypto.randomUUID()),
  );

  function set<K extends keyof Host>(key: K, value: Host[K]) {
    setH((prev) => ({ ...prev, [key]: value }));
  }

  function setForward(i: number, patch: Partial<ForwardSpec>) {
    setH((prev) => {
      const forwards = prev.forwards.map((f, idx) => (idx === i ? { ...f, ...patch } : f));
      return { ...prev, forwards };
    });
  }

  function addForward() {
    setH((prev) => ({
      ...prev,
      forwards: [
        ...prev.forwards,
        { type: "L", bindAddr: DEFAULT_BIND_ADDR, bindPort: 0, destHost: "", destPort: 0 },
      ],
    }));
    setForwardKeys((prev) => [...prev, crypto.randomUUID()]);
  }

  function removeForward(i: number) {
    setH((prev) => ({ ...prev, forwards: prev.forwards.filter((_, idx) => idx !== i) }));
    setForwardKeys((prev) => prev.filter((_, idx) => idx !== i));
  }

  const isNew = !initial.id;

  return (
    <Modal>
      <div className="flex max-h-[90vh] w-full max-w-lg flex-col overflow-hidden rounded-xl border bg-background shadow-lg">
        <div className="flex items-center justify-between border-b px-4 py-3">
          <h2 className="text-sm font-semibold">{isNew ? "New host" : `Edit ${initial.alias}`}</h2>
          <Button variant="ghost" size="icon-sm" onClick={onClose} aria-label="Close">
            <XIcon />
          </Button>
        </div>

        <div className="flex flex-col gap-3 overflow-y-auto px-4 py-4">
          {initial.source === "ssh-config" && (
            <p className="rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground">
              Editing a host from <code>~/.ssh/config</code> — saving rewrites its block in place.
            </p>
          )}

          <div className="grid grid-cols-2 gap-3">
            <Field label="Alias">
              <Input
                value={h.alias}
                onChange={(e) => set("alias", e.target.value)}
                placeholder="prod-web"
              />
            </Field>
            <Field label="Hostname / IP">
              <Input
                value={h.hostname}
                onChange={(e) => set("hostname", e.target.value)}
                placeholder="10.0.4.12"
              />
            </Field>
            <Field label="User">
              <Input
                value={h.user}
                onChange={(e) => set("user", e.target.value)}
                placeholder="root"
              />
            </Field>
            <Field label="Port">
              <Input
                type="number"
                value={h.port}
                onChange={(e) => set("port", Number(e.target.value) || DEFAULT_SSH_PORT)}
              />
            </Field>
          </div>

          <Field label="Identity file (optional)">
            <Input
              value={h.identityFile ?? ""}
              onChange={(e) => set("identityFile", e.target.value)}
              placeholder="~/.ssh/id_ed25519"
            />
          </Field>

          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={h.useAgent}
              onChange={(e) => set("useAgent", e.target.checked)}
            />
            Use ssh-agent
          </label>

          <Field label="ProxyJump (optional)">
            <Input
              value={h.proxyJump ?? ""}
              onChange={(e) => set("proxyJump", e.target.value)}
              placeholder="bastion  or  user@bastion:22"
            />
          </Field>

          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-muted-foreground">Local forwards (-L)</span>
              <Button variant="outline" size="xs" onClick={addForward}>
                <PlusIcon /> Add
              </Button>
            </div>
            {h.forwards.map((f, i) => (
              <div key={forwardKeys[i]} className="flex items-center gap-1.5">
                <Input
                  className="h-7 w-28"
                  value={f.bindAddr}
                  onChange={(e) => setForward(i, { bindAddr: e.target.value })}
                  placeholder="127.0.0.1"
                />
                <Input
                  className="h-7 w-16"
                  type="number"
                  value={f.bindPort || ""}
                  onChange={(e) => setForward(i, { bindPort: Number(e.target.value) || 0 })}
                  placeholder="5432"
                />
                <span className="text-muted-foreground">→</span>
                <Input
                  className="h-7 flex-1"
                  value={f.destHost}
                  onChange={(e) => setForward(i, { destHost: e.target.value })}
                  placeholder="db.internal"
                />
                <Input
                  className="h-7 w-16"
                  type="number"
                  value={f.destPort || ""}
                  onChange={(e) => setForward(i, { destPort: Number(e.target.value) || 0 })}
                  placeholder="5432"
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => removeForward(i)}
                  aria-label="Remove"
                >
                  <Trash2Icon />
                </Button>
              </div>
            ))}
          </div>

          <Field label="Extra ssh options (optional, one per line)">
            <textarea
              className="min-h-16 rounded-md border bg-transparent px-3 py-2 text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
              value={h.extraOptions ?? ""}
              onChange={(e) => set("extraOptions", e.target.value)}
              placeholder="ServerAliveInterval 30"
            />
          </Field>
        </div>

        <div className="flex justify-end gap-2 border-t px-4 py-3">
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => onSave(h)} disabled={!h.alias.trim() || !h.hostname.trim()}>
            Save
          </Button>
        </div>
      </div>
    </Modal>
  );
}
