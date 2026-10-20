"use client";

import { Fragment, useState } from "react";
import { ArrowRight, ChevronDown, ExternalLink } from "lucide-react";
import { TxDetail } from "@/components/tx-detail";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  CHAIN_META,
  formatAmount,
  formatUsd,
  impactTier,
  shortAddr,
  timeAgo,
} from "@/lib/format";
import type { Transaction } from "@/lib/types";

interface WhaleFeedProps {
  transactions: Transaction[];
  loading: boolean;
}

function ScoreBadge({
  score,
  anomaly,
  reason,
}: {
  score: number | null;
  anomaly: boolean | null;
  reason: string | null;
}) {
  if (score == null) return <span className="text-muted-foreground">…</span>;
  const tone =
    score >= 67
      ? "bg-red-600/15 text-red-700"
      : score >= 33
        ? "bg-amber-500/20 text-amber-800"
        : "bg-slate-500/15 text-slate-700";
  return (
    <span
      className="inline-flex items-center gap-1.5"
      title={reason ?? undefined}
    >
      <span
        className={`rounded-md px-2 py-0.5 text-xs font-semibold tabular-nums ${tone}`}
      >
        {Math.round(score)}
      </span>
    </span>
  );
}

const HEAD =
  "text-center text-xs font-semibold uppercase tracking-wide text-slate-700";

function AddrLink({ chain, addr }: { chain: string; addr: string }) {
  const meta = CHAIN_META[chain];
  const [first, ...rest] = addr.split(" ");
  const more = rest.join(" ");
  const label = shortAddr(first ?? addr);

  return (
    <span className="inline-flex min-w-0 flex-col items-center leading-tight">
      {first && meta ? (
        <a
          href={meta.addrUrl(first)}
          target="_blank"
          rel="noopener noreferrer"
          className="font-mono text-xs text-slate-800 hover:text-primary hover:underline"
        >
          {label}
        </a>
      ) : (
        <span className="font-mono text-xs">{label}</span>
      )}
      {more && (
        <span className="text-[10px] text-muted-foreground">{more}</span>
      )}
    </span>
  );
}

export function WhaleFeed({ transactions, loading }: WhaleFeedProps) {
  const [openHash, setOpenHash] = useState<string | null>(null);

  if (loading) {
    return (
      <div className="space-y-3 p-4">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-10 w-full" />
        ))}
      </div>
    );
  }

  if (transactions.length === 0) {
    return (
      <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
        No matching whale transactions. Watching the chain...
      </div>
    );
  }

  return (
    <Table className="table-fixed">
      <TableHeader>
        <TableRow className="border-slate-500/40 bg-slate-900/10 hover:bg-slate-900/10">
          <TableHead className={`${HEAD} w-[11%]`}>Chain</TableHead>
          <TableHead className={`${HEAD} w-[15%]`}>Amount</TableHead>
          <TableHead className={`${HEAD} w-[19%]`}>Value</TableHead>
          <TableHead className={`${HEAD} w-[8%]`}>Score</TableHead>
          <TableHead className={`${HEAD} w-[28%]`}>From → To</TableHead>
          <TableHead className={`${HEAD} w-[9%]`}>Time</TableHead>
          <TableHead className="w-[10%]" />
        </TableRow>
      </TableHeader>
      <TableBody>
        {transactions.map((tx) => {
          const meta = CHAIN_META[tx.chain];
          const tier = impactTier(tx.value_usd);
          const open = openHash === tx.hash;
          return (
            <Fragment key={tx.id}>
              <TableRow className="group cursor-default border-slate-500/30 transition-all duration-300 hover:bg-white/60 hover:shadow-sm">

                <TableCell className="whitespace-normal">
                  <div className="flex justify-center transition-transform duration-300 group-hover:scale-110">
                    <Badge variant="outline" className={meta?.badge}>
                      {meta?.label ?? tx.chain}
                    </Badge>
                  </div>
                </TableCell>

                <TableCell className="whitespace-normal text-center font-medium tabular-nums transition-colors group-hover:text-slate-950">
                  {meta ? formatAmount(tx.value, meta.decimals) : tx.value}{" "}
                  <span className="text-muted-foreground">
                    {tx.token_symbol}
                  </span>
                </TableCell>

                <TableCell className="whitespace-normal">
                  <div className="flex flex-wrap items-center justify-center gap-x-2 gap-y-1">
                    <span className="inline-block font-semibold tabular-nums text-emerald-700 transition-all duration-300 group-hover:scale-105 group-hover:font-bold">
                      {formatUsd(tx.value_usd)}
                    </span>
                    <Badge variant="outline" className={tier.className}>
                      {tier.label}
                    </Badge>
                  </div>
                </TableCell>

                <TableCell className="whitespace-normal">
                  <div className="flex justify-center">
                    <ScoreBadge
                      score={tx.impact_score}
                      anomaly={tx.is_anomaly}
                      reason={tx.anomaly_reason}
                    />
                  </div>
                </TableCell>

                <TableCell className="whitespace-normal">
                  <div className="flex flex-wrap items-center justify-center gap-x-1.5 gap-y-1">
                    <AddrLink chain={tx.chain} addr={tx.from_address} />
                    <ArrowRight className="h-3 w-3 shrink-0 text-muted-foreground transition-transform duration-300 group-hover:translate-x-0.5" />
                    <AddrLink chain={tx.chain} addr={tx.to_address} />
                  </div>
                </TableCell>

                <TableCell className="whitespace-normal text-center text-xs text-muted-foreground transition-colors group-hover:text-slate-900">
                  {timeAgo(tx.tx_timestamp)}
                </TableCell>

                <TableCell className="whitespace-normal">
                  <div className="flex items-center justify-center gap-2">
                    <button
                      type="button"
                      onClick={() => setOpenHash(open ? null : tx.hash)}
                      aria-label={
                        open ? "Hide AI analysis" : "Show AI analysis"
                      }
                      aria-expanded={open}
                      className="text-muted-foreground transition-colors hover:text-primary"
                    >
                      <ChevronDown
                        className={`h-4 w-4 transition-transform duration-300 ${
                          open ? "rotate-180" : ""
                        }`}
                      />
                    </button>
                    {meta && (
                      <a
                        href={meta.txUrl(tx.hash)}
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label="View transaction on explorer"
                        className="text-muted-foreground transition-all duration-300 hover:text-primary group-hover:scale-125 group-hover:text-slate-900"
                      >
                        <ExternalLink className="h-4 w-4" />
                      </a>
                    )}
                  </div>
                </TableCell>
              </TableRow>

              {open && (
                <TableRow className="border-slate-500/30 bg-white/40 hover:bg-white/40">
                  <TableCell
                    colSpan={7}
                    className="whitespace-normal break-words px-6"
                  >
                    <div className="min-w-0 max-w-full">
                      <TxDetail tx={tx} />
                    </div>
                  </TableCell>
                </TableRow>
              )}
            </Fragment>
          );
        })}
      </TableBody>
    </Table>
  );
}