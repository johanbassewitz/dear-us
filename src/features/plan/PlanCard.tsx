import { useState } from "react";
import { Feather, Loader, Lock, RotateCcw } from "lucide-react";
import { daysLeft, mb, planApi, planKeys, quotaError, usePlan, type Plan } from "./planApi";
import { BuyButton, useUpgrade } from "./Purchase";
import { queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { Pill } from "../../components/ui/Pill";
import { toast, toastError } from "../../components/ui/Toast";

function describePlan(p: Plan): { pill: string; tone: "green" | "gold" | "accent" | "gray"; sentence: string } {
  switch (p.tier) {
    case "trial":
      return { pill: "Free trial", tone: "green", sentence: `Everything unlocked for both of you, ${daysLeft(p.trialEndsAt)} more day${daysLeft(p.trialEndsAt) === 1 ? "" : "s"}.` };
    case "premium":
      return { pill: "Premium", tone: "accent", sentence: "Unlimited cloud sync and 100 Cupid arrows a month, for both of you. Thank you ♡" };
    case "free":
      return { pill: "Free", tone: "gray", sentence: "Your memories keep syncing. Photos & videos: 100 MB a month through the cloud, unlimited over your home Wi-Fi." };
    default:
      return { pill: "Not paired yet", tone: "gold", sentence: "Your 30-day free trial starts when your partner joins your space." };
  }
}

export function PlanCard() {
  const { data: plan, isLoading } = usePlan();
  const show = useUpgrade((s) => s.show);
  const [restoring, setRestoring] = useState(false);
  if (isLoading) return null;
  if (!plan) return <p className="text-[12.5px] text-muted">Set up partner sync and your plan shows up here. Your 30-day free trial starts when your partner joins.</p>;
  const d = describePlan(plan);
  const cap = plan.mediaCapBytes;
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Pill tone={d.tone}>{d.pill}</Pill>
        <span className="text-[12.5px] text-muted">{d.sentence}</span>
      </div>
      <div>
        <div className="mb-1.5 flex justify-between text-[11px] text-muted">
          <span>Cloud photos & videos this month</span>
          <span>{cap ? `${mb(plan.mediaUsedBytes)} of ${mb(cap)}` : `${mb(plan.mediaUsedBytes)} · unlimited`}</span>
        </div>
        <div className="h-2 overflow-hidden rounded-full bg-paper-2">
          <div className="h-full rounded-full bg-accent" style={{ width: cap ? `${Math.min(100, (plan.mediaUsedBytes / cap) * 100)}%` : "100%", opacity: cap ? 1 : 0.25 }} />
        </div>
      </div>
      <div className="flex items-center gap-2 text-[12.5px]">
        <Feather size={15} className="text-accent" />
        <span>
          <b>{plan.cupidRemaining}</b> Cupid arrow{plan.cupidRemaining === 1 ? "" : "s"} left
          <span className="text-muted">
            {" "}
            ({Math.max(0, plan.cupidAllowance - plan.cupidUsed)} of {plan.cupidAllowance} this month{plan.cupidBonus ? ` + ${plan.cupidBonus} extra` : ""})
          </span>
        </span>
      </div>
      <div className="flex items-center gap-2 text-[12.5px]">
        <Lock size={15} className="text-accent" />
        <span>
          Time capsules: {plan.capsuleLimit ? <>up to <b>{plan.capsuleLimit}</b> sealed at once</> : <b>unlimited</b>}
          <span className="text-muted"> (writing and opening them is always free)</span>
        </span>
      </div>
      {plan.billingAvailable && (
        <div className="flex flex-wrap gap-2 border-t border-line-soft pt-4">
          {plan.tier !== "premium" && (
            <Button size="sm" onClick={show}>
              Get Premium
            </Button>
          )}
          <BuyButton product="arrows_100" variant="secondary" />
          {plan.billingProvider === "paddle" ? (
            plan.tier === "premium" && (
              <Button
                size="sm"
                variant="ghost"
                onClick={async () => {
                  try {
                    await planApi.manage();
                  } catch (e) {
                    toastError(quotaError(e).message);
                  }
                }}
              >
                Manage subscription
              </Button>
            )
          ) : (
            <Button
              size="sm"
              variant="ghost"
              disabled={restoring}
              icon={restoring ? <Loader size={13} className="animate-spin" /> : <RotateCcw size={13} />}
              onClick={async () => {
                setRestoring(true);
                try {
                  const p = await planApi.restore();
                  if (p) queryClient.setQueryData(planKeys.plan, p);
                  toast("Purchases checked ♡");
                } catch (e) {
                  toastError(quotaError(e).message);
                } finally {
                  setRestoring(false);
                }
              }}
            >
              Restore purchases
            </Button>
          )}
        </div>
      )}
      <p className="text-[11px] text-faint">Whatever the plan, your memories are never deleted or locked. They live on your PCs.</p>
    </div>
  );
}
