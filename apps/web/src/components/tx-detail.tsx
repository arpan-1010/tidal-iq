"use client";

import { useEffect, useState } from "react";
import { ThumbsDown, ThumbsUp, Wallet } from "lucide-react";
import {
  AI_DISCLAIMER,
  type AISummary,
  type WalletProfile,
} from "@tidaliq/shared";
import { ai } from "@/lib/ai";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatUsd, shortAddr } from "@/lib/format";
import type { Transaction } from "@/lib/types";

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

function Thumbs({
  vote,
  onVote,
}: {
  vote: "up" | "down" | null;
  onVote: (helpful: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-1 text-slate-600">
      <Button
        size="icon-sm"
        variant="ghost"
        disabled={!!vote}
        onClick={() => onVote(true)}
        aria-label="Helpful"
        className={vote === "up" ? "text-emerald-700" : ""}
      >
        <ThumbsUp className="h-3.5 w-3.5" />
      </Button>
      <Button
        size="icon-sm"
        variant="ghost"
        disabled={!!vote}
        onClick={() => onVote(false)}
        aria-label="Not helpful"
        className={vote === "down" ? "text-red-700" : ""}
      >
        <ThumbsDown className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}

function WalletProfileBox({
  chain,
  address,
}: {
  chain: string;
  address: string;
}) {
  const [state, setState] = useState<
    "idle" | "loading" | "ready" | "empty" | "error"
  >("idle");
  const [profile, setProfile] = useState<WalletProfile | null>(null);
  const [vote, setVote] = useState<"up" | "down" | null>(null);

  const load = async () => {
    setState("loading");
    try {
      const p = await ai.getWalletProfile(chain, address);
      if (!p) {
        setState("empty");
        return;
      }
      setProfile(p);
      setState("ready");
    } catch {
      setState("error");
    }
  };

  const sendVote = async (helpful: boolean) => {
    if (vote) return;
    setVote(helpful ? "up" : "down");
    try {
      await ai.sendFeedback({
        kind: "profile",
        ref: `${chain}:${address}`.slice(0, 200),
        helpful,
      });
    } catch {
    }
  };

  if (state === "idle") {
    return (
      <Button
        size="xs"
        variant="ghost"
        onClick={() => void load()}
        className="border border-slate-500/50 bg-white/50"
      >
        <Wallet className="h-3.5 w-3.5" />
        Profile sender wallet
      </Button>
    );
  }
  if (state === "loading") return <Skeleton className="h-12 w-full" />;
  if (state === "empty")
    return (
      <p className="text-xs text-slate-700">
        No tracked activity found for this wallet.
      </p>
    );
  if (state === "error" || !profile) {
    return (
      <p className="text-xs text-slate-700">
        Couldn&apos;t load the wallet profile right now.
      </p>
    );
  }

  const s = profile.stats;
  return (
    <div className="space-y-2 rounded-lg border border-slate-500/30 bg-white/50 p-3">
      <p className="text-xs font-semibold uppercase tracking-wide text-slate-700">
        Sender wallet · {profile.label ?? shortAddr(profile.address)}
      </p>
      <p className="text-sm text-slate-900">{profile.profile}</p>
      <div className="flex flex-wrap gap-2 text-xs text-slate-800">
        <span className="rounded-full border border-slate-500/40 bg-white/60 px-2.5 py-0.5">
          Sent {s.sent_count} · {formatUsd(s.sent_total_usd)}
        </span>
        <span className="rounded-full border border-slate-500/40 bg-white/60 px-2.5 py-0.5">
          Received {s.received_count} · {formatUsd(s.received_total_usd)}
        </span>
        {s.first_seen && (
          <span className="rounded-full border border-slate-500/40 bg-white/60 px-2.5 py-0.5">
            Tracked since {s.first_seen}
          </span>
        )}
        {s.top_tokens[0] && (
          <span className="rounded-full border border-slate-500/40 bg-white/60 px-2.5 py-0.5">
            Top token {s.top_tokens[0].token}
          </span>
        )}
      </div>
      <div className="flex items-center justify-between gap-3">
        <p className="text-[11px] text-slate-600">
          Based only on large transfers tracked by TidalIQ.
        </p>
        <Thumbs vote={vote} onVote={(h) => void sendVote(h)} />
      </div>
    </div>
  );
}

export function TxDetail({ tx }: { tx: Transaction }) {
  const [analysis, setAnalysis] = useState<AISummary | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [vote, setVote] = useState<"up" | "down" | null>(null);

  useEffect(() => {
    const ctrl = new AbortController();
    ai.getAnalysis(tx.hash, ctrl.signal)
      .then((a) => {
        setAnalysis(a);
        setState("ready");
      })
      .catch((err) => {
        if (err instanceof DOMException && err.name === "AbortError") return;
        setState("error");
      });
    return () => ctrl.abort();
  }, [tx.hash]);

  const sendVote = async (helpful: boolean) => {
    if (vote) return;
    setVote(helpful ? "up" : "down");
    try {
      await ai.sendFeedback({ kind: "summary", ref: tx.hash, helpful });
    } catch {
    }
  };

  if (state === "loading") {
    return (
      <div className="space-y-2 py-1">
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
      </div>
    );
  }

  const summary = analysis?.summary ?? tx.ai_summary;

  return (
    <div className="space-y-3 py-1 text-left">
      {!summary ? (
        <p className="text-sm text-slate-700">
          No AI analysis for this transaction yet. Larger and more unusual
          transfers are analysed first.
        </p>
      ) : (
        <div className="space-y-2">
          <p className="text-sm text-slate-900">{summary}</p>

          {analysis && (
            <div className="flex flex-wrap items-center gap-2 text-xs">
              <span
                className={`rounded-md px-2 py-0.5 font-semibold ${TIER_TONE[analysis.importance_tier] ?? ""}`}
              >
                {analysis.importance_tier} ·{" "}
                {Math.round(analysis.importance_score)}
              </span>
              {analysis.importance_reason && (
                <span className="text-slate-700">
                  {analysis.importance_reason}
                </span>
              )}
            </div>
          )}

          {analysis && analysis.anomaly_flags.length > 0 && (
            <ul className="flex flex-wrap gap-2">
              {analysis.anomaly_flags.map((f) => (
                <li
                  key={f.code}
                  title={f.detail}
                  className="rounded-full border border-red-500/40 bg-red-500/10 px-2.5 py-0.5 text-xs font-medium text-red-700"
                >
                  {FLAG_LABEL[f.code] ?? f.code}
                </li>
              ))}
            </ul>
          )}

          <div className="flex items-center justify-between gap-3">
            <p className="text-[11px] text-slate-600">
              {analysis?.source === "template" ? "Generated from data. " : ""}
              {AI_DISCLAIMER}
            </p>
            {analysis && (
              <Thumbs vote={vote} onVote={(h) => void sendVote(h)} />
            )}
          </div>
        </div>
      )}

      <WalletProfileBox chain={tx.chain} address={tx.from_address} />
    </div>
  );
}