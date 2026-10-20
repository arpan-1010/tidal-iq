"use client";

import { useEffect, useState } from "react";
import { ThumbsDown, ThumbsUp } from "lucide-react";
import { AI_DISCLAIMER, type Narrative } from "@tidaliq/shared";
import { ai } from "@/lib/ai";
import { CitedText } from "@/components/cited-text";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { CHAIN_META, formatUsd } from "@/lib/format";

const WINDOWS = [
  { label: "6H", hours: 6 },
  { label: "24H", hours: 24 },
] as const;

export function NarrativeCard() {
  const [hours, setHours] = useState<number>(6);
  const [data, setData] = useState<Narrative | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [votedId, setVotedId] = useState<number | null>(null);

  const changeWindow = (h: number) => {
    setHours(h);
    setState("loading");
  };

  useEffect(() => {
    const ctrl = new AbortController();

    const load = async () => {
      try {
        const n = await ai.getNarrative(hours, ctrl.signal);
        setData(n);
        setState("ready");
      } catch (err) {
        if (err instanceof DOMException && err.name === "AbortError") return;
        setState("error");
      }
    };

    void load();
    const id = setInterval(() => void load(), 300_000);
    return () => {
      ctrl.abort();
      clearInterval(id);
    };
  }, [hours]);

  const vote = async (helpful: boolean) => {
    if (!data || data.id == null || votedId === data.id) return;
    setVotedId(data.id);
    try {
      await ai.sendFeedback({
        kind: "narrative",
        ref: String(data.id),
        helpful,
      });
    } catch {
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <div className="flex rounded-lg border border-slate-500/50 bg-white/40 p-0.5">
          {WINDOWS.map((w) => (
            <Button
              key={w.label}
              size="xs"
              variant="ghost"
              onClick={() => changeWindow(w.hours)}
              className={
                w.hours === hours
                  ? "bg-slate-900 text-white hover:bg-slate-900 hover:text-white"
                  : "text-slate-800 hover:bg-white/60"
              }
            >
              {w.label}
            </Button>
          ))}
        </div>
      </div>

      {state === "loading" && (
        <div className="space-y-2">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-11/12" />
          <Skeleton className="h-4 w-3/4" />
        </div>
      )}

      {state === "error" && (
        <p className="text-sm text-slate-700">
          Couldn&apos;t load the narrative right now.
        </p>
      )}

      {state === "ready" && !data && (
        <p className="text-sm text-slate-700">
          No tracked transactions in this window.
        </p>
      )}

      {state === "ready" && data && (
        <>
          <p className="text-sm leading-relaxed text-slate-900">
            <CitedText text={data.text} sources={data.citations} />
          </p>

          <div className="flex flex-wrap gap-2">
            {data.stats.map((s) => (
              <span
                key={s.chain}
                className="rounded-full border border-slate-500/40 bg-white/50 px-2.5 py-0.5 text-xs text-slate-800"
              >
                {CHAIN_META[s.chain]?.label ?? s.chain} · {s.count} txs ·{" "}
                {formatUsd(s.total_usd)}
              </span>
            ))}
          </div>

          <div className="flex items-center justify-between">
            <p className="text-[11px] text-slate-600">{AI_DISCLAIMER}</p>
            {data.id != null && (
              <div className="flex items-center gap-1 text-slate-600">
                <Button
                  size="icon-sm"
                  variant="ghost"
                  disabled={votedId === data.id}
                  onClick={() => void vote(true)}
                  aria-label="Helpful"
                >
                  <ThumbsUp className="h-3.5 w-3.5" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  disabled={votedId === data.id}
                  onClick={() => void vote(false)}
                  aria-label="Not helpful"
                >
                  <ThumbsDown className="h-3.5 w-3.5" />
                </Button>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}