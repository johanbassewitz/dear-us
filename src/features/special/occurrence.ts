// Pure date logic for special dates. All dates are local calendar days (YYYY-MM-DD); no clocks are read here.

export type Recurrence = "none" | "yearly" | "monthly";

export interface DateRule {
  date: string;
  recurrence: Recurrence;
}

export interface Ymd {
  y: number;
  m: number; // 1-12
  d: number;
}

export function parseYmd(iso: string): Ymd | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return null;
  const v = { y: +m[1], m: +m[2], d: +m[3] };
  return v.m >= 1 && v.m <= 12 && v.d >= 1 && v.d <= daysInMonth(v.y, v.m) ? v : null;
}

export const toIso = (v: Ymd) => `${String(v.y).padStart(4, "0")}-${String(v.m).padStart(2, "0")}-${String(v.d).padStart(2, "0")}`;

export const isLeap = (y: number) => (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;

export function daysInMonth(y: number, m: number): number {
  return [31, isLeap(y) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];
}

/** Whole days from a to b (b later = positive). Uses UTC day numbers so DST never shifts the answer. */
export function diffDays(a: string, b: string): number {
  const pa = parseYmd(a)!;
  const pb = parseYmd(b)!;
  return Math.round((Date.UTC(pb.y, pb.m - 1, pb.d) - Date.UTC(pa.y, pa.m - 1, pa.d)) / 86_400_000);
}

export function addDays(iso: string, n: number): string {
  const p = parseYmd(iso)!;
  const t = new Date(Date.UTC(p.y, p.m - 1, p.d + n));
  return toIso({ y: t.getUTCFullYear(), m: t.getUTCMonth() + 1, d: t.getUTCDate() });
}

/** Occurrence in a given year/month, clamping day-of-month (Feb 29 -> Feb 28, 31st -> last day). */
function clamp(y: number, m: number, day: number): string {
  return toIso({ y, m, d: Math.min(day, daysInMonth(y, m)) });
}

export function ordinal(n: number): string {
  const mod100 = n % 100;
  if (mod100 >= 11 && mod100 <= 13) return `${n}th`;
  return `${n}${({ 1: "st", 2: "nd", 3: "rd" } as Record<number, string>)[n % 10] ?? "th"}`;
}

export interface Occurrence {
  /** The occurrence date (YYYY-MM-DD). */
  date: string;
  /** Days from today; 0 = today. */
  days: number;
  /** Years (yearly) or months (monthly) since the first date. 0 = the original date. */
  count: number;
}

/**
 * The next occurrence on or after today. Null for a one-off date that is already past.
 * A recurring date whose first date is still in the future starts at that first date.
 */
export function nextOccurrence(rule: DateRule, today: string): Occurrence | null {
  const s = parseYmd(rule.date);
  const t = parseYmd(today);
  if (!s || !t) return null;
  let date: string;
  let count: number;
  if (rule.recurrence === "none" || rule.date >= today) {
    if (rule.date < today) return null;
    date = rule.date;
    count = 0;
  } else if (rule.recurrence === "yearly") {
    let y = t.y;
    date = clamp(y, s.m, s.d);
    if (date < today) {
      y += 1;
      date = clamp(y, s.m, s.d);
    }
    count = y - s.y;
  } else {
    let y = t.y;
    let m = t.m;
    date = clamp(y, m, s.d);
    if (date < today) {
      m += 1;
      if (m > 12) {
        m = 1;
        y += 1;
      }
      date = clamp(y, m, s.d);
    }
    count = (y - s.y) * 12 + (m - s.m);
  }
  return { date, days: diffDays(today, date), count };
}

/** Wording for "which one": "3rd year", "5th month", or null for the first/one-off date. */
export function repeatLabel(recurrence: Recurrence, count: number): string | null {
  if (count <= 0 || recurrence === "none") return null;
  return recurrence === "yearly" ? `${ordinal(count)} year` : `${ordinal(count)} month`;
}

/** "3 years, 2 months" / "12 days" for a past date. */
export function timeSince(iso: string, today: string): string {
  const a = parseYmd(iso)!;
  const b = parseYmd(today)!;
  if (iso >= today) return "today";
  let months = (b.y - a.y) * 12 + (b.m - a.m);
  if (b.d < a.d) months -= 1;
  const y = Math.floor(months / 12);
  const m = months % 12;
  if (y === 0 && m === 0) {
    const d = diffDays(iso, today);
    return `${d} day${d === 1 ? "" : "s"}`;
  }
  const parts: string[] = [];
  if (y) parts.push(`${y} year${y === 1 ? "" : "s"}`);
  if (m) parts.push(`${m} month${m === 1 ? "" : "s"}`);
  return parts.join(", ");
}

export function countdownLabel(days: number): string {
  if (days === 0) return "Today";
  if (days === 1) return "Tomorrow";
  return `${days} days`;
}

export interface SpecialLike extends DateRule {
  id: string;
  title: string;
  time: string | null;
  remindDaysBefore: number | null;
}

export interface Occasion {
  /** Stable key for notifications: id + occurrence date. */
  key: string;
  /** Real special date id, or null for the computed anniversary. */
  id: string | null;
  title: string;
  time: string | null;
  recurrence: Recurrence;
  date: string;
  days: number;
  count: number;
  remindDaysBefore: number | null;
  computed: boolean;
}

export const ANNIVERSARY_TITLE = "Our anniversary";

/** Computed (never stored) yearly item for space.startDate, unless a special date already sits on that month-day. */
export function anniversaryRule(startDate: string | null | undefined, specials: DateRule[]): DateRule | null {
  const s = startDate ? parseYmd(startDate) : null;
  if (!s) return null;
  const same = specials.some((x) => {
    const p = parseYmd(x.date);
    return p && p.m === s.m && p.d === s.d;
  });
  return same ? null : { date: startDate!, recurrence: "yearly" };
}

/** Upcoming occasions, nearest first. */
export function upcomingOccasions(specials: SpecialLike[], startDate: string | null | undefined, today: string): Occasion[] {
  const out: Occasion[] = [];
  for (const s of specials) {
    const o = nextOccurrence(s, today);
    if (!o) continue;
    out.push({ key: `${s.id}:${o.date}`, id: s.id, title: s.title, time: s.time, recurrence: s.recurrence, date: o.date, days: o.days, count: o.count, remindDaysBefore: s.remindDaysBefore, computed: false });
  }
  const a = anniversaryRule(startDate, specials);
  if (a) {
    const o = nextOccurrence(a, today);
    if (o) out.push({ key: `anniversary:${o.date}`, id: null, title: ANNIVERSARY_TITLE, time: null, recurrence: "yearly", date: o.date, days: o.days, count: o.count, remindDaysBefore: null, computed: true });
  }
  return out.sort((x, y) => x.date.localeCompare(y.date) || (x.time ?? "").localeCompare(y.time ?? "") || x.title.localeCompare(y.title));
}

/** One-off dates already in the past, most recent first. */
export function pastMilestones<T extends SpecialLike>(specials: T[], today: string): T[] {
  return specials.filter((s) => s.recurrence === "none" && s.date < today).sort((a, b) => b.date.localeCompare(a.date));
}

/** Occasions whose reminder day is today (days until == remindDaysBefore). */
export function dueReminders(occasions: Occasion[]): Occasion[] {
  return occasions.filter((o) => o.remindDaysBefore !== null && o.days === o.remindDaysBefore);
}

/** Titles of everything that falls on each day of a month. Keys are day-of-month. */
export function monthMarks(specials: SpecialLike[], startDate: string | null | undefined, year: number, month: number): Map<number, string[]> {
  const marks = new Map<number, string[]>();
  const add = (day: number, title: string) => marks.set(day, [...(marks.get(day) ?? []), title]);
  const visit = (rule: DateRule, title: string) => {
    const s = parseYmd(rule.date);
    if (!s) return;
    const first = toIso({ y: year, m: month, d: 1 });
    const last = toIso({ y: year, m: month, d: daysInMonth(year, month) });
    if (rule.recurrence === "none") {
      if (rule.date >= first && rule.date <= last) add(s.d, title);
      return;
    }
    if (rule.recurrence === "yearly" && s.m !== month) return;
    const iso = clamp(year, month, s.d);
    if (iso >= rule.date) add(+iso.slice(8), title);
  };
  for (const s of specials) visit(s, s.title);
  const a = anniversaryRule(startDate, specials);
  if (a) visit(a, ANNIVERSARY_TITLE);
  return marks;
}
