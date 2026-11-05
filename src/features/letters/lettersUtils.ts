import { useEffect, useState } from "react";
import type { LetterSummary } from "../../lib/api";

export type LetterTab = "letters" | "capsules" | "drafts";

/** Ticks so countdowns and "ready to open" states stay fresh without reloading. */
export function useNow(everyMs = 30_000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(t);
  }, [everyMs]);
  return now;
}

/** "in 12 days", "in 3 hours", "in 5 minutes", "any moment now". */
export function countdown(unlockIso: string, now: number): string {
  const ms = new Date(unlockIso).getTime() - now;
  if (ms <= 0) return "any moment now";
  const min = Math.ceil(ms / 60_000);
  if (min < 60) return `in ${min} minute${min === 1 ? "" : "s"}`;
  const hours = Math.ceil(ms / 3_600_000);
  if (hours < 48) return `in ${hours} hour${hours === 1 ? "" : "s"}`;
  const days = Math.ceil(ms / 86_400_000);
  if (days < 60) return `in ${days} days`;
  const months = Math.round(days / 30.4);
  if (months < 24) return `in about ${months} months`;
  const years = Math.round(days / 365.25);
  return `in about ${years} years`;
}

/** "Jan 5, 2027, 9:00 AM" in the viewer's own time zone. */
export function fmtMoment(iso: string): string {
  return new Date(iso).toLocaleString(undefined, { month: "short", day: "numeric", year: "numeric", hour: "numeric", minute: "2-digit" });
}

export function fmtDay(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

const pad = (n: number) => String(n).padStart(2, "0");

/** ISO instant -> value for <input type="datetime-local"> (local time). */
export function toLocalInput(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** <input type="datetime-local"> value -> ISO instant, or null when empty/invalid. */
export function fromLocalInput(v: string): string | null {
  if (!v) return null;
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? null : d.toISOString();
}

/** Earliest pickable moment: a few minutes from now. */
export function minLocalInput(now = Date.now()): string {
  return toLocalInput(new Date(now + 5 * 60_000).toISOString());
}

export function isFuture(iso: string | null, now = Date.now()): boolean {
  return !!iso && new Date(iso).getTime() > now;
}

/** Has the moment come for a capsule you received that is not open yet? */
export function readyToOpen(l: LetterSummary, now: number): boolean {
  return !l.mine && l.kind === "capsule" && l.status === "sent" && !l.unlockedAt && !!l.unlockAt && new Date(l.unlockAt).getTime() <= now;
}

export function stillSealed(l: LetterSummary, now: number): boolean {
  return l.kind === "capsule" && l.status === "sent" && !l.unlockedAt && !!l.unlockAt && new Date(l.unlockAt).getTime() > now;
}

export interface Grouped {
  received: LetterSummary[];
  sent: LetterSummary[];
  drafts: LetterSummary[];
}

/** Sorts letters into the tab's two shelves (received / sent) plus drafts. */
export function group(all: LetterSummary[], tab: LetterTab): Grouped {
  const drafts = all.filter((l) => l.status === "draft");
  const sentOnes = all.filter((l) => l.status === "sent");
  if (tab === "drafts") return { received: [], sent: [], drafts };
  const kind = tab === "letters" ? "letter" : "capsule";
  const ofKind = sentOnes.filter((l) => l.kind === kind);
  return { received: ofKind.filter((l) => !l.mine), sent: ofKind.filter((l) => l.mine), drafts: [] };
}

/** Capsule you wrote (or received) that opens soonest, for the hero. */
export function nextToOpen(all: LetterSummary[], now: number): LetterSummary | null {
  const sealed = all.filter((l) => stillSealed(l, now));
  sealed.sort((a, b) => new Date(a.unlockAt!).getTime() - new Date(b.unlockAt!).getTime());
  return sealed[0] ?? null;
}

/** What to call the person a letter is going to. */
export function toName(l: Pick<LetterSummary, "recipientName">): string {
  return l.recipientName ?? "your person";
}
