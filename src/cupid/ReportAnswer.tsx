import { useState } from "react";
import { Flag, Loader } from "lucide-react";
import { cupidApi, type ReportReason } from "./cupidApi";
import { Button } from "../components/ui/Button";
import { Modal } from "../components/ui/Modal";
import { toast } from "../components/ui/Toast";

const REASONS: { id: ReportReason; label: string }[] = [
  { id: "harmful", label: "Hurtful or unsafe" },
  { id: "inappropriate", label: "Inappropriate" },
  { id: "wrong", label: "Wrong or misleading" },
  { id: "other", label: "Something else" },
];

/** "Report this answer" under one of Cupid's (AI) replies. */
export function ReportAnswer({ messageId }: { messageId: string }) {
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState<ReportReason | null>(null);
  const [details, setDetails] = useState("");
  const [include, setInclude] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    setOpen(false);
    setReason(null);
    setDetails("");
    setError(null);
  };
  const send = async () => {
    if (!reason) return;
    setBusy(true);
    setError(null);
    try {
      await cupidApi.report(messageId, reason, details.trim() || null, include);
      toast("Thank you. We'll look at it.");
      close();
    } catch (e) {
      setError(String(e).replace(/^Error:\s*/, ""));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button onClick={() => setOpen(true)} aria-label="Report this answer" title="Report this answer" className="mt-1 inline-flex items-center gap-1 text-[10.5px] text-faint hover:text-[#b0413e]">
        <Flag size={11} /> Report
      </button>
      <Modal
        open={open}
        onClose={close}
        title="Report Cupid's answer"
        description="Cupid's answers are written by AI and can be wrong. Tell us what was off, and we'll use it to make him better."
        footer={
          <>
            <Button variant="secondary" onClick={close}>
              Cancel
            </Button>
            <Button onClick={send} disabled={!reason || busy} icon={busy ? <Loader size={14} className="animate-spin" /> : <Flag size={14} />}>
              Send report
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-2" role="radiogroup" aria-label="What was wrong?">
          {REASONS.map((r) => (
            <label key={r.id} className="flex items-center gap-2 text-[12.5px]">
              <input type="radio" name="reason" checked={reason === r.id} onChange={() => setReason(r.id)} className="accent-[var(--accent)]" /> {r.label}
            </label>
          ))}
        </div>
        <textarea value={details} onChange={(e) => setDetails(e.target.value)} maxLength={2000} rows={3} placeholder="Anything to add? (optional)" aria-label="Details" className="field-input mt-3 w-full resize-none text-[12.5px]" />
        <label className="mt-3 flex items-start gap-2 text-[12px] text-ink-soft">
          <input type="checkbox" checked={include} onChange={(e) => setInclude(e.target.checked)} className="mt-0.5 accent-[var(--accent)]" />
          <span>Include this answer's text in the report. Your own messages are never sent.</span>
        </label>
        {error && <p className="mt-2 text-[12px] text-[#b0413e]">{error}</p>}
      </Modal>
    </>
  );
}
