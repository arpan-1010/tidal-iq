"use client";

import { useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { WhaleFeed } from "@/components/whale-feed";
import type { Transaction } from "@/lib/types";

const PAGE_SIZE = 10;

export function PagedFeed({
  transactions,
  loading,
}: {
  transactions: Transaction[];
  loading: boolean;
}) {
  const [page, setPage] = useState(0);
  const [dir, setDir] = useState<1 | -1>(1);

  const total = transactions.length;
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const current = Math.min(page, pageCount - 1);
  const slice = transactions.slice(
    current * PAGE_SIZE,
    (current + 1) * PAGE_SIZE,
  );

  const go = (next: number) => {
    const n = Math.max(0, Math.min(pageCount - 1, next));
    setDir(n >= current ? 1 : -1);
    setPage(n);
  };

  return (
    <div>
      <div className="overflow-hidden">
        <div
          key={current}
          className={`animate-in fade-in duration-300 ${
            dir === 1 ? "slide-in-from-right-8" : "slide-in-from-left-8"
          }`}
        >
          <WhaleFeed transactions={slice} loading={loading} />
        </div>
      </div>

      {!loading && total > 0 && (
        <div className="flex items-center justify-between border-t border-slate-500/40 bg-slate-900/5 px-4 py-3">
          <span className="text-xs text-muted-foreground">
            {current * PAGE_SIZE + 1}–
            {Math.min(total, (current + 1) * PAGE_SIZE)} of {total}
          </span>

          <div className="flex items-center gap-2">
            <Button
              size="icon-sm"
              variant="ghost"
              disabled={current === 0}
              onClick={() => go(current - 1)}
              aria-label="Previous page"
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>

            <div className="flex items-center gap-1.5">
              {Array.from({ length: pageCount }).map((_, i) => (
                <button
                  key={i}
                  onClick={() => go(i)}
                  aria-label={`Page ${i + 1}`}
                  className={`h-2 rounded-full transition-all ${
                    i === current
                      ? "w-5 bg-primary"
                      : "w-2 bg-muted-foreground/40 hover:bg-muted-foreground"
                  }`}
                />
              ))}
            </div>

            <Button
              size="icon-sm"
              variant="ghost"
              disabled={current >= pageCount - 1}
              onClick={() => go(current + 1)}
              aria-label="Next page"
            >
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}