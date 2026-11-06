import { useState } from "react";
import { X } from "lucide-react";
import { planNotice, usePlan } from "./planApi";
import { useUpgrade } from "./Purchase";
import { useSyncStatus } from "../sync/syncApi";
import { useIsStore } from "../../lib/edition";
import { Button } from "../../components/ui/Button";

const KEY = "dearus.dismissedNotices";

function dismissed(): string[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]");
  } catch {
    return [];
  }
}

/** Store edition: one gentle, dismissible note when the trial ends soon or photo space runs out. */
export function PlanBanner() {
  const store = useIsStore();
  const { data: plan } = usePlan();
  const { data: sync } = useSyncStatus();
  const show = useUpgrade((s) => s.show);
  const [hidden, setHidden] = useState(dismissed);
  const n = store ? planNotice(plan, sync?.state === "waiting_for_space") : null;
  if (!n || hidden.includes(n.id)) return null;
  const tone = n.tone === "gold" ? "bg-gold" : n.tone === "green" ? "bg-sage" : "bg-accent-soft";
  return (
    <div role="status" className={`mb-6 flex items-start gap-3 rounded-2xl ${tone} px-5 py-4`}>
      <div className="min-w-0 flex-1">
        <strong className="block text-[13.5px]">{n.title}</strong>
        <p className="mt-1 text-[12.5px] leading-relaxed text-ink-soft">{n.body}</p>
        {n.cta === "premium" && plan?.billingAvailable && (
          <Button className="mt-3" size="sm" onClick={show}>
            See Premium
          </Button>
        )}
      </div>
      <button
        aria-label="Dismiss"
        className="text-faint hover:text-ink"
        onClick={() => {
          const next = [...hidden, n.id].slice(-30);
          setHidden(next);
          try {
            localStorage.setItem(KEY, JSON.stringify(next));
          } catch {
            /* private mode: dismissed for this session only */
          }
        }}
      >
        <X size={15} />
      </button>
    </div>
  );
}
