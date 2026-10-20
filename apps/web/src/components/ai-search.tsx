"use client";

import { useRef, useState, type FormEvent } from "react";
import { ExternalLink, Sparkles, X } from "lucide-react";
import {
  AI_DISCLAIMER,
  type SearchFilters,
  type SearchResponse,
} from "@tidaliq/shared";
import { ai } from "@/lib/ai";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { CHAIN_META, formatUsd } from "@/lib/format";

const FLAG_LABEL: Record<string, string> = {
  UNUSUAL_SIZE: "Unusual size",
  FIRST_LARGE_MOVE: "First large move",
  RAPID_SUCCESSION: "Rapid succession",
  TOKEN_OUTLIER: "Token outlier",
};

const TIER_TONE: Record<string, string> = {
  Critical: "bg-red-600/15 text-red-700",
  High: "bg-amber-500/20 text-amber-800",
  Medium: "bg-sky-500/15 text-sky-800",
  Low: "bg-slate-500/15 text-slate-700",
};

const EXAMPLES = [
  "big bitcoin transfers today",
  "unusual transfers this week",
  "ETH moves over $2M yesterday",
];

function describeFilters(f: SearchFilters): string[] {
  const out: string[] = [];
  f.chains?.forEach((c) => out.push(CHAIN_META[c]?.label ?? c));
  f.symbols?.forEach((s) => out.push(s));
  if (f.period) out.push(f.period);
  if (f.min_usd != null) out.push(`over ${formatUsd(f.min_usd)}`);
  if (f.max_usd != null) out.push(`under ${formatUsd(f.max_usd)}`);
  if (f.anomaly_only) out.push("anomalies only");
  f.flags?.forEach((fl) => out.push(FLAG_LABEL[fl] ?? fl));
  if (f.semantic_text) out.push(`"${f.semantic_text}"`);
  return out;
}

export function AiSearch() {
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<SearchResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const run = async (raw: string) => {
    const q = raw.trim();
    if (!q) return;

    abortRef.current?.abort();
    const ctrl = new AbortController();
    abortRef.current = ctrl;

    setQuery(q);
    setBusy(true);
    setError(null);
    try {
      setRes(await ai.search(q, 10, ctrl.signal));
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") return;
      const msg = err instanceof Error ? err.message : "";
      setRes(null);
      setError(
        msg.includes("503")
          ? "AI search is currently unavailable."
          : msg.includes("429")
            ? "You're searching too fast. Please wait a moment."
            : "Search failed. Please try again.",
      );
    } finally {
      if (abortRef.current === ctrl) setBusy(false);
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    void run(query);
  };

  const clear = () => {
    abortRef.current?.abort();
    setQuery("");
    setRes(null);
    setError(null);
    setBusy(false);
  };

  const chips = res ? describeFilters(res.filters) : [];

  return (
    <div className="space-y-3 rounded-2xl border border-slate-500/50 bg-white/30 p-3 shadow-sm backdrop-blur-sm">
      <form onSubmit={onSubmit} className="flex max-w-2xl gap-2">
        <div className="relative flex-1">
          <Sparkles className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-primary" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            maxLength={300}
            placeholder="Find transactions, e.g. big bitcoin transfers today"
            className="h-8 border-slate-500/60 bg-white/60 pl-8 text-sm placeholder:text-slate-600"
          />
        </div>
        <Button type="submit" size="sm" disabled={busy || !query.trim()}>
          {busy ? "Searching…" : "Find"}
        </Button>
        {(res || error) && (
          <Button
            type="button"
            variant="ghost"
            onClick={clear}
            aria-label="Clear search"
          >
            <X className="h-4 w-4" />
          </Button>
        )}
      </form>

      {!res && !error && !busy && (
        <div className="flex flex-wrap gap-2">
          {EXAMPLES.map((ex) => (
            <button
              key={ex}
              type="button"
              onClick={() => void run(ex)}
              className="rounded-full border border-slate-500/50 bg-white/50 px-3 py-1 text-xs text-slate-800 transition-colors hover:bg-white/80"
            >
              {ex}
            </button>
          ))}
        </div>
      )}

      {busy && (
        <div className="space-y-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      )}

      {error && <p className="text-sm text-red-700">{error}</p>}

      {res && !busy && (
        <div className="space-y-3">
          {(res.count > 0 || chips.length > 0) && (
            <div className="flex flex-wrap items-center gap-2 text-xs text-slate-700">
              <span className="font-semibold">
                {res.count} results · understood as:
              </span>
              {chips.length > 0 ? (
                chips.map((c, i) => (
                  <span
                    key={i}
                    className="rounded-full border border-slate-500/40 bg-white/60 px-2.5 py-0.5 text-slate-800"
                  >
                    {c}
                  </span>
                ))
              ) : (
                <span>no filters (showing the largest recent matches)</span>
              )}
            </div>
          )}
          {!res.ai_used && (
            <p className="text-xs text-slate-600">
              AI query parsing wasn&apos;t available, so basic keyword filters
              were used.
            </p>
          )}
          {res.note && res.count > 0 && (
            <p className="text-xs text-slate-700">{res.note}</p>
          )}

          {res.results.length === 0 ? (
            <p className="text-sm text-slate-700">
              Not found.
              {res.available_tokens && res.available_tokens.length > 0 && (
                <>
                  {" "}
                  TidalIQ currently tracks: {res.available_tokens.join(", ")}.
                </>
              )}
            </p>
          ) : (
            <ul className="space-y-2">
              {res.results.map((r) => (
                <li
                  key={r.hash}
                  className="rounded-lg border border-slate-500/30 bg-white/40 p-3 transition-all duration-300 hover:bg-white/60 hover:shadow-sm"
                >
                  <div className="flex items-center justify-between gap-3">
                    <span className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                      {CHAIN_META[r.chain]?.label ?? r.chain} · {r.token}
                      {r.importance_tier && (
                        <span
                          className={`rounded-md px-2 py-0.5 text-[11px] font-semibold ${TIER_TONE[r.importance_tier] ?? ""}`}
                        >
                          {r.importance_tier}
                        </span>
                      )}
                      {r.similarity != null && (
                        <span className="rounded-md bg-primary/10 px-2 py-0.5 text-[11px] font-medium text-primary">
                          semantic match
                        </span>
                      )}
                    </span>
                    <span className="flex items-center gap-2">
                      {r.value_usd != null && (
                        <span className="font-semibold tabular-nums text-emerald-700">
                          {formatUsd(r.value_usd)}
                        </span>
                      )}
                      <a
                        href={r.explorer_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label="View transaction on explorer"
                        className="text-muted-foreground hover:text-primary"
                      >
                        <ExternalLink className="h-4 w-4" />
                      </a>
                    </span>
                  </div>
                  <p className="mt-1 text-xs text-slate-700">
                    {r.summary ??
                      (r.summary_pending
                        ? "Summary pending…"
                        : "No AI summary for this transaction.")}
                  </p>
                  <p className="mt-1 text-[11px] text-slate-600">
                    {r.from} → {r.to} · {r.time}
                  </p>
                </li>
              ))}
            </ul>
          )}

          <p className="text-[11px] text-slate-600">{AI_DISCLAIMER}</p>
        </div>
      )}
    </div>
  );
}