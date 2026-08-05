import { DEFAULT_SSH_PORT } from "./constants.ts";

export interface ForwardSpec {
  type: "L";
  bindAddr: string;
  bindPort: number;
  destHost: string;
  destPort: number;
}

export interface Host {
  id: string;
  source: "ssh-config" | "app";
  alias: string;
  hostname: string;
  user: string;
  port: number;
  identityFile?: string | null;
  useAgent: boolean;
  proxyJump?: string | null;
  forwards: ForwardSpec[];
  extraOptions?: string | null;
}

export interface ForwardInfo {
  id: string;
  spec: ForwardSpec;
}

export interface HostKeyPrompt {
  promptId: string;
  host: string;
  fingerprint: string;
  algorithm: string;
}

export interface SshDataEvent {
  sessionId: string;
  bytes: number[];
}

export interface SshClosedEvent {
  sessionId: string;
  reason: string;
}

export function emptyHost(): Host {
  return {
    id: "",
    source: "app",
    alias: "",
    hostname: "",
    user: "",
    port: DEFAULT_SSH_PORT,
    identityFile: null,
    useAgent: true,
    proxyJump: null,
    forwards: [],
    extraOptions: null,
  };
}
