"use client";

import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  DollarSign,
  MessageSquare,
  Search,
  Sparkles,
  TrendingUp,
} from "lucide-react";
import { SiteHeader } from "@/components/site-header";
import { BrandLogo } from "@/components/brand-logo";
import { StatCard } from "@/components/stat-card";
import { ChainTabs } from "@/components/chain-tabs";
import { ChainIcon } from "@/components/chain-icon";
import { PagedFeed } from "@/components/paged-feed";
import { VolumeChart } from "@/components/volume-chart";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { CHAIN_META, formatUsd } from "@/lib/format";
import type {
  ChainStat,
  PricePoint,
  Transaction,
  VolumePoint,
} from "@/lib/types";
import { SiteFooter } from "@/components/site-footer";
import { InsightsCard } from "@/components/insights-card";
import { ChatPanel } from "@/components/chat-panel";
import { NarrativeCard } from "@/components/narrative-card";
import { AiSearch } from "@/components/ai-search";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:4000";

const MIN_OPTIONS = [
  { label: "Any size", value: 0 },
  { label: "$100K+", value: 100_000 },
  { label: "$1M+", value: 1_000_000 },
  { label: "$10M+", value: 10_000_000 },
];

const RANGES = [
  { label: "24H", hours: 24, bucket: "hour" },
  { label: "7D", hours: 168, bucket: "hour" },
  { label: "30D", hours: 720, bucket: "day" },
] as const;

const SYMBOLS = ["BTC", "ETH", "SOL"] as const;

function IconBadge({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex h-8 w-8 items-center justify-center rounded-full bg-white/95 ring-1 ring-white/10">
      {children}
    </span>
  );
}

