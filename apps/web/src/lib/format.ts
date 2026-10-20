export const CHAIN_META: Record<
  string,
  {
    label: string;
    decimals: number;
    badge: string;
    dot: string;
    txUrl: (hash: string) => string;
    addrUrl: (addr: string) => string;
  }
> = {
  ethereum: {
    label: "Ethereum",
    decimals: 18,
    badge: "border-indigo-500/40 bg-indigo-500/10 text-indigo-700",
    dot: "bg-indigo-400",
    txUrl: (h) => `https://etherscan.io/tx/${h}`,
    addrUrl: (a) => `https://etherscan.io/address/${a}`,
  },
  solana: {
    label: "Solana",
    decimals: 9,
    badge: "border-fuchsia-500/40 bg-fuchsia-500/10 text-fuchsia-700",
    dot: "bg-fuchsia-400",
    txUrl: (h) => `https://solscan.io/tx/${h}`,
    addrUrl: (a) => `https://solscan.io/account/${a}`,
  },
  bitcoin: {
    label: "Bitcoin",
    decimals: 8,
    badge: "border-orange-500/40 bg-orange-500/10 text-orange-700",
    dot: "bg-orange-400",
    txUrl: (h) => `https://mempool.space/tx/${h}`,
    addrUrl: (a) => `https://mempool.space/address/${a}`,
  },
};

export function formatUsd(value: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    notation: "compact",
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  }).format(value);
}

export function formatAmount(raw: string, decimals: number) {
  const n = Number(raw) / 10 ** decimals;
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: n >= 1000 ? 0 : 2,
  }).format(n);
}

export function shortAddr(addr: string) {
  if (!addr) return "—";
  const [first, ...extra] = addr.split(" ");
  const short =
    first.length > 12 ? `${first.slice(0, 6)}…${first.slice(-4)}` : first;
  return extra.length ? `${short} ${extra.join(" ")}` : short;
}

export function timeAgo(timestamp: string) {
  const iso = /(Z|[+-]\d{2}:?\d{2})$/.test(timestamp)
    ? timestamp
    : `${timestamp}Z`;
  const diff = Math.max(
    0,
    Math.floor((Date.now() - new Date(iso).getTime()) / 1000),
  );
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

export function impactTier(usd: number) {
  if (usd >= 10_000_000)
    return {
      label: "Mega",
      className: "border-red-500/40 bg-red-500/10 text-red-700",
    };
  if (usd >= 1_000_000)
    return {
      label: "Large",
      className: "border-amber-500/50 bg-amber-500/15 text-amber-700",
    };
  if (usd >= 250_000)
    return {
      label: "Medium",
      className: "border-sky-500/40 bg-sky-500/10 text-sky-700",
    };
  return {
    label: "Standard",
    className: "border-border bg-muted/50 text-muted-foreground",
  };
}