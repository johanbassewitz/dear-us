/** Local calendar date as YYYY-MM-DD (not UTC — "today" means today for the user). */
export function localIsoDate(d: Date = new Date()): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function parseIsoDate(iso: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return d.getMonth() === Number(m[2]) - 1 ? d : null;
}

/** Whole days from start date to today. Null if no/invalid date or date is in the future. */
export function daysTogether(startIso: string | null | undefined, today: Date = new Date()): number | null {
  if (!startIso) return null;
  const start = parseIsoDate(startIso);
  if (!start) return null;
  const t = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  // Round to absorb DST hour shifts.
  const days = Math.round((t.getTime() - start.getTime()) / 86_400_000);
  return days >= 0 ? days : null;
}

/** "2 years, 4 months" style duration, or null. */
export function togetherDuration(startIso: string | null | undefined, today: Date = new Date()): string | null {
  const start = startIso ? parseIsoDate(startIso) : null;
  if (!start || daysTogether(startIso, today) === null) return null;
  let months = (today.getFullYear() - start.getFullYear()) * 12 + (today.getMonth() - start.getMonth());
  if (today.getDate() < start.getDate()) months -= 1;
  const y = Math.floor(months / 12);
  const m = months % 12;
  const parts: string[] = [];
  if (y) parts.push(`${y} year${y === 1 ? "" : "s"}`);
  if (m) parts.push(`${m} month${m === 1 ? "" : "s"}`);
  return parts.length ? parts.join(", ") : "less than a month";
}

export function longDate(d: Date = new Date()): string {
  return d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
}

export function greetingFor(d: Date = new Date()): string {
  const h = d.getHours();
  if (h < 5) return "still up";
  if (h < 12) return "good morning";
  if (h < 18) return "good afternoon";
  return "good evening";
}
