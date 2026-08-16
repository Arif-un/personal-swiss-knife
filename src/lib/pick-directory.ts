import { invoke } from "@tauri-apps/api/core";

/** Open a native folder picker. Returns the chosen path, or null if cancelled. */
export function pickDirectory() {
  return invoke<string | null>("pick_directory");
}
