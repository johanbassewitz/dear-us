import { useState } from "react";
import { Feather, Heart, Loader, Sparkles } from "lucide-react";
import { create } from "zustand";
import { PRODUCTS, planApi, planKeys, quotaError, waitForPayment, type Plan, type Product } from "./planApi";
import { queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { Modal } from "../../components/ui/Modal";
import { toast, toastError } from "../../components/ui/Toast";

/** Starts a purchase: Paddle checkout in the browser (or the Store dialog if that's configured). */
export async function buy(product: Product): Promise<boolean> {
  try {
    const before = queryClient.getQueryData<Plan | null>(planKeys.plan) ?? null;
    const r = await planApi.purchase(product);
    if (r.status === "checkout_opened") {
      toast("Checkout opened in your browser. Come back here when you're done ♡");
      const paid = await waitForPayment(r.plan ?? before, product);
      if (paid) toast(product.startsWith("arrows") ? "Cupid's quiver is full again ♡" : "Welcome to Dear Us Premium ♡ for both of you");
      return paid;
    }
    if (r.plan) queryClient.setQueryData(planKeys.plan, r.plan);
    queryClient.invalidateQueries({ queryKey: ["cupid", "status"] });
    if (r.status === "purchased" || r.status === "already_owned") {
      toast(product.startsWith("arrows") ? "Cupid's quiver is full again ♡" : "Welcome to Dear Us Premium ♡ for both of you");
      return true;
    }
    if (r.status === "failed") toastError(r.message ?? "The purchase didn't go through. Nothing was charged.");
    return false;
  } catch (e) {
    toastError(quotaError(e).message);
    return false;
  }
}

export function BuyButton({ product, variant = "primary", size = "sm", onDone }: { product: Product; variant?: "primary" | "secondary"; size?: "sm" | "md"; onDone?: () => void }) {
  const [busy, setBusy] = useState(false);
  const p = PRODUCTS[product];
  return (
    <Button
      variant={variant}
      size={size}
      disabled={busy}
      icon={busy ? <Loader size={13} className="animate-spin" /> : undefined}
      onClick={async () => {
        setBusy(true);
        const ok = await buy(product);
        setBusy(false);
        if (ok) onDone?.();
      }}
    >
      {p.title} · {p.price}
      {p.note ? ` (${p.note})` : ""}
    </Button>
  );
}

/** One place that can open the upgrade dialog from anywhere (banner, Cupid, settings). */
export const useUpgrade = create<{ open: boolean; show: () => void; hide: () => void }>((set) => ({
  open: false,
  show: () => set({ open: true }),
  hide: () => set({ open: false }),
}));

const Row = ({ icon, children }: { icon: React.ReactNode; children: React.ReactNode }) => (
  <li className="flex items-start gap-2.5 text-[12.5px] leading-relaxed text-ink-soft">
    <span className="mt-0.5 text-accent">{icon}</span>
    <span>{children}</span>
  </li>
);

export function UpgradeDialog() {
  const { open, hide } = useUpgrade();
  return (
    <Modal open={open} onClose={hide} title="Dear Us Premium" description="One subscription covers both of you, on all your devices.">
      <ul className="flex flex-col gap-2.5">
        <Row icon={<Sparkles size={15} />}>Unlimited cloud sync for photos and videos, anywhere in the world.</Row>
        <Row icon={<Feather size={15} />}>100 Cupid arrows every month for chats and date ideas.</Row>
        <Row icon={<Heart size={15} />}>Without it, nothing is ever lost or locked: your memories keep syncing, and photos travel over your home Wi-Fi.</Row>
      </ul>
      <div className="mt-5 flex flex-col gap-2">
        <BuyButton product="premium_yearly" size="md" onDone={hide} />
        <BuyButton product="premium_monthly" variant="secondary" size="md" onDone={hide} />
      </div>
      <p className="mt-3 text-[11px] text-faint">
        Secure checkout in your browser by Paddle, our reseller, which shows the price in your currency with taxes included. Cancel anytime from Settings → Your plan → Manage
        subscription.
      </p>
    </Modal>
  );
}
