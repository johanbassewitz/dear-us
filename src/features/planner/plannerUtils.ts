import library from "../../data/dateIdeas.json";
import type { CustomIdea, Plan } from "../../lib/api";

export interface DateIdea {
  id: string;
  title: string;
  description: string;
  category: string;
  budget: string;
  duration: string;
  setting: string;
  mood: string;
  /** true for ideas the couple added themselves */
  custom: boolean;
}

export const BUDGETS = ["free", "low", "medium", "high"] as const;
export const DURATIONS = ["short", "half-day", "full-day"] as const;
export const SETTINGS = ["indoor", "outdoor", "either"] as const;
export const MOODS = ["cozy", "playful", "relaxed", "romantic", "adventurous"] as const;
export const CATEGORIES = ["at-home", "outdoors", "adventure", "food", "entertainment", "low-budget", "special-occasion", "romantic"] as const;

export const LABELS: Record<string, string> = {
  free: "Free",
  low: "Low",
  medium: "Medium",
  high: "Splurge",
  short: "A couple of hours",
  "half-day": "Half a day",
  "full-day": "A whole day",
  indoor: "Indoors",
  outdoor: "Outdoors",
  either: "Either",
  cozy: "Cozy",
  playful: "Playful",
  relaxed: "Relaxed",
  romantic: "Romantic",
  adventurous: "Adventurous",
  "at-home": "At home",
  outdoors: "Outdoors",
  adventure: "Adventure",
  food: "Food & drink",
  entertainment: "Entertainment",
  "low-budget": "Low budget",
  "special-occasion": "Special occasion",
};
export const label = (v: string) => LABELS[v] ?? v;

export interface IdeaFilters {
  /** "" means any */
  budget: string;
  duration: string;
  category: string;
  setting: string;
  mood: string;
}
export const NO_FILTERS: IdeaFilters = { budget: "", duration: "", category: "", setting: "", mood: "" };

/** The curated library, then the couple's own ideas (newest first). */
export function allIdeas(custom: CustomIdea[] = []): DateIdea[] {
  const lib = (library as Omit<DateIdea, "custom">[]).map((i) => ({ ...i, custom: false }));
  return [...custom.map((c) => ({ ...c, custom: true })), ...lib];
}

const rank = (list: readonly string[], v: string) => list.indexOf(v);

/** Budget and time are ceilings ("up to"); indoor/outdoor also accepts ideas that work either way. */
export function filterIdeas(ideas: DateIdea[], f: IdeaFilters): DateIdea[] {
  return ideas.filter(
    (i) =>
      (!f.budget || rank(BUDGETS, i.budget) <= rank(BUDGETS, f.budget)) &&
      (!f.duration || rank(DURATIONS, i.duration) <= rank(DURATIONS, f.duration)) &&
      (!f.category || i.category === f.category) &&
      (!f.setting || f.setting === "either" || i.setting === f.setting || i.setting === "either") &&
      (!f.mood || i.mood === f.mood),
  );
}

/** A random idea, avoiding `exceptId` when there is any other choice. */
export function pickRandom(ideas: DateIdea[], exceptId?: string | null, rng: () => number = Math.random): DateIdea | null {
  if (!ideas.length) return null;
  const pool = ideas.length > 1 ? ideas.filter((i) => i.id !== exceptId) : ideas;
  return pool[Math.min(pool.length - 1, Math.floor(rng() * pool.length))];
}

/** Currencies offered in the editor; Intl formats any code, these are just the picker. */
export const CURRENCIES = ["USD", "EUR", "GBP", "CAD", "AUD", "CHF", "JPY", "LBP"];

function minorDigits(currency: string): number {
  try {
    return new Intl.NumberFormat("en", { style: "currency", currency }).resolvedOptions().maximumFractionDigits ?? 2;
  } catch {
    return 2;
  }
}

export function formatMoney(cents: number, currency: string): string {
  const d = minorDigits(currency);
  try {
    return new Intl.NumberFormat(undefined, { style: "currency", currency }).format(cents / 10 ** d);
  } catch {
    return `${(cents / 10 ** d).toFixed(d)} ${currency}`;
  }
}

/** "12.50" -> 1250 minor units. Blank -> null. Invalid -> undefined. */
export function parseMoney(text: string, currency: string): number | null | undefined {
  const t = text.trim().replace(",", ".");
  if (!t) return null;
  if (!/^\d+(\.\d+)?$/.test(t)) return undefined;
  return Math.round(Number(t) * 10 ** minorDigits(currency));
}

export function moneyToText(cents: number | null, currency: string): string {
  if (cents == null) return "";
  const d = minorDigits(currency);
  return (cents / 10 ** d).toFixed(d);
}

export interface PlanGroups {
  upcoming: Plan[];
  ideas: Plan[];
  earlier: Plan[];
}

/** Upcoming plans soonest first; the shelf of undated ideas; and everything finished, cancelled or past. */
export function groupPlans(plans: Plan[], today: string): PlanGroups {
  const upcoming = plans.filter((p) => p.status === "planned" && p.date && p.date >= today);
  const ideas = plans.filter((p) => p.status === "idea");
  const earlier = plans.filter((p) => !upcoming.includes(p) && !ideas.includes(p));
  const byDate = (a: Plan, b: Plan) => (a.date ?? "").localeCompare(b.date ?? "") || (a.time ?? "").localeCompare(b.time ?? "");
  return { upcoming: [...upcoming].sort(byDate), ideas, earlier: [...earlier].sort((a, b) => byDate(b, a)) };
}

export function nextPlan(plans: Plan[], today: string): Plan | undefined {
  return groupPlans(plans, today).upcoming[0];
}

export function checklistProgress(p: Pick<Plan, "checklist">): string | null {
  if (!p.checklist.length) return null;
  return `${p.checklist.filter((c) => c.done).length} of ${p.checklist.length} ready`;
}
