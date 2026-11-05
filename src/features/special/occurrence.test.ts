import { describe, expect, it } from "vitest";
import {
  addDays, diffDays, dueReminders, monthMarks, nextOccurrence, ordinal, pastMilestones, repeatLabel, timeSince, upcomingOccasions,
  type SpecialLike,
} from "./occurrence";

const sp = (o: Partial<SpecialLike> & { id: string; date: string }): SpecialLike => ({
  title: o.id, time: null, recurrence: "yearly", remindDaysBefore: null, ...o,
});

describe("nextOccurrence", () => {
  it("yearly: this year if still ahead, else next year, with the anniversary count", () => {
    expect(nextOccurrence({ date: "2022-07-03", recurrence: "yearly" }, "2025-06-21")).toEqual({ date: "2025-07-03", days: 12, count: 3 });
    expect(nextOccurrence({ date: "2022-07-03", recurrence: "yearly" }, "2025-07-03")).toEqual({ date: "2025-07-03", days: 0, count: 3 });
    expect(nextOccurrence({ date: "2022-07-03", recurrence: "yearly" }, "2025-07-04")).toEqual({ date: "2026-07-03", days: 364, count: 4 });
  });
  it("yearly Feb 29 falls back to Feb 28 in non-leap years", () => {
    expect(nextOccurrence({ date: "2024-02-29", recurrence: "yearly" }, "2025-01-10")?.date).toBe("2025-02-28");
    expect(nextOccurrence({ date: "2024-02-29", recurrence: "yearly" }, "2027-03-01")?.date).toBe("2028-02-29");
  });
  it("monthly on the 31st falls back to the last day of shorter months", () => {
    const r = { date: "2025-01-31", recurrence: "monthly" as const };
    expect(nextOccurrence(r, "2025-02-01")?.date).toBe("2025-02-28");
    expect(nextOccurrence(r, "2025-04-05")?.date).toBe("2025-04-30");
    expect(nextOccurrence(r, "2025-12-31")).toEqual({ date: "2025-12-31", days: 0, count: 11 });
    expect(nextOccurrence(r, "2026-01-01")?.date).toBe("2026-01-31");
  });
  it("monthly rolls over the year end", () => {
    expect(nextOccurrence({ date: "2025-03-10", recurrence: "monthly" }, "2025-12-20")).toEqual({ date: "2026-01-10", days: 21, count: 10 });
  });
  it("a recurring date that starts in the future begins at its first date", () => {
    expect(nextOccurrence({ date: "2030-05-01", recurrence: "yearly" }, "2025-01-01")).toMatchObject({ date: "2030-05-01", count: 0 });
  });
  it("one-off dates: upcoming or null once past", () => {
    expect(nextOccurrence({ date: "2025-08-01", recurrence: "none" }, "2025-07-30")).toEqual({ date: "2025-08-01", days: 2, count: 0 });
    expect(nextOccurrence({ date: "2025-07-01", recurrence: "none" }, "2025-07-30")).toBeNull();
  });
});

describe("helpers", () => {
  it("ordinals and labels", () => {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 103].map(ordinal)).toEqual(["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "103rd"]);
    expect(repeatLabel("yearly", 3)).toBe("3rd year");
    expect(repeatLabel("monthly", 5)).toBe("5th month");
    expect(repeatLabel("yearly", 0)).toBeNull();
    expect(repeatLabel("none", 2)).toBeNull();
  });
  it("time since", () => {
    expect(timeSince("2023-05-14", "2025-07-20")).toBe("2 years, 2 months");
    expect(timeSince("2024-07-03", "2025-07-02")).toBe("11 months");
    expect(timeSince("2025-07-01", "2025-07-02")).toBe("1 day");
    expect(timeSince("2025-06-20", "2025-07-02")).toBe("12 days");
    expect(timeSince("2024-07-03", "2025-07-03")).toBe("1 year");
  });
  it("day arithmetic is DST-safe", () => {
    expect(diffDays("2025-03-29", "2025-04-02")).toBe(4);
    expect(addDays("2025-12-30", 3)).toBe("2026-01-02");
  });
});

describe("occasions", () => {
  it("adds a computed anniversary from the start date, unless a special date is on that month-day", () => {
    const list = upcomingOccasions([sp({ id: "b", date: "2000-08-01", title: "Birthday" })], "2024-07-03", "2025-06-21");
    expect(list.map((o) => [o.title, o.days, o.computed])).toEqual([["Our anniversary", 12, true], ["Birthday", 41, false]]);
    expect(list[0].count).toBe(1);
    const dup = upcomingOccasions([sp({ id: "a", date: "2024-07-03", title: "Our day" })], "2024-07-03", "2025-06-21");
    expect(dup.map((o) => o.title)).toEqual(["Our day"]);
    expect(upcomingOccasions([], null, "2025-06-21")).toEqual([]);
  });
  it("splits past one-offs and finds due reminders", () => {
    const items = [sp({ id: "p", date: "2025-01-01", recurrence: "none" }), sp({ id: "q", date: "2024-03-01", recurrence: "none" }), sp({ id: "t", date: "2025-06-28", recurrence: "none", remindDaysBefore: 7 })];
    expect(pastMilestones(items, "2025-06-21").map((s) => s.id)).toEqual(["p", "q"]);
    const occ = upcomingOccasions(items, null, "2025-06-21");
    expect(dueReminders(occ).map((o) => o.key)).toEqual(["t:2025-06-28"]);
    expect(dueReminders(upcomingOccasions(items, null, "2025-06-22"))).toEqual([]);
  });
  it("marks a month", () => {
    const marks = monthMarks([sp({ id: "m", date: "2025-01-31", recurrence: "monthly" }), sp({ id: "o", date: "2025-02-10", recurrence: "none" })], null, 2025, 2);
    expect([...marks.keys()].sort((a, b) => a - b)).toEqual([10, 28]);
    expect(monthMarks([sp({ id: "y", date: "2020-03-05" })], null, 2025, 2).size).toBe(0);
    expect(monthMarks([sp({ id: "y", date: "2030-03-05" })], null, 2025, 3).size).toBe(0);
  });
});
