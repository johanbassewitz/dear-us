import { describe, expect, it } from "vitest";
import { CATEGORIES, isoWeek, QUESTIONS, questionById, questionOfTheDay, weeklyCheckIn } from "./questions";

describe("question library", () => {
  it("is a full library with unique ids across the known categories", () => {
    expect(QUESTIONS.length).toBeGreaterThanOrEqual(100);
    expect(new Set(QUESTIONS.map((q) => q.id)).size).toBe(QUESTIONS.length);
    const cats = CATEGORIES.map((c) => c.id) as string[];
    expect(QUESTIONS.every((q) => cats.includes(q.category))).toBe(true);
  });
});

describe("questionOfTheDay", () => {
  it("is identical for both partners on the same day (same space)", () => {
    expect(questionOfTheDay("2026-11-03", "space-1")).toEqual(questionOfTheDay("2026-11-03", "space-1"));
  });
  it("always returns a library question", () => {
    for (let i = 1; i <= 28; i++) {
      const q = questionOfTheDay(`2026-02-${String(i).padStart(2, "0")}`, "s");
      expect(questionById(q.id)).toBe(q);
    }
  });
  it("varies across days", () => {
    const seen = new Set(Array.from({ length: 30 }, (_, i) => questionOfTheDay(`2026-06-${String(i + 1).padStart(2, "0")}`, "s").id));
    expect(seen.size).toBeGreaterThan(15);
  });
});

describe("weekly check-in", () => {
  it("computes ISO weeks, including year boundaries", () => {
    expect(isoWeek("2026-11-04")).toEqual({ key: "2026-W45", start: "2026-11-02", end: "2026-11-08" });
    expect(isoWeek("2027-01-01").key).toBe("2026-W53");
    expect(isoWeek("2024-12-30").key).toBe("2025-W01");
  });
  it("is the same all week, and picks three prompts from three different categories", () => {
    const mon = weeklyCheckIn("2026-11-02", "s");
    expect(weeklyCheckIn("2026-11-08", "s")).toEqual(mon);
    expect(mon).toHaveLength(3);
    expect(new Set(mon.map((q) => q.category)).size).toBe(3);
    for (let w = 0; w < 52; w++) {
      const d = new Date(Date.UTC(2026, 0, 1 + w * 7)).toISOString().slice(0, 10);
      expect(new Set(weeklyCheckIn(d, "s").map((q) => q.category)).size).toBe(3);
    }
  });
});
