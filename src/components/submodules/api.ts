import { invoke } from "@tauri-apps/api/core";

export interface GitmodConfig {
  path: string;
}

/** One repo row: the parent superproject or a submodule. */
export interface RepoRow {
  /** `.` for the parent, else the submodule path. */
  name: string;
  isParent: boolean;
  /** Current branch, or empty when detached. */
  branch: string;
  detached: boolean;
  /** Tag or short sha of HEAD (`git describe --tags --always`). */
  headDesc: string;
  dirty: boolean;
  ahead: number | null;
  behind: number | null;
  branches: string[];
  error: string | null;
}

/** Dirty-tree strategy when switching branches. */
export type DirtyAction = "none" | "stash" | "carry";

/** Result of bulk "switch all": refreshed rows + notes for skipped/failed repos. */
export interface SwitchAllResult {
  rows: RepoRow[];
  notes: string[];
}

export const gitmodApi = {
  getConfig: () => invoke<GitmodConfig>("gitmod_get_config"),
  setConfig: (path: string) => invoke<GitmodConfig>("gitmod_set_config", { path }),
  status: (path: string) => invoke<RepoRow[]>("gitmod_status", { path }),
  switch: (path: string, sub: string, branch: string, action: DirtyAction) =>
    invoke<RepoRow>("gitmod_switch", { path, sub, branch, action }),
  refreshPull: (path: string) => invoke<RepoRow[]>("gitmod_refresh_pull", { path }),
  switchAll: (path: string, action: DirtyAction) =>
    invoke<SwitchAllResult>("gitmod_switch_all", { path, action }),
  openApp: (path: string, sub: string, app: "github" | "vscode" | "terminal") =>
    invoke<void>("gitmod_open_app", { path, sub, app }),
};

export const gitmodKeys = {
  config: () => ["gitmod", "config"] as const,
  status: (path: string) => ["gitmod", "status", path] as const,
};
