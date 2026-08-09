/** Human-readable byte size, e.g. `1.4 GB`. Binary (1024) units. */
export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

/** Axis tick label: `HH:mm` for intraday ranges, `MMM d` for multi-day. */
export function formatTick(tsSeconds: number, rangeSeconds: number): string {
  const d = new Date(tsSeconds * 1000);
  if (rangeSeconds <= 24 * 60 * 60) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

/** Full timestamp for tooltips, e.g. `Aug 9, 14:30`. */
export function formatStamp(tsSeconds: number): string {
  const d = new Date(tsSeconds * 1000);
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })}, ${d.toLocaleTimeString(
    [],
    { hour: "2-digit", minute: "2-digit" },
  )}`;
}
