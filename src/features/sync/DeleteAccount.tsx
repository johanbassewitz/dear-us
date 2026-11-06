import { useState } from "react";
import { Loader, Trash2 } from "lucide-react";
import { syncApi } from "./syncApi";
import { queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { Modal } from "../../components/ui/Modal";
import { toast } from "../../components/ui/Toast";

/** Store edition: delete the Dear Us account. Memories on this PC stay. */
export function DeleteAccountButton() {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    setOpen(false);
    setPassword("");
    setError(null);
  };
  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await syncApi.deleteAccount(password);
      queryClient.invalidateQueries();
      toast("Your account is deleted. Your memories are still here on this PC.");
      close();
    } catch (e) {
      setError(String(e).replace(/^Error:\s*/, ""));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button onClick={() => setOpen(true)} className="text-[11.5px] text-muted hover:text-[#b0413e]">
        Delete my account
      </button>
      <Modal
        open={open}
        onClose={close}
        title="Delete your Dear Us account?"
        footer={
          <>
            <Button variant="secondary" onClick={close}>
              Keep my account
            </Button>
            <Button variant="danger" onClick={remove} disabled={busy || password.length < 8} icon={busy ? <Loader size={14} className="animate-spin" /> : <Trash2 size={14} />}>
              Delete for good
            </Button>
          </>
        }
      >
        <ul className="mb-4 list-disc space-y-1.5 pl-5 text-[12.5px] leading-relaxed text-ink-soft">
          <li>Your memories stay on this PC, untouched, and keep working offline.</li>
          <li>Your partner keeps your shared space, its plan and everything on their PC. Syncing with you stops.</li>
          <li>
            Your account and anything still waiting in the Dear Us cloud for your devices are deleted. This can't be undone.
          </li>
          <li>
            Paying for Premium? Cancel it in your Microsoft account (account.microsoft.com → Subscriptions) so it isn't renewed.
          </li>
        </ul>
        <Field label="Your password, to confirm" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" />
        {error && <p className="mt-2 text-[12px] text-[#b0413e]">{error}</p>}
      </Modal>
    </>
  );
}
