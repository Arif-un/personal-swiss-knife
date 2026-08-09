import { invoke } from "@tauri-apps/api/core";

export interface CiscoStatus {
  /** Cisco Secure Client is installed on this machine. */
  installed: boolean;
  /** The Umbrella agent (acumbrellaagent) is currently running. */
  running: boolean;
  /** The Umbrella profile is in place, i.e. the module is enabled. */
  profilePresent: boolean;
}

export const utilsApi = {
  /** Read the current Cisco Umbrella state (no admin prompt). */
  ciscoStatus: () => invoke<CiscoStatus>("cisco_status"),
  /** Enable/disable Cisco Umbrella (triggers a macOS admin prompt). */
  ciscoSetEnabled: (enabled: boolean) => invoke<CiscoStatus>("cisco_set_enabled", { enabled }),
};

/** react-query keys for the utils feature. */
export const utilsKeys = {
  ciscoStatus: () => ["cisco-status"] as const,
};