export default function DashboardPage() {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [stats, setStats] = useState<ChainStat[]>([]);
  const [volume, setVolume] = useState<VolumePoint[]>([]);
  const [prices, setPrices] = useState<Record<string, PricePoint[]>>({});
  const [chain, setChain] = useState("all");
  const [minUsd, setMinUsd] = useState(0);
  const [query, setQuery] = useState("");
  const [rangeIdx, setRangeIdx] = useState(1);
  const [loading, setLoading] = useState(true);

  const handleChainChange = (value: string) => {
    setChain(value);
    setLoading(true);
  };

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const params = new URLSearchParams({ limit: "100" });
        if (chain !== "all") params.set("chain", chain);

        const [txResult, statsResult] = await Promise.allSettled([
          fetch(`${API_URL}/transactions?${params}`).then((r) => {
            if (!r.ok) throw new Error(`/transactions responded ${r.status}`);
            return r.json() as Promise<{ data: Transaction[] }>;
          }),
          fetch(`${API_URL}/stats`).then((r) => {
            if (!r.ok) throw new Error(`/stats responded ${r.status}`);
            return r.json() as Promise<{ data: ChainStat[] }>;
          }),
        ]);

        if (cancelled) return;

        if (txResult.status === "fulfilled")
          setTransactions(txResult.value.data);
        else console.error("Failed to fetch transactions", txResult.reason);

        if (statsResult.status === "fulfilled")
          setStats(statsResult.value.data);
        else console.error("Failed to fetch stats", statsResult.reason);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    void load();
    const interval = setInterval(() => {
      void load();
    }, 15_000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [chain]);

  useEffect(() => {
    const { hours, bucket } = RANGES[rangeIdx];
    let cancelled = false;

    const load = async () => {
      const volP = fetch(
        `${API_URL}/volume?${new URLSearchParams({ hours: String(hours), bucket })}`,
      ).then((r) => {
        if (!r.ok) throw new Error(`/volume responded ${r.status}`);
        return r.json() as Promise<{ data: VolumePoint[] }>;
      });

      const pricesP = Promise.allSettled(
        SYMBOLS.map((symbol) =>
          fetch(
            `${API_URL}/price-history?${new URLSearchParams({ symbol, hours: String(hours), bucket })}`,
          ).then((r) => {
            if (!r.ok)
              throw new Error(
                `/price-history (${symbol}) responded ${r.status}`,
              );
            return r.json() as Promise<{ data: PricePoint[] }>;
          }),
        ),
      );

      const [volResult] = await Promise.allSettled([volP]);
      const priceResults = await pricesP;

      if (cancelled) return;

      if (volResult.status === "fulfilled") setVolume(volResult.value.data);
      else console.error("Failed to fetch volume", volResult.reason);

      setPrices((prev) => {
        const next = { ...prev };
        priceResults.forEach((res, i) => {
          if (res.status === "fulfilled") next[SYMBOLS[i]] = res.value.data;
          else
            console.error(
              `Failed to fetch ${SYMBOLS[i]} price history`,
              res.reason,
            );
        });
        return next;
      });
    };

    void load();
    const id = setInterval(() => {
      void load();
    }, 60_000);

    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [rangeIdx]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return transactions.filter(
      (tx) =>
        tx.value_usd >= minUsd &&
        (!q ||
          tx.hash.toLowerCase().includes(q) ||
          tx.from_address.toLowerCase().includes(q) ||
          tx.to_address.toLowerCase().includes(q)),
    );
  }, [transactions, minUsd, query]);

  const countFor = (c: string) => stats.find((s) => s.chain === c)?.count ?? 0;
  const totalVolume = stats.reduce((sum, s) => sum + (s.volume_usd ?? 0), 0);
  const maxVolume = Math.max(1, ...stats.map((s) => s.volume_usd ?? 0));
  const largest = transactions.reduce(
    (max, tx) => (tx.value_usd > max ? tx.value_usd : max),
    0,
  );

  return (
    <div id="top" className="relative min-h-screen">
      <SiteHeader />

      <div className="lg:pointer-events-none lg:absolute lg:inset-x-0 lg:top-2.5">
        <div className="mx-auto max-w-7xl px-4 pt-4 sm:px-6 lg:pt-0">
          <BrandLogo />
        </div>
      </div>

      <main className="mx-auto max-w-7xl space-y-10 px-4 pb-8 pt-10 sm:px-6 lg:pt-16">
        <section className="space-y-2">
          <h1 className="text-3xl font-bold tracking-tight sm:text-4xl">
            Track the biggest moves on-chain
          </h1>
          <p className="max-w-2xl text-muted-foreground">
            Real-time large transactions across Ethereum, Solana and Bitcoin,
            straight from the blockchain.
          </p>
        </section>

        <section className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-5">
          <StatCard
            title="Total Volume"
            value={formatUsd(totalVolume)}
            subtitle="all tracked txs"
            icon={
              <IconBadge>
                <DollarSign className="h-4 w-4 text-slate-900" />
              </IconBadge>
            }
          />
          <StatCard
            title="Largest Tx"
            value={formatUsd(largest)}
            icon={
              <IconBadge>
                <TrendingUp className="h-4 w-4 text-slate-900" />
              </IconBadge>
            }
          />
          <StatCard
            title="Ethereum"
            value={String(countFor("ethereum"))}
            subtitle="whale txs"
            icon={<ChainIcon chain="ethereum" />}
          />
          <StatCard
            title="Solana"
            value={String(countFor("solana"))}
            subtitle="whale txs"
            icon={<ChainIcon chain="solana" />}
          />
          <StatCard
            title="Bitcoin"
            value={String(countFor("bitcoin"))}
            subtitle="whale txs"
            icon={<ChainIcon chain="bitcoin" />}
          />
        </section>

        <section id="alerts" className="scroll-mt-20 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="text-xl font-semibold">Live Alerts</h2>
            <ChainTabs value={chain} onChange={handleChainChange} />
          </div>
          <AiSearch />
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap gap-2">
              {MIN_OPTIONS.map((o) => (
                <Button
                  key={o.value}
                  size="xs"
                  variant="ghost"
                  onClick={() => setMinUsd(o.value)}
                  className={`border ${
                    minUsd === o.value
                      ? "border-slate-900 bg-slate-900 text-white hover:bg-slate-900 hover:text-white"
                      : "border-slate-500/60 bg-white/40 text-slate-900 hover:bg-white/70"
                  }`}
                >
                  {o.label}
                </Button>
              ))}
            </div>
            <div className="relative w-full sm:w-72">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search hash or address"
                className="border-slate-500/60 bg-white/60 pl-8 placeholder:text-slate-600"
              />
            </div>
          </div>

          <div className="overflow-hidden rounded-2xl border border-slate-500/50 bg-white/30 shadow-sm backdrop-blur-sm">
            <PagedFeed
              key={`${chain}-${minUsd}-${query}`}
              transactions={visible}
              loading={loading}
            />
          </div>
        </section>

        <section id="ask" className="scroll-mt-20 space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <MessageSquare className="h-4 w-4 text-primary" />
                Ask TidalIQ
              </CardTitle>
            </CardHeader>
            <CardContent>
              <ChatPanel />
            </CardContent>
          </Card>
        </section>

        <section id="analytics" className="scroll-mt-20 space-y-4">
          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base">
                Whale volume vs prices
              </CardTitle>
              <div className="flex rounded-lg border border-slate-500/50 bg-white/40 p-0.5">
                {RANGES.map((r, i) => (
                  <Button
                    key={r.label}
                    size="xs"
                    variant="ghost"
                    onClick={() => setRangeIdx(i)}
                    className={
                      i === rangeIdx
                        ? "bg-slate-900 text-white hover:bg-slate-900 hover:text-white"
                        : "text-slate-800 hover:bg-white/60"
                    }
                  >
                    {r.label}
                  </Button>
                ))}
              </div>
            </CardHeader>
            <CardContent>
              <VolumeChart
                points={volume}
                prices={prices}
                bucket={RANGES[rangeIdx].bucket}
                hours={RANGES[rangeIdx].hours}
              />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">Volume by chain</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {stats.length === 0 && (
                <p className="text-sm text-muted-foreground">No data yet.</p>
              )}
              {stats.map((s) => {
                const meta = CHAIN_META[s.chain];
                const vol = s.volume_usd ?? 0;
                return (
                  <div
                    key={s.chain}
                    className="group -mx-3 cursor-default space-y-2 rounded-xl px-3 py-2.5 transition-all duration-300 hover:bg-white/50 hover:shadow-sm hover:ring-1 hover:ring-slate-500/30"
                  >
                    <div className="flex items-center justify-between text-sm">
                      <span className="flex items-center gap-2.5">
                        <span className="transition-transform duration-300 group-hover:scale-110">
                          <ChainIcon chain={s.chain} size={24} />
                        </span>
                        <span className="font-medium transition-colors group-hover:text-slate-950">
                          {meta?.label ?? s.chain}
                        </span>
                        <span className="text-xs text-muted-foreground">
                          {s.count} txs
                        </span>
                      </span>
                      <span className="font-medium tabular-nums transition-all duration-300 group-hover:scale-105 group-hover:font-bold">
                        {formatUsd(vol)}
                      </span>
                    </div>
                    <div className="h-2 overflow-hidden rounded-full bg-muted transition-all duration-300 group-hover:h-3">
                      <div
                        className={`h-full rounded-full transition-all duration-300 group-hover:brightness-110 ${meta?.dot ?? "bg-primary"}`}
                        style={{ width: `${(vol / maxVolume) * 100}%` }}
                      />
                    </div>
                  </div>
                );
              })}
            </CardContent>
          </Card>
        </section>

        <section id="insights" className="scroll-mt-20 space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Sparkles className="h-4 w-4 text-primary" />
                Market narrative
              </CardTitle>
            </CardHeader>
            <CardContent>
              <NarrativeCard />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Sparkles className="h-4 w-4 text-primary" />
                AI Insights
              </CardTitle>
            </CardHeader>
            <CardContent>
              <InsightsCard />
            </CardContent>
          </Card>
        </section>
      </main>

      <SiteFooter />
    </div>
  );
}