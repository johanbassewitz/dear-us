import { useState } from "react";
import { KeyRound, Loader, Mail } from "lucide-react";
import { syncApi } from "./syncApi";
import { queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { Modal } from "../../components/ui/Modal";
import { toast } from "../../components/ui/Toast";

const clean = (e: unknown) => String(e).replace(/^Error:\s*/, "");

/** Store edition: a 6-digit code by email, then a new password. Memories aren't affected. */
export function ForgotPassword({ initialEmail = "", onDone }: { initialEmail?: string; onDone?: () => void }) {
  const [open, setOpen] = useState(false);
  const [email, setEmail] = useState(initialEmail);
  const [sent, setSent] = useState(false);
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    setOpen(false);
    setSent(false);
    setCode("");
    setPassword("");
    setError(null);
  };
  const send = async () => {
    setBusy(true);
    setError(null);
    try {
      await syncApi.forgotPassword(email);
      setSent(true);
    } catch (e) {
      setError(clean(e));
    } finally {
      setBusy(false);
    }
  };
  const reset = async () => {
    setBusy(true);
    setError(null);
    try {
      await syncApi.resetPassword(email, code, password);
      queryClient.invalidateQueries({ queryKey: ["sync"] });
      toast("New password saved, you're signed in ♡");
      close();
      onDone?.();
    } catch (e) {
      setError(clean(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button type="button" onClick={() => setOpen(true)} className="self-start text-[11.5px] text-muted hover:text-ink hover:underline">
        Forgot your password?
      </button>
      <Modal open={open} onClose={close} title="Choose a new password" description="Your memories aren't affected: they're locked with your space key, not your password.">
        <div className="flex flex-col gap-3">
          <Field label="Your email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="email" disabled={sent} />
          {sent && (
            <>
              <p className="rounded-lg bg-sage px-3 py-2 text-[12px] text-[#3f6545]">If there's an account for this email, a 6-digit code is on its way. Check your spam folder too.</p>
              <Field label="Code from the email" value={code} onChange={(e) => setCode(e.target.value.replace(/\D/g, "").slice(0, 6))} inputMode="numeric" autoComplete="one-time-code" />
              <Field label="New password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" hint="At least 8 characters" />
            </>
          )}
          {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
          <div className="flex gap-2">
            {!sent ? (
              <Button onClick={send} disabled={busy || !email.includes("@")} icon={busy ? <Loader size={14} className="animate-spin" /> : <Mail size={14} />}>
                Email me a code
              </Button>
            ) : (
              <>
                <Button onClick={reset} disabled={busy || code.length !== 6 || password.length < 8} icon={busy ? <Loader size={14} className="animate-spin" /> : <KeyRound size={14} />}>
                  Save new password
                </Button>
                <Button variant="ghost" size="sm" onClick={send} disabled={busy}>
                  Send another code
                </Button>
              </>
            )}
          </div>
        </div>
      </Modal>
    </>
  );
}
