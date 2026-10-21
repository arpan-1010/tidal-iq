import Image from "next/image";
import { ArrowUp, ChevronRight } from "lucide-react";

const LINKS = [
  { href: "#alerts", label: "Alerts" },
  { href: "#ask", label: "Ask AI" },
  { href: "#analytics", label: "Analytics" },
  { href: "#insights", label: "AI Insights" },
];

const CHAINS = [
  { sym: "ETH", name: "Ethereum", dot: "bg-indigo-400" },
  { sym: "SOL", name: "Solana", dot: "bg-fuchsia-400" },
  { sym: "BTC", name: "Bitcoin", dot: "bg-orange-400" },
];

export function SiteFooter() {
  return (
    <footer className="relative mt-10 overflow-hidden bg-[#0f1f3d] pb-24 text-slate-300 md:pb-0">

      <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-sky-400/60 to-transparent" />
      <div className="pointer-events-none absolute -top-24 left-1/2 h-48 w-96 -translate-x-1/2 rounded-full bg-sky-500/10 blur-3xl" />

      <div className="relative mx-auto grid max-w-7xl gap-10 px-4 py-12 sm:px-6 md:grid-cols-[1.4fr_1fr_1fr]">

        <div className="space-y-4">
          <a href="#top" className="inline-flex items-center gap-3">
            <span className="relative block h-11 w-11 shrink-0 overflow-hidden rounded-xl bg-[#010b26] ring-1 ring-white/10">
              <Image
                src="/brand/tidaliq.png"
                alt="TidalIQ logo"
                width={1254}
                height={1254}
                sizes="80px"
                className="absolute max-w-none"
                style={{
                  width: "153%",
                  height: "auto",
                  left: "-27.4%",
                  top: "-26.6%",
                }}
              />
            </span>
            <span className="text-2xl font-extrabold leading-none tracking-normal">
              <span className="text-white">Tidal</span>
              <span className="text-sky-400">IQ</span>
            </span>
          </a>

          <p className="max-w-xs text-sm leading-relaxed text-slate-400">
            Real-time large transactions across Ethereum, Solana and Bitcoin,
            with intelligence layered on top.
          </p>

          <span className="inline-flex items-center gap-2 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-3 py-1 text-xs font-medium text-emerald-300">
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-60" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-400" />
            </span>
            Live on-chain data
          </span>
        </div>

        <div className="grid grid-cols-2 gap-8 md:contents">
          <div className="space-y-4">
            <h3 className="text-xs font-semibold uppercase tracking-widest text-sky-300/80">
              Explore
            </h3>
            <ul className="space-y-2.5 text-sm">
              {LINKS.map((l) => (
                <li key={l.href}>
                  <a
                    href={l.href}
                    className="group inline-flex items-center gap-1 text-slate-300 transition-colors duration-200 hover:text-white"
                  >
                    <ChevronRight className="h-3.5 w-3.5 -ml-1 text-sky-400/0 transition-all duration-200 group-hover:translate-x-1 group-hover:text-sky-400" />
                    {l.label}
                  </a>
                </li>
              ))}
            </ul>
          </div>

          <div className="space-y-4">
            <h3 className="text-xs font-semibold uppercase tracking-widest text-sky-300/80">
              Chains tracked
            </h3>
            <div className="flex flex-wrap gap-2">
              {CHAINS.map((c) => (
                <span
                  key={c.sym}
                  title={c.name}
                  className="inline-flex items-center gap-1.5 rounded-full border border-white/15 bg-white/5 px-3 py-1 text-xs font-semibold text-slate-200 transition-colors hover:bg-white/15"
                >
                  <span className={`h-1.5 w-1.5 rounded-full ${c.dot}`} />
                  {c.sym}
                </span>
              ))}
            </div>
            <p className="text-xs leading-relaxed text-slate-400">
              Data comes from public blockchains.
            </p>
          </div>
        </div>
      </div>

      <div className="relative border-t border-white/10">
        <div className="mx-auto flex max-w-7xl flex-col items-center justify-between gap-3 px-4 py-5 text-xs text-slate-400 sm:flex-row sm:px-6">
          <span>© {new Date().getFullYear()} TidalIQ. All rights reserved.</span>
          <span className="order-last sm:order-none">
            AI-generated insights, not financial advice.
          </span>
          <a
            href="#top"
            className="inline-flex items-center gap-1.5 rounded-full border border-white/15 bg-white/5 px-3 py-1 font-medium text-slate-200 transition-colors hover:bg-white/15"
          >
            Back to top
            <ArrowUp className="h-3 w-3" />
          </a>
        </div>
      </div>
    </footer>
  );
}