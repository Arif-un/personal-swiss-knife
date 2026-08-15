import { invoke } from "@tauri-apps/api/core";
import type { Snapshot, SnapshotSummary } from "./types.ts";

export const memoryApi = {
  /** All retained snapshot summaries (oldest first); UI slices by range. */
  history: () => invoke<SnapshotSummary[]>("memory_history"),
  /** Latest snapshot with per-process breakdown, or null if none yet. */
  latest: () => invoke<Snapshot | null>("memory_latest"),
  /** Full snapshot (per-process breakdown) recorded at `ts`, or null if none. */
  snapshotAt: (ts: number) => invoke<Snapshot | null>("memory_snapshot_at", { ts }),
  /** Take, persist, and return a snapshot right now. */
  snapshotNow: () => invoke<Snapshot>("memory_snapshot_now"),
};

/** react-query keys for the memory feature. */
export const memoryKeys = {
  history: () => ["memory-history"] as const,
  latest: () => ["memory-latest"] as const,
  snapshot: (ts: number) => ["memory-snapshot", ts] as const,
};
