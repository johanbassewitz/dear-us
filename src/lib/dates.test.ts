import { describe, expect, it } from "vitest";
import { daysTogether, localIsoDate, togetherDuration } from "./dates";

const d = (y: number, m: number, day: number) => new Date(y, m - 1, day, 15, 30);

describe("daysTogether", () => {
  it("counts whole local days", () => {
    expect(daysTogether("2026-11-01", d(2026, 11, 3))).toBe(2);
    expect(daysTogether("2026-11-03", d(2026, 11, 3))).toBe(0);
  });
  it("handles leap years and DST spans", () => {
    expect(daysTogether("2024-01-01", d(2025, 1, 1))).toBe(366);
    expect(daysTogether("2026-03-01", d(2026, 4, 1))).toBe(31);
  });
  it("returns null for missing, invalid or future dates", () => {
    expect(daysTogether(null)).toBeNull();
    expect(daysTogether("2026-02-30", d(2026, 11, 3))).toBeNull();
    expect(daysTogether("not a date")).toBeNull();
    expect(daysTogether("2027-01-01", d(2026, 11, 3))).toBeNull();
  });
});

describe("togetherDuration", () => {
  it("formats years and months", () => {
    expect(togetherDuration("2024-07-03", d(2026, 11, 3))).toBe("2 years, 4 months");
    expect(togetherDuration("2025-11-03", d(2026, 11, 3))).toBe("1 year");
    expect(togetherDuration("2026-10-20", d(2026, 11, 3))).toBe("less than a month");
  });
  it("doesn't count a month that hasn't completed", () => {
    expect(togetherDuration("2026-09-15", d(2026, 11, 3))).toBe("1 month");
  });
});

describe("localIsoDate", () => {
  it("uses the local calendar date", () => {
    expect(localIsoDate(new Date(2026, 0, 5, 23, 59))).toBe("2026-01-05");
  });
});
