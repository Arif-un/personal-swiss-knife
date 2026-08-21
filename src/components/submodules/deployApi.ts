import { invoke } from "@tauri-apps/api/core";
import { listen, type Event } from "@tauri-apps/api/event";
import type { Host } from "#components/ssh/types.ts";

/** Persisted deploy settings (mirrors backend `WpDeployConfig`). */
export interface WpDeployConfig {
  targetHostId: string;
  zipBase: string;
  docroots: Record<string, string>;
}

/** One deployable product inside a repo. */
export interface Product {
  group: string;
  slug: string;
  isLite: boolean;
}

/** One streamed output line for a running deploy. */
export interface LogLine {
  deployId: string;
  /** `"step"` | `"out"` | `"err"`. */
  stream: string;
  line: string;
}

/** Terminal event for a deploy/rollback. */
export interface DoneEvent {
  deployId: string;
  ok: boolean;
  message: string;
  version: string | null;
}

export const wpDeployApi = {
  configGet: () => invoke<WpDeployConfig>("wpdeploy_config_get"),
  configSave: (targetHostId: string, zipBase: string) =>
    invoke<WpDeployConfig>("wpdeploy_config_save", { targetHostId, zipBase }),
  configReset: () => invoke<WpDeployConfig>("wpdeploy_config_reset"),
  setDocroot: (hostId: string, docroot: string) =>
    invoke<WpDeployConfig>("wpdeploy_set_docroot", { hostId, docroot }),
  products: (enviraDev: string, repo: string) =>
    invoke<Product[]>("wpdeploy_products", { enviraDev, repo }),
  detectDocroot: (hostId: string) => invoke<string[]>("wpdeploy_detect_docroot", { hostId }),
  deploy: (enviraDev: string, slug: string, build: boolean, deployId: string) =>
    invoke<void>("wpdeploy_deploy", { enviraDev, slug, build, deployId }),
  rollback: (slug: string, deployId: string) =>
    invoke<void>("wpdeploy_rollback", { slug, deployId }),
  hostsList: () => invoke<Host[]>("hosts_list"),
};

/** Tauri event channels emitted by the deploy backend. */
export const WPDEPLOY_EVENTS = {
  log: "wpdeploy://log",
  done: "wpdeploy://done",
} as const;

export const wpDeployEvents = {
  onLog: (cb: (e: Event<LogLine>) => void) => listen<LogLine>(WPDEPLOY_EVENTS.log, cb),
  onDone: (cb: (e: Event<DoneEvent>) => void) => listen<DoneEvent>(WPDEPLOY_EVENTS.done, cb),
};

export const wpDeployKeys = {
  config: () => ["wpdeploy", "config"] as const,
  hosts: () => ["wpdeploy", "hosts"] as const,
  products: (enviraDev: string, repo: string) =>
    ["wpdeploy", "products", enviraDev, repo] as const,
};
