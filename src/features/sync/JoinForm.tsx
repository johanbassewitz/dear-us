import { useState } from "react";
import { Heart, Loader } from "lucide-react";
import { syncApi, syncKeys } from "./syncApi";
import { keys, queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";

/** Join the partner's space with their invite code. `needName` on a brand-new install. */
export function JoinForm({ needName, onJoined }: { needName: boolean; onJoined?: () => void }) {
  const [invite, setInvite] = useState("");
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [create, setCreate] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmEmail, setConfirmEmail] = useState(false);

  const submit = async (createAccount: boolean) => {
    setError(null);
    if (!invite.trim().startsWith("DEARUS1-")) return setError("Paste the whole invite code your partner sent you. It starts with DEARUS1-");
    if (needName && !name.trim()) return setError("Tell us your name first ♡");
    setBusy(true);
    try {
      const r = await syncApi.join({ invite: invite.trim(), email, password, createAccount, myName: needName ? name.trim() : null });
      if (r.needsEmailConfirmation) {
        setConfirmEmail(true);
        setCreate(false);
      } else if (r.joined) {
        await queryClient.invalidateQueries({ queryKey: keys.profile });
        queryClient.invalidateQueries({ queryKey: syncKeys.status });
        onJoined?.();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <label htmlFor="invite" className="text-[11px] font-bold text-ink-soft">
          Invite code from your partner
        </label>
        <textarea id="invite" value={invite} onChange={(e) => setInvite(e.target.value)} rows={3} placeholder="DEARUS1-…" spellCheck={false} className="field-input resize-none font-mono text-[11.5px]" />
        <span className="text-[11px] text-muted">It works once and expires after 48 hours. Your partner can make a new one anytime.</span>
      </div>
      {needName && <Field label="Your name" value={name} onChange={(e) => setName(e.target.value)} maxLength={60} />}
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Your email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="email" />
        <Field label="Password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete={create ? "new-password" : "current-password"} hint="At least 8 characters" />
      </div>
      <label className="flex items-center gap-2 text-[12px] text-ink-soft">
        <input type="checkbox" checked={!create} onChange={(e) => setCreate(!e.target.checked)} className="accent-[var(--accent)]" />
        I already have an account in our project
      </label>
      {confirmEmail && (
        <div className="rounded-xl bg-gold px-4 py-3 text-[12.5px] text-ink-soft">
          Check your inbox and confirm your email, then press <b>I've confirmed, join</b>.
        </div>
      )}
      {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
      <div>
        <Button onClick={() => submit(confirmEmail ? false : create)} disabled={busy} icon={busy ? <Loader size={15} className="animate-spin" /> : <Heart size={15} />}>
          {busy ? "Joining…" : confirmEmail ? "I've confirmed, join" : "Join our space"}
        </Button>
      </div>
    </div>
  );
}
