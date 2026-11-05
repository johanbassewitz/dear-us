import { useState } from "react";
import { BookHeart, Link2, PenLine } from "lucide-react";
import type { BucketItem } from "../../lib/api";
import { api } from "../../lib/api";
import { invalidateBucket, useEntries } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { localIsoDate } from "../../lib/dates";
import { fmtDate } from "../../lib/format";
import { Modal } from "../../components/ui/Modal";
import { Button } from "../../components/ui/Button";
import { toast, toastError } from "../../components/ui/Toast";

/** Shown after a dream is crossed off: keep the memory of it. */
export function CompleteDialog({ item, onClose }: { item: BucketItem; onClose: () => void }) {
  const openJournal = useNav((s) => s.openJournal);
  const [picking, setPicking] = useState(false);
  const [entryId, setEntryId] = useState("");
  const [busy, setBusy] = useState(false);
  const { data: entries } = useEntries({ limit: 300 });

  const write = () => {
    onClose();
    openJournal({
      mode: "edit",
      id: null,
      prefill: { title: item.title, entryDate: item.completedAt?.slice(0, 10) ?? localIsoDate(), linkTo: { kind: "bucket", id: item.id } },
    });
  };

  const link = async () => {
    if (!entryId) return;
    setBusy(true);
    try {
      await api.bucketLinkEntry(item.id, entryId);
      invalidateBucket();
      toast("Memory linked ♡");
      onClose();
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title="You did it, together ♡"
      description={`"${item.title}" is crossed off. Want to keep the memory of it?`}
      width={500}
      footer={
        <Button variant="secondary" onClick={onClose}>
          Not now
        </Button>
      }
    >
      <div className="flex flex-col gap-3">
        <button type="button" onClick={write} className="flex items-center gap-3 rounded-2xl border border-line bg-paper p-4 text-left transition-colors hover:border-accent">
          <span className="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-accent-soft text-accent">
            <PenLine size={18} />
          </span>
          <span>
            <strong className="block text-[13px]">Write the memory</strong>
            <span className="text-[11.5px] text-muted">Opens the journal with the title ready. It links back here when you save.</span>
          </span>
        </button>
        {!picking ? (
          <button type="button" onClick={() => setPicking(true)} className="flex items-center gap-3 rounded-2xl border border-line bg-paper p-4 text-left transition-colors hover:border-accent">
            <span className="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-lilac text-accent">
              <Link2 size={18} />
            </span>
            <span>
              <strong className="block text-[13px]">Link a memory</strong>
              <span className="text-[11.5px] text-muted">Pick a journal entry you already wrote.</span>
            </span>
          </button>
        ) : (
          <div className="rounded-2xl border border-line bg-paper p-4">
            {entries?.length ? (
              <>
                <label htmlFor="link-entry" className="text-[11px] font-bold text-ink-soft">
                  Choose a memory
                </label>
                <select id="link-entry" className="field-input mt-1.5" value={entryId} onChange={(e) => setEntryId(e.target.value)}>
                  <option value="">Select…</option>
                  {entries.map((e) => (
                    <option key={e.id} value={e.id}>
                      {e.title} ({fmtDate(e.entryDate)})
                    </option>
                  ))}
                </select>
                <div className="mt-3 flex justify-end">
                  <Button size="sm" icon={<BookHeart size={13} />} onClick={link} disabled={!entryId || busy}>
                    Link this memory
                  </Button>
                </div>
              </>
            ) : (
              <p className="text-[12.5px] text-muted">There are no memories in your journal yet. Write one instead ♡</p>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}
