import { useState } from "react";
import { ClipboardCopy, Loader, MonitorSmartphone, Smartphone } from "lucide-react";
import { syncApi, type DeviceLink } from "./syncApi";
import { queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { Modal } from "../../components/ui/Modal";
import { toast, toastError } from "../../components/ui/Toast";
import { ForgotPassword } from "./ForgotPassword";
import cupidThinking from "../../cupid/assets/cupid-thinking.webp";

const clean = (e: unknown) => String(e).replace(/^Error:\s*/, "");

/** On a device that's already set up: a code (and QR) to add one more of MY devices. */
export function LinkDeviceButton() {
  const [link, setLink] = useState<DeviceLink | null>(null);
  const [busy, setBusy] = useState(false);
  const make = async () => {
    setBusy(true);
    try {
      setLink(await syncApi.createDeviceLink());
    } catch (e) {
      toastError(clean(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Button variant="secondary" size="sm" icon={busy ? <Loader size={13} className="animate-spin" /> : <MonitorSmartphone size={13} />} onClick={make} disabled={busy}>
        Link my phone or another PC
      </Button>
      <Modal
        open={!!link}
        onClose={() => setLink(null)}
        title="Link another device of yours"
        description="Use the same Dear Us account on it. Everything you two have saved comes along, then it stays in sync."
      >
        {link && (
          <div className="flex flex-col gap-4">
            <div className="flex flex-wrap items-center gap-5">
              <div className="h-[200px] w-[200px] shrink-0 rounded-xl bg-white p-2" aria-label="QR code" dangerouslySetInnerHTML={{ __html: link.qrSvg }} />
              <ol className="min-w-[200px] flex-1 list-decimal space-y-1.5 pl-4 text-[12.5px] text-ink-soft">
                <li>Install Dear Us on the other device.</li>
                <li>
                  Choose <b>Link to my other device</b>.
                </li>
                <li>Scan this code with the phone's camera, or paste the code below on a PC.</li>
                <li>Sign in with your own password. Keep Dear Us open here until it's done.</li>
              </ol>
            </div>
            <textarea readOnly value={link.code} rows={3} className="field-input resize-none font-mono text-[10.5px]" onFocus={(e) => e.target.select()} />
            <div className="flex flex-wrap items-center gap-2">
              <Button
                size="sm"
                icon={<ClipboardCopy size={13} />}
                onClick={async () => {
                  await navigator.clipboard.writeText(link.code).catch(() => {});
                  toast("Link code copied");
                }}
              >
                Copy link code
              </Button>
              <span className="text-[11px] text-faint">Works once, for {link.expiresInMinutes} minutes, and only with your account. It holds your space key: don't share it.</span>
            </div>
          </div>
        )}
      </Modal>
    </>
  );
}

/** On the new device: paste the link code, sign in as the same person. */
export function LinkForm({ onLinked }: { onLinked?: () => void }) {
  const [code, setCode] = useState("");
  const [who, setWho] = useState<{ email: string; fromDevice: string } | null>(null);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const read = async (c: string) => {
    setCode(c);
    setError(null);
    setWho(null);
    if (!c.trim().startsWith("DEARUS-LINK1-")) return;
    try {
      setWho(await syncApi.readDeviceLink(c));
    } catch (e) {
      setError(clean(e));
    }
  };
  const link = async () => {
    setBusy(true);
    setError(null);
    try {
      await syncApi.linkDevice(code, password);
      queryClient.invalidateQueries({ queryKey: ["sync"] });
      onLinked?.();
    } catch (e) {
      setError(clean(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <label htmlFor="link-code" className="text-[11px] font-bold text-ink-soft">
          Link code from your other device
        </label>
        <textarea id="link-code" value={code} onChange={(e) => read(e.target.value)} rows={3} placeholder="DEARUS-LINK1-…" spellCheck={false} className="field-input resize-none font-mono text-[11.5px]" />
        <span className="text-[11px] text-muted">
          On your other device: Settings &amp; Sync → <b>Link my phone or another PC</b>.
        </span>
      </div>
      {who && (
        <>
          <p className="rounded-lg bg-paper-2 px-3 py-2 text-[12px] text-ink-soft">
            Linking to <b>{who.fromDevice}</b> as <b>{who.email}</b>.
          </p>
          <Field label="Your password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" />
          <ForgotPassword initialEmail={who.email} />
        </>
      )}
      {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
      <div>
        <Button onClick={link} disabled={busy || !who || password.length < 8} icon={busy ? <Loader size={15} className="animate-spin" /> : <Smartphone size={15} />}>
          {busy ? "Linking…" : "Link this device"}
        </Button>
      </div>
    </div>
  );
}

/** Shown on a newly linked device until its snapshot has arrived. */
export function LinkingScreen() {
  return (
    <div className="flex h-full items-center justify-center bg-bg p-6">
      <div className="card w-full max-w-[480px] p-10 text-center">
        <img src={cupidThinking} alt="" className="cupid-float mx-auto mb-3 w-[160px] select-none" draggable={false} />
        <h1 className="page-title">Bringing your memories over…</h1>
        <p className="mt-3 text-[13px] leading-relaxed text-muted">
          Your other device is sending everything you two have saved, encrypted all the way. Keep Dear Us open there. Photos keep arriving in the background after you're in.
        </p>
        <Loader size={18} className="mx-auto mt-5 animate-spin text-accent" />
      </div>
    </div>
  );
}
