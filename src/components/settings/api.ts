import { invoke } from "@tauri-apps/api/core";
import type { Branding } from "#hooks/use-branding.tsx";

/** Standard-path knobs for the Cisco Umbrella toggle (Utils page). */
export interface CiscoConfig {
  orginfo: string;
  orginfoOff: string;
  daemonLabel: string;
  daemonPlist: string;
}

/** One submodule-repo -> product-group mapping for WP deploy. */
export interface RepoMapping {
  repo: string;
  group: string;
  /** lite | pro | theme */
  kind: string;
}

/** Full persisted WP deploy config (only the product-map part is edited here). */
export interface WpDeployConfig {
  targetHostId: string;
  zipBase: string;
  docroots: Record<string, string>;
  themeSlug: string;
  slugsRelPath: string;
  repoMap: RepoMapping[];
}

export const settingsApi = {
  getBranding: () => invoke<Branding>("branding_get"),
  setBranding: (branding: Branding) => invoke<void>("branding_set", { branding }),
  /** Export all settings + secrets to a chosen file; returns the path or null if cancelled. */
  exportAll: () => invoke<string | null>("settings_export"),
  /** Import a backup file; returns false if the pick was cancelled. */
  importAll: () => invoke<boolean>("settings_import"),

  getCisco: () => invoke<CiscoConfig>("cisco_get_config"),
  setCisco: (config: CiscoConfig) => invoke<void>("cisco_set_config", { config }),

  setDevkon: (repo: string, workflow: string, clusterDomain: string) =>
    invoke<unknown>("devkon_set_config", { repo, workflow, clusterDomain }),

  getWp: () => invoke<WpDeployConfig>("wpdeploy_config_get"),
  setWpProducts: (themeSlug: string, slugsRelPath: string, repoMap: RepoMapping[]) =>
    invoke<WpDeployConfig>("wpdeploy_set_products", { themeSlug, slugsRelPath, repoMap }),
};

export const settingsKeys = {
  branding: () => ["settings", "branding"] as const,
  cisco: () => ["settings", "cisco"] as const,
  wp: () => ["settings", "wp"] as const,
};
