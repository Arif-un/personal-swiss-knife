import { invoke } from "@tauri-apps/api/core";

/** Deploy mode -> workflow inputs (type + clean). */
export type DevkonMode = "full" | "backend" | "cleanFull" | "cleanBackend";

export const MODE_LABELS: Record<DevkonMode, string> = {
  full: "Full",
  backend: "Backend",
  cleanFull: "Clean redeploy (full)",
  cleanBackend: "Clean redeploy (backend)",
};

/** One managed name = one devkon namespace deployment. */
export interface DevkonEntry {
  id: string;
  name: string;
  branch: string;
  mode: DevkonMode;
  lastRunId: number | null;
  lastRunKind: "apply" | "destroy" | null;
  lastRunUrl: string | null;
  lastDeployedAt: string | null;
}

export interface DevkonStore {
  /** owner/repo hosting the deploy workflow (blank until configured). */
  repo: string;
  /** workflow_dispatch file name. */
  workflow: string;
  /** Namespace URL template; `{name}` is replaced with the entry name. */
  clusterDomain: string;
  entries: DevkonEntry[];
}

export interface RunStatus {
  runId: number | null;
  kind: "apply" | "destroy" | null;
  /** queued | in_progress | completed | none */
  state: string;
  /** success | failure | ... (only when completed) */
  conclusion: string | null;
  lastDeployedAt: string | null;
}

export const devkonApi = {
  list: () => invoke<DevkonStore>("devkon_list"),
  save: (entry: Partial<DevkonEntry> & { name: string }) =>
    invoke<DevkonEntry>("devkon_save", { entry }),
  remove: (id: string) => invoke<void>("devkon_delete", { id }),
  branches: () => invoke<string[]>("devkon_branches"),
  deploy: (id: string) => invoke<DevkonEntry>("devkon_deploy", { id }),
  destroy: (id: string) => invoke<DevkonEntry>("devkon_destroy", { id }),
  status: (id: string) => invoke<RunStatus>("devkon_status", { id }),
  setConfig: (repo: string, workflow: string, clusterDomain: string) =>
    invoke<DevkonStore>("devkon_set_config", { repo, workflow, clusterDomain }),
};

export const devkonKeys = {
  list: () => ["devkon", "list"] as const,
  branches: () => ["devkon", "branches"] as const,
  status: (id: string) => ["devkon", "status", id] as const,
};
