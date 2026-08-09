/** One process's resident memory at snapshot time. */
export interface ProcSample {
  pid: number;
  name: string;
  rssBytes: number;
}

/** A full snapshot: summed total plus the per-process breakdown. */
export interface Snapshot {
  /** Unix seconds. */
  ts: number;
  totalRss: number;
  processes: ProcSample[];
}

/** Lightweight point for the time-series chart. */
export interface SnapshotSummary {
  /** Unix seconds. */
  ts: number;
  totalRss: number;
}

/** Selectable history windows on the page. */
export type RangeKey = "1h" | "24h" | "7d" | "30d";

export const RANGES: { key: RangeKey; label: string; seconds: number }[] = [
  { key: "1h", label: "1H", seconds: 60 * 60 },
  { key: "24h", label: "24H", seconds: 24 * 60 * 60 },
  { key: "7d", label: "7D", seconds: 7 * 24 * 60 * 60 },
  { key: "30d", label: "30D", seconds: 30 * 24 * 60 * 60 },
];
