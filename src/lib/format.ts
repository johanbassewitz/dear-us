function parse(iso: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  return m ? new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])) : null;
}

/** "July 12, 2025" */
export function fmtDate(iso: string): string {
  const d = parse(iso);
  return d ? d.toLocaleDateString(undefined, { month: "long", day: "numeric", year: "numeric" }) : iso;
}

/** "Jul 12" */
export function fmtShort(iso: string): string {
  const d = parse(iso);
  return d ? d.toLocaleDateString(undefined, { month: "short", day: "numeric" }) : iso;
}

/** "July 2025" */
export function fmtMonth(iso: string): string {
  const d = parse(iso);
  return d ? d.toLocaleDateString(undefined, { month: "long", year: "numeric" }) : iso;
}

/** "14:30" → locale time */
export function fmtTime(hhmm: string | null): string | null {
  if (!hhmm) return null;
  const [h, m] = hhmm.split(":").map(Number);
  const d = new Date();
  d.setHours(h, m, 0, 0);
  return d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

export function yearsAgo(entryIso: string, todayIso: string): number {
  return Number(todayIso.slice(0, 4)) - Number(entryIso.slice(0, 4));
}

export function daysUntilPurge(deletedAtIso: string, retentionDays = 30, now = new Date()): number {
  const purge = new Date(deletedAtIso).getTime() + retentionDays * 86_400_000;
  return Math.max(0, Math.ceil((purge - now.getTime()) / 86_400_000));
}
