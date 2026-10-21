"use client";

import { useEffect, useMemo, useState } from "react";
import {
  Bar,
  CartesianGrid,
  Cell,
  ComposedChart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { PricePoint, VolumePoint } from "@/lib/types";

const HOUR = 3_600_000;
const DAY = 86_400_000;
const UP = "#22a05a";
const DOWN = "#dc4a4a";
const INK = "#1e293b";

const SERIES = [
  { sym: "BTC", name: "Bitcoin", color: "#ea580c" },
  { sym: "ETH", name: "Ethereum", color: "#4f46e5" },
  { sym: "SOL", name: "Solana", color: "#c026d3" },
] as const;

interface Row {
  t: number;
  volume: number;
  change: number;
  price: Record<string, number | undefined>;
  pct: Record<string, number | undefined>;
}

const usdFull = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});

const usdPrice = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

function useIsMobile(breakpoint = 640) {
  const [mobile, setMobile] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${breakpoint - 1}px)`);
    const update = () => setMobile(mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, [breakpoint]);
  return mobile;
}

function toMs(ts: string) {
  const iso = /(Z|[+-]\d{2}:?\d{2})$/.test(ts) ? ts : `${ts}Z`;
  return new Date(iso).getTime();
}

function fmtAxis(v: number) {
  const a = Math.abs(v);
  const sign = v < 0 ? "-" : "";
  if (a >= 1e9) return `${sign}${(a / 1e9).toFixed(1)}B`;
  if (a >= 1e6) return `${sign}${Math.round(a / 1e6)}M`;
  if (a >= 1e3) return `${sign}${Math.round(a / 1e3)}K`;
  return String(Math.round(v));
}

const fmtPct = (v: number) => `${v > 0 ? "+" : ""}${v.toFixed(1)}%`;
const pad = (n: number) => String(n).padStart(2, "0");

function fmtFull(t: number) {
  const d = new Date(t);
  const mon = d.toLocaleString("en-US", { month: "short" });
  return `${d.getDate()} ${mon} '${String(d.getFullYear()).slice(2)} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function makeTickFormatter(bucket: "hour" | "day", hours: number) {
  return (t: number) => {
    const d = new Date(t);
    const mon = d.toLocaleString("en-US", { month: "short" });
    if (bucket === "hour" && hours <= 48) return `${pad(d.getHours())}:00`;
    if (bucket === "hour")
      return d.getHours() === 0
        ? `${d.getDate()} ${mon}`
        : `${pad(d.getHours())}:00`;
    if (d.getDate() === 1) return `${mon} '${String(d.getFullYear()).slice(2)}`;
    return `${d.getDate()} ${mon}`;
  };
}

function DashedCursor(props: {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  points?: { x: number; y: number }[];
}) {
  const { x = 0, y = 0, width = 0, height = 0, points } = props;
  const cx = points?.[0]?.x ?? x + width / 2;
  const top = points?.[0]?.y ?? y;
  const bottom = points?.[1]?.y ?? y + height;
  return (
    <line
      x1={cx}
      x2={cx}
      y1={top}
      y2={bottom}
      stroke="#334155"
      strokeWidth={1}
      strokeDasharray="4 4"
    />
  );
}

function ChartTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload?: Row }[];
}) {
  const row = payload?.[0]?.payload;
  if (!active || !row) return null;
  const up = row.change >= 0;

  return (
    <div className="w-60 overflow-hidden rounded-xl bg-white text-xs text-slate-900 shadow-2xl ring-1 ring-slate-900/10 sm:w-auto sm:min-w-72 sm:text-sm">
      <div className="bg-[#0f1f3d] px-3 py-2 font-medium text-white sm:px-4 sm:py-2.5">
        {fmtFull(row.t)}
      </div>
      <div className="space-y-2 px-3 py-3 sm:space-y-2.5 sm:px-4 sm:py-3.5">
        <div className="flex items-center gap-2">
          <span
            className="h-2.5 w-2.5 shrink-0 rounded-full sm:h-3 sm:w-3"
            style={{ background: up ? UP : DOWN }}
          />
          <span className="text-slate-600">Volume change</span>
          <span className="ml-auto font-semibold tabular-nums">
            {up ? "+" : "-"}
            {usdFull.format(Math.abs(row.change))}
          </span>
        </div>

        {SERIES.map((s) => {
          const price = row.price[s.sym];
          const pct = row.pct[s.sym];
          if (price == null) return null;
          return (
            <div key={s.sym} className="flex items-center gap-2">
              <span
                className="h-2.5 w-2.5 shrink-0 rounded-full sm:h-3 sm:w-3"
                style={{ background: s.color }}
              />
              <span className="text-slate-600">{s.sym}</span>
              <span className="ml-auto text-right font-semibold tabular-nums">
                {usdPrice.format(price)}
                {pct != null && (
                  <span
                    className="ml-1.5 text-[10px] font-medium sm:ml-2 sm:text-xs"
                    style={{ color: pct >= 0 ? "#15803d" : "#b91c1c" }}
                  >
                    {fmtPct(pct)}
                  </span>
                )}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Chip({ children }: { children: React.ReactNode }) {
  return (
    <span className="flex items-center gap-1.5 rounded-full border border-slate-500/30 bg-white/50 px-2 py-0.5 text-[11px] font-medium text-slate-800 sm:gap-2 sm:px-3 sm:py-1 sm:text-xs">
      {children}
    </span>
  );
}

interface VolumeChartProps {
  points: VolumePoint[];
  prices: Record<string, PricePoint[]>;
  bucket: "hour" | "day";
  hours: number;
}

export function VolumeChart({
  points,
  prices,
  bucket,
  hours,
}: VolumeChartProps) {
  const isMobile = useIsMobile();
  const step = bucket === "day" ? DAY : HOUR;

  const rows = useMemo<Row[]>(() => {
    const vol = new Map<number, number>();
    for (const p of points) vol.set(toMs(p.bucket), p.volume_usd);

    const px: Record<string, Map<number, number>> = {};
    for (const s of SERIES) {
      const m = new Map<number, number>();
      for (const p of prices[s.sym] ?? [])
        m.set(Math.floor(p.t / step) * step, p.price);
      px[s.sym] = m;
    }

    const all = [
      ...vol.keys(),
      ...SERIES.flatMap((s) => [...px[s.sym].keys()]),
    ];
    if (all.length === 0) return [];

    const end = Math.max(...all);
    const count = Math.max(1, Math.round((hours * HOUR) / step));
    const start = end - (count - 1) * step;

    const out: Row[] = [];
    const base: Record<string, number | undefined> = {};
    let prev = 0;

    for (let i = 0; i < count; i++) {
      const t = start + i * step;
      const volume = vol.get(t) ?? 0;
      const price: Record<string, number | undefined> = {};
      const pct: Record<string, number | undefined> = {};

      for (const s of SERIES) {
        const p = px[s.sym].get(t);
        price[s.sym] = p;
        if (p != null) {
          if (base[s.sym] == null) base[s.sym] = p;
          pct[s.sym] = (p / base[s.sym]! - 1) * 100;
        }
      }

      out.push({ t, volume, change: i === 0 ? 0 : volume - prev, price, pct });
      prev = volume;
    }
    return out;
  }, [points, prices, step, hours]);

  if (rows.length === 0) {
    return (
      <div className="flex h-80 items-center justify-center text-sm text-slate-600">
        No data in this range yet. The chart fills in as the trackers save
        transactions.
      </div>
    );
  }

  const tick = { fill: INK, fontSize: isMobile ? 10 : 12 };

  return (
    <div>
      <div className="mb-2 flex flex-wrap items-center gap-1.5 sm:mb-4 sm:gap-2">
        <Chip>
          <span className="h-2.5 w-2.5 rounded-sm" style={{ background: UP }} />{" "}
          Volume up
        </Chip>
        <Chip>
          <span
            className="h-2.5 w-2.5 rounded-sm"
            style={{ background: DOWN }}
          />{" "}
          Volume down
        </Chip>
        {SERIES.map((s) => (
          <Chip key={s.sym}>
            <span
              className="h-1 w-4 rounded-full"
              style={{ background: s.color }}
            />{" "}
            {s.sym}
          </Chip>
        ))}
        <span className="ml-auto hidden text-xs text-slate-600 sm:block">
          Left: whale volume change · Right: price change %
        </span>
      </div>

      <p className="mb-2 text-[11px] text-slate-600 sm:hidden">
        Bars: whale volume change · Lines: price change % (tap for details)
      </p>

      <div className="h-72 w-full sm:h-80">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={rows}
            margin={{ top: 8, right: isMobile ? 4 : 8, bottom: 0, left: 0 }}
          >
            <CartesianGrid stroke="rgba(15,23,42,0.28)" strokeDasharray="3 4" />
            <XAxis
              dataKey="t"
              tickFormatter={makeTickFormatter(bucket, hours)}
              minTickGap={isMobile ? 36 : 48}
              tickMargin={6}
              tickLine={false}
              axisLine={false}
              tick={tick}
            />
            <YAxis
              yAxisId="vol"
              tickFormatter={fmtAxis}
              width={isMobile ? 40 : 60}
              tickCount={isMobile ? 5 : 6}
              tickLine={false}
              axisLine={false}
              tick={tick}
            />

            <YAxis
              yAxisId="pct"
              orientation="right"
              hide={isMobile}
              tickFormatter={fmtPct}
              width={56}
              domain={["auto", "auto"]}
              tickLine={false}
              axisLine={false}
              tick={tick}
            />
            <ReferenceLine yAxisId="vol" y={0} stroke="rgba(15,23,42,0.45)" />
            <Tooltip content={<ChartTooltip />} cursor={<DashedCursor />} />

            <Bar
              yAxisId="vol"
              dataKey="change"
              name="Whale volume change"
              maxBarSize={isMobile ? 6 : 12}
              radius={[3, 3, 3, 3]}
            >
              {rows.map((r) => (
                <Cell
                  key={r.t}
                  fill={r.change >= 0 ? UP : DOWN}
                  fillOpacity={0.9}
                />
              ))}
            </Bar>

            {SERIES.map((s) => (
              <Line
                key={s.sym}
                yAxisId="pct"
                type="monotone"
                dataKey={`pct.${s.sym}`}
                name={`${s.sym} price`}
                stroke={s.color}
                strokeWidth={isMobile ? 1.75 : 2.5}
                dot={false}
                connectNulls
                activeDot={{
                  r: isMobile ? 4 : 6,
                  fill: s.color,
                  stroke: "#ffffff",
                  strokeWidth: 2,
                }}
              />
            ))}
          </ComposedChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}