"use client";

import { useEffect, useState } from "react";
import {
  BarChart3,
  Bell,
  Home,
  Lightbulb,
  Sparkles,
  type LucideIcon,
} from "lucide-react";

interface NavItem {
  id: string;
  label: string;
  icon: LucideIcon;
}

const NAV: NavItem[] = [
  { id: "top", label: "Home", icon: Home },
  { id: "alerts", label: "Alerts", icon: Bell },
  { id: "ask", label: "Ask AI", icon: Sparkles },
  { id: "analytics", label: "Analytics", icon: BarChart3 },
  { id: "insights", label: "Insights", icon: Lightbulb },
];

const TRACKED = ["alerts", "ask", "analytics", "insights"];

export function SiteHeader() {
  const [active, setActive] = useState("top");

  useEffect(() => {
    const onScroll = () => {
      const line = window.innerHeight * 0.4;
      let current = "top";
      for (const id of TRACKED) {
        const el = document.getElementById(id);
        if (el && el.getBoundingClientRect().top <= line) current = id;
      }

      const atBottom =
        window.innerHeight + window.scrollY >=
        document.documentElement.scrollHeight - 4;
      if (atBottom) current = TRACKED[TRACKED.length - 1];

      setActive(current);
    };

    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <>
      <header className="pointer-events-none sticky top-4 z-40 hidden justify-center px-4 md:flex">
        <nav className="pointer-events-auto flex max-w-full items-center gap-1 rounded-full border border-white/10 bg-card/60 p-1.5 shadow-lg shadow-black/20 backdrop-blur-md">
          {NAV.map((item) => {
            const isActive = active === item.id;
            const isHome = item.id === "top";
            const Icon = item.icon;
            return (
              <a
                key={item.id}
                href={`#${item.id}`}
                aria-label={isHome ? item.label : undefined}
                className={`relative flex items-center justify-center whitespace-nowrap rounded-full px-5 py-2.5 text-sm font-medium transition-colors ${
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {isHome ? <Icon className="h-4 w-4" /> : item.label}
              </a>
            );
          })}
        </nav>
      </header>

      <nav
        aria-label="Primary"
        className="fixed inset-x-3 bottom-3 z-50 md:hidden"
        style={{ paddingBottom: "env(safe-area-inset-bottom)" }}
      >
        <div className="mx-auto flex max-w-md items-stretch gap-1 rounded-2xl border border-white/40 bg-card/70 p-1.5 shadow-xl shadow-black/25 backdrop-blur-xl">
          {NAV.map((item) => {
            const isActive = active === item.id;
            const Icon = item.icon;
            return (
              <a
                key={item.id}
                href={`#${item.id}`}
                aria-current={isActive ? "page" : undefined}
                className={`flex flex-1 flex-col items-center justify-center gap-0.5 rounded-xl py-1.5 text-[10px] font-medium leading-tight transition-all duration-300 ${
                  isActive
                    ? "bg-primary text-primary-foreground shadow-md"
                    : "text-muted-foreground active:scale-95"
                }`}
              >
                <Icon
                  className={`h-5 w-5 transition-transform duration-300 ${
                    isActive ? "scale-110" : ""
                  }`}
                />
                <span className="whitespace-nowrap">{item.label}</span>
              </a>
            );
          })}
        </div>
      </nav>
    </>
  );
}