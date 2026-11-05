// The question library (src/data/questions.json) plus deterministic picks, so both PCs
// show the same "question of the day" and the same weekly check-in without talking to each other.
import raw from "../data/questions.json";

export type QuestionCategory = "favorite-moments" | "appreciation" | "communication" | "plans" | "goals" | "reflection";

export interface LibraryQuestion {
  id: string;
  text: string;
  category: QuestionCategory;
}

export const QUESTIONS = raw as LibraryQuestion[];

export const CATEGORIES: { id: QuestionCategory; label: string }[] = [
  { id: "favorite-moments", label: "Favorite moments" },
  { id: "appreciation", label: "Appreciation" },
  { id: "communication", label: "Communication" },
  { id: "plans", label: "Plans" },
  { id: "goals", label: "Goals" },
  { id: "reflection", label: "Reflection" },
];

export const categoryLabel = (id: string) => CATEGORIES.find((c) => c.id === id)?.label ?? id;

export function questionById(id: string): LibraryQuestion | undefined {
  return QUESTIONS.find((q) => q.id === id);
}

/** FNV-1a — tiny, stable string hash (same result on both PCs). */
export function hashString(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** Same question on both partners' PCs for a given day (seeded by date + space id). */
export function questionOfTheDay(isoDate: string, spaceId: string | null | undefined): LibraryQuestion {
  return QUESTIONS[hashString(`${isoDate}|${spaceId ?? ""}`) % QUESTIONS.length];
}

function parseIso(iso: string): Date {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d));
}

/** ISO-8601 week of a YYYY-MM-DD date: key like "2026-W45", plus its Monday and Sunday. */
export function isoWeek(isoDate: string): { key: string; start: string; end: string } {
  const d = parseIso(isoDate);
  const dow = (d.getUTCDay() + 6) % 7; // Monday = 0
  const monday = new Date(d);
  monday.setUTCDate(d.getUTCDate() - dow);
  const sunday = new Date(monday);
  sunday.setUTCDate(monday.getUTCDate() + 6);
  const thursday = new Date(monday);
  thursday.setUTCDate(monday.getUTCDate() + 3); // the Thursday decides the ISO year
  const yearStart = Date.UTC(thursday.getUTCFullYear(), 0, 1);
  const week = Math.floor((thursday.getTime() - yearStart) / 86400000 / 7) + 1;
  const f = (x: Date) => x.toISOString().slice(0, 10);
  return { key: `${thursday.getUTCFullYear()}-W${String(week).padStart(2, "0")}`, start: f(monday), end: f(sunday) };
}

const CHECKIN_CATEGORIES: QuestionCategory[] = ["appreciation", "communication", "reflection", "plans", "goals", "favorite-moments"];

/** Three gentle prompts for the week, each from a different category (same on both PCs). */
export function weeklyCheckIn(isoDate: string, spaceId: string | null | undefined): LibraryQuestion[] {
  const { key } = isoWeek(isoDate);
  const first = hashString(`${key}|${spaceId ?? ""}|cats`) % CHECKIN_CATEGORIES.length;
  const step = 1 + (hashString(`${key}|step`) % 2); // 1 or 2: both visit 3 distinct categories of 6
  return [0, 1, 2].map((n) => {
    const cat = CHECKIN_CATEGORIES[(first + n * step) % CHECKIN_CATEGORIES.length];
    const pool = QUESTIONS.filter((q) => q.category === cat);
    return pool[hashString(`${key}|${spaceId ?? ""}|${cat}`) % pool.length];
  });
}
