import { invoke } from "@tauri-apps/api/core";

/** User-editable settings for the AWS login button. */
export interface AwsAuthConfig {
  /** Brave profile display name (e.g. "OP"). */
  braveProfile: string;
  /** Repo root to run tools/awsauth from. */
  repoDir: string;
}

export const awsauthApi = {
  getConfig: () => invoke<AwsAuthConfig>("awsauth_get_config"),
  setConfig: (config: AwsAuthConfig) => invoke<void>("awsauth_set_config", { config }),
  /** Opens Brave and returns the credentials file's baseline mtime (epoch millis,
   * or null if absent) to poll against. */
  openBrave: () => invoke<number | null>("awsauth_open_brave"),
  /** True once the credentials file is newer than `baseline` (the manual download landed). */
  checkFresh: (baseline: number | null) => invoke<boolean>("awsauth_check_fresh", { baseline }),
  /** Ensures Docker is up and runs `tools/awsauth`; resolves with its output, rejects on failure. */
  finish: () => invoke<string>("awsauth_finish"),
};

export const awsauthKeys = {
  config: () => ["awsauth", "config"] as const,
};
