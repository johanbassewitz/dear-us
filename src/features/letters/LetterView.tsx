import { useState } from "react";
import { Lock, Pencil, Trash2, Unlock } from "lucide-react";
import { api, type EntryMedia } from "../../lib/api";
import { invalidateLetters, useLetter } from "../../lib/queries";
import { Modal } from "../../components/ui/Modal";
import { Button } from "../../components/ui/Button";
import { Pill } from "../../components/ui/Pill";
import { MediaThumb } from "../../components/media/MediaThumb";
import { MediaViewer } from "../../components/media/MediaViewer";
import { RichTextView } from "../../components/richtext/RichText";
import { toastError } from "../../components/ui/Toast";
import { Envelope } from "./Envelope";
import { countdown, fmtDay, fmtMoment, readyToOpen, stillSealed, toName, useNow } from "./lettersUtils";

interface Props {
  id: string;
  onClose: () => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
}

export function LetterView({ id, onClose, onEdit, onDelete }: Props) {
  const { data: l, error } = useLetter(id);
  const now = useNow();
  const [opening, setOpening] = useState(false);
  const [viewer, setViewer] = useState<number | null>(null);

  const open = async () => {
    setOpening(true);
    try {
      await api.letterUnlock(id);
      invalidateLetters();
    } catch (e) {
      toastError(String(e));
    } finally {
      setOpening(false);
    }
  };

  const body = () => {
    if (error) return <p className="text-[13px] text-muted">{String(error)}</p>;
    if (!l) return <p className="text-[13px] text-muted">Opening…</p>;

    const sealedForMe = !l.mine && l.kind === "capsule" && !l.unlockedAt && !l.bodyJson;
    const ready = readyToOpen(l, now);
    const sealedNow = stillSealed(l, now);

    if (sealedForMe) {
      return (
        <div className="flex flex-col items-center gap-3 rounded-2xl bg-hero px-6 py-8 text-center">
          <Envelope />
          <div className="eyebrow">Time capsule from {l.senderName}</div>
          {l.title && <h3 className="font-serif text-[21px] text-ink">{l.title}</h3>}
          {ready ? (
            <>
              <p className="max-w-sm text-[12.5px] leading-relaxed text-muted">The moment has come. Opening it asks the public time-lock network for its key, so you need to be online.</p>
              <Button icon={<Unlock size={14} />} onClick={open} disabled={opening}>
                {opening ? "Opening…" : "Open the capsule"}
              </Button>
            </>
          ) : (
            <p className="max-w-sm text-[12.5px] leading-relaxed text-muted">
              Sealed until {l.unlockAt ? fmtMoment(l.unlockAt) : "its day"}{l.unlockAt ? ` (${countdown(l.unlockAt, now)})` : ""}. Some words are worth waiting for.
            </p>
          )}
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-4">
        {l.mine && sealedNow && l.unlockAt && (
          <div className="flex items-start gap-2.5 rounded-xl bg-accent-soft px-4 py-3 text-[12.5px] text-accent-deep">
            <Lock size={15} className="mt-0.5 shrink-0" />
            <span>
              Sealed until {fmtMoment(l.unlockAt)} ({countdown(l.unlockAt, now)}). {toName(l)} can't open it before then; you can always read your own copy here.
            </span>
          </div>
        )}
        <RichTextView json={l.bodyJson} />
        {l.media.length > 0 && (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(130px,1fr))] gap-2.5">
            {l.media.map((m, i) => (
              <button key={m.id} type="button" onClick={() => setViewer(i)} aria-label={`View photo ${i + 1}`} className="aspect-[4/3] overflow-hidden rounded-xl border border-line bg-paper-2">
                <MediaThumb media={m} />
              </button>
            ))}
          </div>
        )}
        {l.mine && l.status === "sent" && (
          <p className="border-t border-line-soft pt-3 text-[12px] text-muted">Will reach {toName(l)} once you're connected.</p>
        )}
      </div>
    );
  };

  const title = l ? (l.title ?? (l.mine ? "Untitled" : `A letter from ${l.senderName}`)) : "A letter";
  const sub = l
    ? `${l.mine ? "From you" : `From ${l.senderName}`} to ${l.mine ? toName(l) : "you"}${l.sentAt ? ` · ${fmtDay(l.sentAt)}` : ""}`
    : undefined;

  return (
    <>
      <Modal
        open
        onClose={onClose}
        title={title || "Untitled"}
        description={sub}
        width={640}
        footer={
          l && (
            <>
              {l.mine && l.status === "draft" && (
                <Button variant="secondary" icon={<Pencil size={14} />} onClick={() => onEdit(l.id)}>
                  Keep writing
                </Button>
              )}
              <Button variant="secondary" icon={<Trash2 size={14} />} onClick={() => onDelete(l.id)}>
                Delete
              </Button>
              <Button onClick={onClose}>Close</Button>
            </>
          )
        }
      >
        {l && (
          <div className="mb-3 flex flex-wrap gap-1.5">
            <Pill tone={l.kind === "capsule" ? "gold" : "accent"}>{l.kind === "capsule" ? "Time capsule" : "Letter"}</Pill>
            {l.status === "draft" && <Pill tone="gray">Draft · only on this PC</Pill>}
          </div>
        )}
        {body()}
      </Modal>
      {l && <MediaViewer items={l.media as EntryMedia[]} index={viewer} onClose={() => setViewer(null)} />}
    </>
  );
}
