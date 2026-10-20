const TABS = [
  {
    value: "all",
    label: "All chains",
    idle: "text-slate-900 hover:bg-white/60",
    active: "border-slate-900 bg-slate-900 text-white",
  },
  {
    value: "ethereum",
    label: "ETH",
    idle: "text-indigo-800 hover:bg-indigo-100",
    active: "border-indigo-600 bg-indigo-600 text-white",
  },
  {
    value: "solana",
    label: "SOL",
    idle: "text-fuchsia-800 hover:bg-fuchsia-100",
    active: "border-fuchsia-600 bg-fuchsia-600 text-white",
  },
  {
    value: "bitcoin",
    label: "BTC",
    idle: "text-orange-800 hover:bg-orange-100",
    active: "border-orange-600 bg-orange-600 text-white",
  },
];

export function ChainTabs({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {TABS.map((tab) => {
        const selected = value === tab.value;
        return (
          <button
            key={tab.value}
            type="button"
            onClick={() => onChange(tab.value)}
            className={`rounded-lg border px-4 py-1.5 text-sm font-semibold shadow-sm transition-colors ${
              selected
                ? tab.active
                : `border-slate-500/50 bg-white/50 ${tab.idle}`
            }`}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}