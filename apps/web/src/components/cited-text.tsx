import { CHAIN_META, formatUsd } from "@/lib/format";

export interface CiteSource {
  n: number;
  chain: string;
  token: string;
  value_usd: number | null;
  explorer_url: string;
}

export function CitedText({
  text,
  sources,
}: {
  text: string;
  sources: CiteSource[];
}) {
  return (
    <>
      {text.split(/(\[\d+\])/g).map((part, i) => {
        const m = /^\[(\d+)\]$/.exec(part);
        if (!m) return part;
        const n = Number(m[1]);
        const src = sources.find((s) => s.n === n);
        if (!src?.explorer_url) return part;
        const title = `${CHAIN_META[src.chain]?.label ?? src.chain} ${src.token}${
          src.value_usd != null ? ` ${formatUsd(src.value_usd)}` : ""
        }`;
        return (
          <a
            key={i}
            href={src.explorer_url}
            target="_blank"
            rel="noopener noreferrer"
            title={title}
            className="mx-0.5 rounded bg-slate-900/10 px-1 text-xs font-semibold text-slate-900 hover:bg-slate-900 hover:text-white"
          >
            {n}
          </a>
        );
      })}
    </>
  );
}