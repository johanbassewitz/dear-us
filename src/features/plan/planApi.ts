import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { call, isDesktop } from "../../lib/api";
import { queryClient } from "../../lib/queries";
import { useIsStore } from "../../lib/edition";

/** The couple's plan, decided by the Dear Us cloud (Store edition). */
export type Tier = "solo" | "trial" | "premium" | "free";
export interface Plan {
  tier: Tier;
  trialEndsAt: string | null;
  premiumUntil: string | null;
  period: string;
  /** null = unlimited */
  mediaCapBytes: number | null;
  mediaUsedBytes: number;
  cupidAllowance: number;
  cupidUsed: number;
  cupidBonus: number;
  cupidRemaining: number;
  /** Time capsules that can be sealed and waiting at once (null = unlimited). */
  capsuleLimit?: number | null;
  billingAvailable: boolean;
  /** "paddle": checkout in the browser. "microsoft": the Store's purchase dialog. */
  billingProvider?: "paddle" | "microsoft" | null;
}

export type Product = "premium_monthly" | "premium_yearly" | "arrows_100" | "arrows_300";
export interface PurchaseResult {
  status: "purchased" | "already_owned" | "cancelled" | "failed" | "checkout_opened";
  message: string | null;
  plan: Plan | null;
}

/** Shown next to the buttons; the Store's own dialog shows the exact price in the buyer's currency. */
export const PRODUCTS: Record<Product, { title: string; price: string; note?: string }> = {
  premium_monthly: { title: "Premium, monthly", price: "$3.99 / month" },
  premium_yearly: { title: "Premium, yearly", price: "$29.99 / year", note: "save 37%" },
  arrows_100: { title: "100 Cupid arrows", price: "$1.99" },
  arrows_300: { title: "300 Cupid arrows", price: "$3.99" },
};

export const planApi = {
  get: (refresh: boolean) => call<Plan | null>("plan_get", { refresh }),
  purchase: (product: Product) => call<PurchaseResult>("billing_purchase", { product }),
  restore: () => call<Plan | null>("billing_restore"),
  manage: () => call<void>("billing_manage"),
};

export const planKeys = { plan: ["plan"] as const };

export function usePlan() {
  const store = useIsStore();
  return useQuery({ queryKey: planKeys.plan, queryFn: () => planApi.get(true), enabled: isDesktop && store, staleTime: 60_000, refetchInterval: 10 * 60_000 });
}

/** Cupid used an arrow, a purchase went through, …: refresh what's on screen. */
export function usePlanEvents() {
  useEffect(() => {
    if (!isDesktop) return;
    let off: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen("plan-changed", () => {
        queryClient.invalidateQueries({ queryKey: planKeys.plan });
        queryClient.invalidateQueries({ queryKey: ["cupid", "status"] });
      }).then((u) => (cancelled ? u() : (off = u))),
    );
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);
}

const DAY = 24 * 60 * 60 * 1000;

/** Whole days left until `iso` (rounded up), 0 when past. */
export function daysLeft(iso: string | null, now = Date.now()): number {
  if (!iso) return 0;
  return Math.max(0, Math.ceil((new Date(iso).getTime() - now) / DAY));
}

export const mb = (bytes: number) => `${Math.round(bytes / 1024 / 1024)} MB`;

/**
 * Plan limits come back from the app as "[code] message" (e.g. "[cupid_quota] Cupid is out of
 * arrows…"). Returns the code and the message without it.
 */
export function quotaError(e: unknown): { code: string | null; message: string } {
  const raw = String(e).replace(/^Error:\s*/, "");
  const m = raw.match(/^\[([a-z_]+)\]\s*(.*)$/s);
  return m ? { code: m[1], message: m[2] } : { code: null, message: raw };
}

export interface PlanNotice {
  id: string;
  tone: "gold" | "accent" | "green";
  title: string;
  body: string;
  cta: "premium" | null;
}

/**
 * The one gentle nudge worth showing right now (or null). Never alarming: memories are never
 * deleted or locked, whatever the plan.
 */
export function planNotice(plan: Plan | null | undefined, waitingForSpace: boolean, now = Date.now()): PlanNotice | null {
  if (!plan) return null;
  if (plan.tier === "trial") {
    const d = daysLeft(plan.trialEndsAt, now);
    if (d <= 3)
      return {
        id: `trial-ending-${plan.trialEndsAt}`,
        tone: "gold",
        title: d <= 1 ? "Your free trial ends today" : `Your free trial ends in ${d} days`,
        body: "Nothing is lost afterwards: every memory stays on both PCs and keeps syncing. Only cloud photos & videos (100 MB a month) and Cupid (10 arrows a month) get smaller. Premium keeps everything unlimited for both of you.",
        cta: "premium",
      };
    return null;
  }
  if (plan.tier === "free" || plan.tier === "solo") {
    const cap = plan.mediaCapBytes ?? 0;
    if (waitingForSpace || (cap > 0 && plan.mediaUsedBytes >= cap))
      return {
        id: `media-full-${plan.period}`,
        tone: "accent",
        title: "This month's free cloud space for photos is used up",
        body: "Your memories and words keep syncing as usual. New photos and videos travel over your home Wi-Fi when you're both home, or through the cloud again next month. Premium sends them anywhere, right away.",
        cta: "premium",
      };
    if (cap > 0 && plan.mediaUsedBytes >= cap * 0.8)
      return {
        id: `media-80-${plan.period}`,
        tone: "gold",
        title: `${mb(plan.mediaUsedBytes)} of ${mb(cap)} cloud photo space used this month`,
        body: "When it's full, photos and videos simply wait for your home Wi-Fi or next month. Nothing is ever lost.",
        cta: "premium",
      };
  }
  return null;
}

/**
 * After a checkout opened in the browser: watches the plan until the payment shows up
 * (Paddle tells our server within seconds). Resolves true when it did.
 */
export async function waitForPayment(before: Plan | null, product: Product, opts = { everyMs: 4000, forMs: 10 * 60_000 }): Promise<boolean> {
  const done = (p: Plan | null) =>
    !!p && (product.startsWith("arrows") ? p.cupidBonus > (before?.cupidBonus ?? 0) : p.tier === "premium");
  const until = Date.now() + opts.forMs;
  while (Date.now() < until) {
    await new Promise((r) => setTimeout(r, opts.everyMs));
    try {
      const p = await planApi.get(true);
      if (done(p)) {
        queryClient.setQueryData(planKeys.plan, p);
        queryClient.invalidateQueries({ queryKey: ["cupid", "status"] });
        return true;
      }
    } catch {
      /* offline for a moment: keep waiting */
    }
  }
  return false;
}
