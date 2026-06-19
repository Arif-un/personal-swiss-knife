import { invoke } from "@tauri-apps/api/core";
import type { ForwardInfo, ForwardSpec, Host } from "./types.ts";

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
  write: (sessionId: string, data: string) =>
    invoke<void>("ssh_write", { sessionId, data }),
  resize: (sessionId: string, cols: number, rows: number) =>
    invoke<void>("ssh_resize", { sessionId, cols, rows }),
  disconnect: (sessionId: string) => invoke<void>("ssh_disconnect", { sessionId }),

  forwardStart: (sessionId: string, spec: ForwardSpec) =>
    invoke<string>("forward_start", { sessionId, spec }),
  forwardStop: (sessionId: string, forwardId: string) =>
    invoke<void>("forward_stop", { sessionId, forwardId }),
  forwardsList: (sessionId: string) =>
    invoke<ForwardInfo[]>("forwards_list", { sessionId }),
};
