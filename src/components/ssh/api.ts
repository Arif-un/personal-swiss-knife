import { invoke } from "@tauri-apps/api/core";
import { listen, type Event } from "@tauri-apps/api/event";
import type {
  ForwardInfo,
  ForwardSpec,
  Host,
  HostKeyPrompt,
  SshClosedEvent,
  SshDataEvent,
} from "./types.ts";

export const sshApi = {
  hostsList: () => invoke<Host[]>("hosts_list"),
  hostSave: (host: Host) => invoke<string>("host_save", { host }),
  hostDelete: (host: Host) => invoke<void>("host_delete", { host }),
  discoverHistory: () => invoke<Host[]>("discover_history_hosts"),
  buildCommand: (hostId: string) => invoke<string>("ssh_build_command", { hostId }),
  setPassphrase: (keyPath: string, secret: string) =>
    invoke<void>("ssh_set_passphrase", { keyPath, secret }),

  connect: (hostId: string, cols: number, rows: number) =>
    invoke<string>("ssh_connect", { hostId, cols, rows }),
  trustHostkey: (promptId: string, trust: boolean) =>
    invoke<void>("ssh_trust_hostkey", { promptId, trust }),
  write: (sessionId: string, data: string) => invoke<void>("ssh_write", { sessionId, data }),
  resize: (sessionId: string, cols: number, rows: number) =>
    invoke<void>("ssh_resize", { sessionId, cols, rows }),
  disconnect: (sessionId: string) => invoke<void>("ssh_disconnect", { sessionId }),

  forwardStart: (sessionId: string, spec: ForwardSpec) =>
    invoke<string>("forward_start", { sessionId, spec }),
  forwardStop: (sessionId: string, forwardId: string) =>
    invoke<void>("forward_stop", { sessionId, forwardId }),
  forwardsList: (sessionId: string) => invoke<ForwardInfo[]>("forwards_list", { sessionId }),
};

/** Tauri event channel names emitted by the SSH backend. */
export const SSH_EVENTS = {
  data: "ssh://data",
  closed: "ssh://closed",
  hostkey: "ssh://hostkey",
} as const;

/** Typed `listen` wrappers so channel names live in one place (parity with the
 *  invoke wrappers above). Each returns the unlisten promise. */
export const sshEvents = {
  onData: (cb: (e: Event<SshDataEvent>) => void) => listen<SshDataEvent>(SSH_EVENTS.data, cb),
  onClosed: (cb: (e: Event<SshClosedEvent>) => void) =>
    listen<SshClosedEvent>(SSH_EVENTS.closed, cb),
  onHostkey: (cb: (e: Event<HostKeyPrompt>) => void) =>
    listen<HostKeyPrompt>(SSH_EVENTS.hostkey, cb),
};

/** react-query keys for the SSH feature. */
export const sshKeys = {
  hosts: () => ["ssh-hosts"] as const,
};
