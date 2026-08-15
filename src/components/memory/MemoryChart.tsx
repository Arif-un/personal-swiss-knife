import {
  Area,
  AreaChart,
  CartesianGrid,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { formatBytes, formatStamp, formatTick } from "./format.ts";
import type { SnapshotSummary } from "./types.ts";

/** Background sampler cadence (mirrors SAMPLE_INTERVAL_SECS in the Rust
 *  memtrack module). Two samples farther apart than this * GAP_BREAK_FACTOR are
 *  treated as a gap the app was closed across. */
const SAMPLE_INTERVAL_SECONDS = 15 * 60;
const GAP_BREAK_FACTOR = 2;

interface Props {
  data: SnapshotSummary[];
  /** Selected window in seconds, used to pick tick granularity. */
  rangeSeconds: number;
  /** ts (unix seconds) of the point whose breakdown the table is showing. */
  selectedTs: number | null;
  /** Fires with a point's ts when the user clicks it, to drive the table. */
  onSelect: (ts: number) => void;
}

/** recharts 3's chart onClick param exposes the hovered x value as `activeLabel`
 *  (no `activePayload` like v2); our XAxis dataKey is `tsMs`, so this is epoch ms. */
interface ChartClick {
  activeLabel?: number;
}

/** A chart point; `totalRss` is null at an inserted break so recharts leaves a
 *  gap instead of drawing a line across app-closed downtime. */
interface ChartPoint {
  ts: number;
  totalRss: number | null;
  tsMs: number;
}

interface TooltipPayload {
  active?: boolean;
  payload?: { payload: ChartPoint }[];
}

function ChartTooltip({ active, payload }: TooltipPayload) {
  if (!active || !payload?.length) return null;
  const point = payload[0].payload;
  if (point.totalRss == null) return null;
  return (
    <div className="rounded-md border bg-popover px-3 py-2 text-xs shadow-md">
      <div className="text-muted-foreground">{formatStamp(point.ts)}</div>
      <div className="font-medium tabular-nums text-popover-foreground">
        {formatBytes(point.totalRss)}
      </div>
    </div>
  );
}

/** Single-series area chart of total RAM over time. The page title names the
 *  series, so no legend is needed (dataviz single-series rule). */
export function MemoryChart({ data, rangeSeconds, selectedTs, onSelect }: Props) {
  if (data.length === 0) {
    return (
      <div className="flex h-72 items-center justify-center rounded-lg border text-sm text-muted-foreground">
        No snapshots in this range yet.
      </div>
    );
  }

  // recharts `scale="time"` builds a d3 time scale that reads the domain as
  // epoch-milliseconds, but our `ts` is unix seconds; feed a ms field for the axis
  // so ticks land on real time boundaries. `ts` stays seconds for the tooltip.
  // A null-valued point is inserted across any gap wider than
  // SAMPLE_INTERVAL_SECONDS * GAP_BREAK_FACTOR so the line breaks over times the
  // app was closed instead of interpolating a straight line across them.
  const chartData: ChartPoint[] = [];
  for (let i = 0; i < data.length; i += 1) {
    const p = data[i];
    if (i > 0) {
      const prev = data[i - 1];
      if (p.ts - prev.ts > SAMPLE_INTERVAL_SECONDS * GAP_BREAK_FACTOR) {
        const midTs = prev.ts + (p.ts - prev.ts) / 2;
        chartData.push({ ts: midTs, totalRss: null, tsMs: midTs * 1000 });
      }
    }
    chartData.push({ ts: p.ts, totalRss: p.totalRss, tsMs: p.ts * 1000 });
  }

  return (
    <div className="h-72 w-full rounded-lg border p-2">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart
          data={chartData}
          margin={{ top: 8, right: 12, bottom: 4, left: 4 }}
          onClick={(state) => {
            const label = (state as ChartClick).activeLabel;
            if (typeof label !== "number") return;
            const ts = Math.round(label / 1000);
            // Only select a real snapshot, never an inserted gap midpoint.
            if (data.some((d) => d.ts === ts)) onSelect(ts);
          }}
          className="cursor-pointer"
        >
          <defs>
            <linearGradient id="memFill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--mem-series)" stopOpacity={0.25} />
              <stop offset="100%" stopColor="var(--mem-series)" stopOpacity={0.02} />
            </linearGradient>
          </defs>
          <CartesianGrid stroke="var(--mem-grid)" strokeDasharray="3 3" vertical={false} />
          <XAxis
            dataKey="tsMs"
            type="number"
            scale="time"
            domain={["dataMin", "dataMax"]}
            tickFormatter={(v: number) => formatTick(v / 1000, rangeSeconds)}
            tick={{ fill: "var(--mem-axis)", fontSize: 11 }}
            stroke="var(--mem-grid)"
            minTickGap={40}
          />
          <YAxis
            width={64}
            tickFormatter={(v: number) => formatBytes(v)}
            tick={{ fill: "var(--mem-axis)", fontSize: 11 }}
            stroke="var(--mem-grid)"
          />
          <Tooltip content={<ChartTooltip />} />
          {selectedTs != null && (
            <ReferenceLine
              x={selectedTs * 1000}
              stroke="var(--mem-series)"
              strokeWidth={1.5}
              strokeDasharray="4 3"
            />
          )}
          <Area
            type="monotone"
            dataKey="totalRss"
            stroke="var(--mem-series)"
            strokeWidth={2}
            fill="url(#memFill)"
            // A lone point draws no line segment; show its dot so a single
            // snapshot (e.g. right after first launch) isn't an empty chart.
            dot={data.length === 1}
            activeDot={{ r: 4, strokeWidth: 0 }}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
