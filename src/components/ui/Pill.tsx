import type { ReactNode } from "react";

const tones = {
  accent: "bg-accent-soft text-accent-deep",
  green: "bg-sage text-[#4f7a55]",
  gold: "bg-gold text-[#9a7448]",
  gray: "bg-paper-2 text-muted",
} as const;

export function Pill({ tone = "accent", children }: { tone?: keyof typeof tones; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1.5 text-[10.5px] font-bold ${tones[tone]}`}>
      {children}
    </span>
  );
}
