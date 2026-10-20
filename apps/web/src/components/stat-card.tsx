import type { ReactNode } from "react";

interface StatCardProps {
  title: string;
  value: string;
  subtitle?: string;
  icon: ReactNode;
}

export function StatCard({ title, value, subtitle, icon }: StatCardProps) {
  return (
    <div className="group cursor-default rounded-2xl border border-border bg-card p-6 shadow-sm transition-all duration-300 ease-out hover:-translate-y-1.5 hover:border-transparent hover:bg-[#0f1f3d] hover:shadow-xl hover:shadow-slate-900/30">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-medium text-muted-foreground transition-colors duration-300 group-hover:text-slate-300">
          {title}
        </span>
        <span className="transition-transform duration-300 group-hover:scale-110">
          {icon}
        </span>
      </div>

      <div className="mt-8 text-3xl font-bold tracking-tight tabular-nums transition-colors duration-300 group-hover:text-white">
        {value}
      </div>
      <div className="mt-1 min-h-5 text-sm text-muted-foreground transition-colors duration-300 group-hover:text-slate-400">
        {subtitle}
      </div>
    </div>
  );
}