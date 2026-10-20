"use client";

import { useEffect, useState } from "react";
import { Home } from "lucide-react";

const NAV = [
  { id: "top", label: "Home", icon: Home },
  { id: "alerts", label: "Alerts" },
  { id: "ask", label: "Ask AI" },
  { id: "analytics", label: "Analytics" },
  { id: "insights", label: "AI Insights" },
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
    <header className="pointer-events-none sticky top-4 z-40 flex justify-center px-4">
      <nav className="pointer-events-auto flex max-w-full items-center gap-1 rounded-full border border-white/10 bg-card/60 p-1.5 shadow-lg shadow-black/20 backdrop-blur-md">
        {NAV.map((item) => {
          const isActive = active === item.id;
          const Icon = item.icon;
          return (
            <a
              key={item.id}
              href={`#${item.id}`}
              aria-label={Icon ? item.label : undefined}
              className={`relative flex items-center justify-center rounded-full px-3 py-2.5 text-sm font-medium transition-colors sm:px-5 ${
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {Icon ? <Icon className="h-4 w-4" /> : item.label}
            </a>
          );
        })}
      </nav>
    </header>
  );
}