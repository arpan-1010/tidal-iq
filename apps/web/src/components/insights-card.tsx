"use client";

import { useEffect, useState } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { CHAIN_META, formatUsd, timeAgo } from "@/lib/format";
import type { InsightRow, Insights } from "@/lib/types";

const API = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:4000";

function Row({ row }: { row: InsightRow }) {
  const meta = CHAIN_META[row.chain];
  return (
    <li className="rounded-lg border border-slate-500/30 bg-white/40 p-3 transition-all duration-300 hover:bg-white/60 hover:shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-semibold text-slate-900">
          {meta?.label ?? row.chain} · {row.token_symbol}
        </span>
        <span className="font-semibold tabular-nums text-emerald-700">
          {formatUsd(row.value_usd)}
        </span>
      </div>
      <p className="mt-1 text-xs text-slate-700">
        {row.ai_summary ?? row.anomaly_reason ?? "Summary pending…"}
      </p>
      <p className="mt-1 text-[11px] text-slate-600">
        Score {row.impact_score != null ? Math.round(row.impact_score) : "—"} ·{" "}
        {timeAgo(row.tx_timestamp)}
      </p>
    </li>
  );
}

function Column({
  title,
  rows,
  empty,
}: {
  title: string;
  rows: InsightRow[];
  empty: string;
}) {
  return (
    <div>
      <h3 className="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-800">
        {title}
      </h3>
      {rows.length === 0 ? (
        <p className="text-sm text-slate-600">{empty}</p>
      ) : (
        <ul className="space-y-2">
          {rows.map((r) => (
            <Row key={r.hash} row={r} />
          ))}
        </ul>
      )}
    </div>
  );
}

export function InsightsCard() {
  const [data, setData] = useState<Insights | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch(`${API}/insights`);
        if (!res.ok) throw new Error(String(res.status));
        const json: Insights = await res.json();
        if (!cancelled) {
          setData(json);
          setError(false);
        }
      } catch {
        if (!cancelled) setError(true);
      }
    }
    load();
    const id = setInterval(load, 30_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  if (error && !data) {
    return (
      <p className="text-sm text-slate-700">
        Couldn&apos;t load insights right now.
      </p>
    );
  }
  if (!data) {
    return (
      <div className="space-y-3">
        {Array.from({ length: 3 }).map((_, i) => (
          <Skeleton key={i} className="h-16 w-full" />
        ))}
      </div>
    );
  }

  return (
    <div className="grid gap-6 md:grid-cols-2">
      <Column
        title="Top moves · 24h"
        rows={data.top}
        empty="No scored transactions yet."
      />
      <Column
        title="Anomalies · 24h"
        rows={data.anomalies}
        empty="Nothing unusual flagged."
      />
    </div>
  );
}