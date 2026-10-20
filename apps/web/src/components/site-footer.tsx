import Image from "next/image";

const LINKS = [
  { href: "#alerts", label: "Alerts" },
  { href: "#analytics", label: "Analytics" },
  { href: "#insights", label: "AI Insights" },
];

const CHAINS = ["ETH", "SOL", "BTC"];

export function SiteFooter() {
  return (
    <footer className="mt-10 bg-[#0f1f3d] text-slate-300">
      <div className="mx-auto grid max-w-7xl gap-10 px-4 py-12 sm:px-6 md:grid-cols-3">

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
        </div>

        <div className="space-y-4">
          <h3 className="text-xs font-semibold uppercase tracking-widest text-slate-400">
            Explore
          </h3>
          <ul className="space-y-2.5 text-sm">
            {LINKS.map((l) => (
              <li key={l.href}>
                <a
                  href={l.href}
                  className="inline-block transition-all duration-200 hover:translate-x-1 hover:text-white"
                >
                  {l.label}
                </a>
              </li>
            ))}
          </ul>
        </div>

        <div className="space-y-4">
          <h3 className="text-xs font-semibold uppercase tracking-widest text-slate-400">
            Chains tracked
          </h3>
          <div className="flex flex-wrap gap-2">
            {CHAINS.map((c) => (
              <span
                key={c}
                className="rounded-full border border-white/15 bg-white/5 px-3.5 py-1 text-xs font-semibold text-slate-200 transition-colors hover:bg-white/15"
              >
                {c}
              </span>
            ))}
          </div>
          <p className="text-xs text-slate-400">
            Data comes from public blockchains.
          </p>
        </div>
      </div>

      <div className="border-t border-white/10">
        <div className="mx-auto flex max-w-7xl flex-col items-center justify-between gap-2 px-4 py-5 text-xs text-slate-400 sm:flex-row sm:px-6">
          <span>© TidalIQ. All rights reserved.</span>
          <span>Not financial advice.</span>
        </div>
      </div>
    </footer>
  );
}