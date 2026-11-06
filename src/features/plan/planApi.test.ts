import { describe, expect, it, vi } from "vitest";
import { daysLeft, planApi, planNotice, quotaError, waitForPayment, type Plan } from "./planApi";

const MB = 1024 * 1024;
const now = Date.parse("2026-11-05T12:00:00Z");
const base: Plan = {
  tier: "free",
  trialEndsAt: "2026-10-01T00:00:00Z",
  premiumUntil: null,
  period: "2026-11",
  mediaCapBytes: 100 * MB,
  mediaUsedBytes: 0,
  cupidAllowance: 10,
  cupidUsed: 0,
  cupidBonus: 0,
  cupidRemaining: 10,
  billingAvailable: true,
};

describe("plan notices", () => {
  it("stays quiet during most of the trial and nudges in the last days", () => {
    expect(planNotice({ ...base, tier: "trial", mediaCapBytes: null, trialEndsAt: "2026-11-20T00:00:00Z" }, false, now)).toBeNull();
    const n = planNotice({ ...base, tier: "trial", mediaCapBytes: null, trialEndsAt: "2026-11-07T00:00:00Z" }, false, now);
    expect(n?.title).toBe("Your free trial ends in 2 days");
    expect(n?.body).toMatch(/Nothing is lost/);
  });

  it("warns at 80% of the free photo space and explains Wi-Fi when it's full", () => {
    expect(planNotice({ ...base, mediaUsedBytes: 50 * MB }, false, now)).toBeNull();
    expect(planNotice({ ...base, mediaUsedBytes: 85 * MB }, false, now)?.title).toBe("85 MB of 100 MB cloud photo space used this month");
    const full = planNotice({ ...base, mediaUsedBytes: 20 * MB }, true, now);
    expect(full?.title).toMatch(/used up/);
    expect(full?.body).toMatch(/home Wi-Fi/);
  });

  it("never nags Premium couples", () => {
    expect(planNotice({ ...base, tier: "premium", mediaCapBytes: null, mediaUsedBytes: 999 * MB }, true, now)).toBeNull();
    expect(planNotice(null, true, now)).toBeNull();
  });

  it("counts days left, rounding up", () => {
    expect(daysLeft("2026-11-05T13:00:00Z", now)).toBe(1);
    expect(daysLeft("2026-11-01T00:00:00Z", now)).toBe(0);
    expect(daysLeft(null, now)).toBe(0);
  });
});

describe("plan errors", () => {
  it("separates the code from the message", () => {
    expect(quotaError(new Error("[cupid_quota] Cupid is out of arrows."))).toEqual({ code: "cupid_quota", message: "Cupid is out of arrows." });
    expect(quotaError("offline: no network")).toEqual({ code: null, message: "offline: no network" });
  });
});

describe("paying in the browser", () => {
  it("waits until the payment shows up on the server", async () => {
    const get = vi.spyOn(planApi, "get").mockResolvedValueOnce(base).mockResolvedValueOnce({ ...base, tier: "premium" });
    expect(await waitForPayment(base, "premium_monthly", { everyMs: 1, forMs: 1000 })).toBe(true);
    expect(get).toHaveBeenCalledTimes(2);
    get.mockReset();
  });

  it("notices arrows by the extra balance, and gives up quietly after a while", async () => {
    vi.spyOn(planApi, "get").mockResolvedValue({ ...base, cupidBonus: 100 });
    expect(await waitForPayment(base, "arrows_100", { everyMs: 1, forMs: 1000 })).toBe(true);
    vi.spyOn(planApi, "get").mockResolvedValue(base);
    expect(await waitForPayment(base, "arrows_100", { everyMs: 1, forMs: 20 })).toBe(false);
  });
});
