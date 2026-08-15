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
};

export const devkonKeys = {
  list: () => ["devkon", "list"] as const,
  branches: () => ["devkon", "branches"] as const,
  status: (id: string) => ["devkon", "status", id] as const,
};
