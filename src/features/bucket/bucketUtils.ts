import type { BucketItem, BucketPriority, BucketStatus } from "../../lib/api";

export const COLUMNS: { status: BucketStatus; title: string; hint: string; tone: string }[] = [
  { status: "want", title: "Dreaming of", hint: "Ideas you'd love to try someday", tone: "bg-blush" },
  { status: "planned", title: "Making plans", hint: "Dreams with a date or a plan", tone: "bg-lilac" },
  { status: "done", title: "Done together", hint: "Crossed off, with love", tone: "bg-sage" },
];

export const CATEGORY_SUGGESTIONS = ["Travel", "Food", "Adventure", "At home", "Learning"];

const PRIORITY_RANK: Record<BucketPriority, number> = { high: 0, medium: 1, low: 2 };

/** High priority first, then the nearest target date, then newest. Done items: most recent first. */
export function sortColumn(items: BucketItem[], status: BucketStatus): BucketItem[] {
  const list = [...items];
  if (status === "done") return list.sort((a, b) => (b.completedAt ?? "").localeCompare(a.completedAt ?? ""));
  return list.sort(
    (a, b) =>
      PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] ||
      (a.targetDate ?? "9999").localeCompare(b.targetDate ?? "9999") ||
      b.createdAt.localeCompare(a.createdAt),
  );
}

export interface BucketFilters {
  text: string;
  status: BucketStatus | null;
  category: string | null;
}

export function filterItems(items: BucketItem[], f: BucketFilters): BucketItem[] {
  const q = f.text.trim().toLowerCase();
  return items.filter(
    (i) =>
      (!f.status || i.status === f.status) &&
      (!f.category || i.category.toLowerCase() === f.category.toLowerCase()) &&
      (!q || `${i.title} ${i.description} ${i.category}`.toLowerCase().includes(q)),
  );
}

export function categoriesOf(items: BucketItem[]): string[] {
  const seen = new Map<string, string>();
  for (const i of items) if (i.category.trim() && !seen.has(i.category.toLowerCase())) seen.set(i.category.toLowerCase(), i.category);
  return [...seen.values()].sort((a, b) => a.localeCompare(b));
}

export function progressLabel(done: number, total: number): string {
  return total === 0 ? "Nothing here yet" : `${done} of ${total} done together`;
}
