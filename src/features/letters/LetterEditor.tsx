import { useCallback, useRef, useState } from "react";
import { ImagePlus, Lock, X } from "lucide-react";
import { api, type LetterDetail, type LetterInput, type LetterKind, type MediaRef } from "../../lib/api";
import { invalidateLetters, useProfile, useSaveLetterDraft } from "../../lib/queries";
import { useMediaImport } from "../../lib/useMediaImport";
import { Modal } from "../../components/ui/Modal";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { MediaThumb } from "../../components/media/MediaThumb";
import { RichTextEditor } from "../../components/richtext/RichText";
import { toast } from "../../components/ui/Toast";
import { fromLocalInput, isFuture, minLocalInput, toLocalInput } from "./lettersUtils";

interface Props {
  /** null = a new letter */
  draft: LetterDetail | null;
  defaultKind?: LetterKind;
  onClose: () => void;
}

export function LetterEditor({ draft, defaultKind = "letter", onClose }: Props) {
  const { data: profile } = useProfile();
  const saveDraft = useSaveLetterDraft();
  const [draftId, setDraftId] = useState<string | null>(draft?.id ?? null);
  const [kind, setKind] = useState<LetterKind>(draft?.kind ?? defaultKind);
  const [title, setTitle] = useState(draft?.title ?? "");
  const [bodyJson, setBodyJson] = useState(draft?.bodyJson ?? "");
  const [unlock, setUnlock] = useState(toLocalInput(draft?.unlockAt ?? null));
  const [showTitle, setShowTitle] = useState(draft?.showTitle ?? true);
  const [media, setMedia] = useState<MediaRef[]>(draft?.media ?? []);
  const [error, setError] = useState<string | null>(null);
  const [confirmSend, setConfirmSend] = useState(false);
  const [sending, setSending] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [confirmClose, setConfirmClose] = useState(false);

  const partner = profile?.partner?.displayName ?? null;
  const toLabel = partner ?? "your person";
  const capsule = kind === "capsule";

  const changed = () => setDirty(true);

  const onAdded = useCallback((added: MediaRef[]) => {
    setMedia((m) => [...m, ...added.filter((a) => !m.some((x) => x.id === a.id))]);
    setDirty(true);
  }, []);
  const { pick, busy } = useMediaImport(onAdded, { dropEnabled: false });

  const input = (): LetterInput => ({
    title,
    bodyJson,
    kind,
    unlockAt: capsule ? fromLocalInput(unlock) : null,
    showTitle: capsule ? showTitle : true,
    mediaIds: media.map((m) => m.id),
  });

  /** Saves the draft on this PC. Nothing is shared until it is sent. */
  const persist = async (): Promise<string> => {
    const id = await saveDraft.mutateAsync({ id: draftId, input: input() });
    setDraftId(id);
    setDirty(false);
    return id;
  };

  const onSaveDraft = async () => {
    setError(null);
    if (!title.trim() && !bodyJson) return setError("Write a few words first, then save it for later");
    try {
      await persist();
      toast("Saved as a draft. It stays on this PC until you send it ♡");
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  const beforeSend = () => {
    setError(null);
    if (!title.trim()) return setError("Give it a title before sending");
    if (!bodyJson) return setError("Write a few words first - the page is still empty");
    if (capsule && !isFuture(fromLocalInput(unlock))) return setError("Pick a moment in the future to open the capsule");
    setConfirmSend(true);
  };

  const doSend = async () => {
    setConfirmSend(false);
    setSending(true);
    try {
      const id = await persist();
      await api.letterSend(id);
      invalidateLetters();
      toast(capsule ? "Capsule sealed ♡" : "Letter sent ♡");
      onClose();
    } catch (e) {
      invalidateLetters();
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  // Modal re-focuses its first field whenever onClose changes, so hand it one stable function.
  const closeState = useRef({ dirty, blocked: false, onClose });
  closeState.current = { dirty, blocked: confirmSend || confirmClose, onClose };
  const requestClose = useCallback(() => {
    const c = closeState.current;
    if (c.blocked) return; // a confirmation is already open on top
    if (c.dirty) setConfirmClose(true);
    else c.onClose();
  }, []);
  const working = saveDraft.isPending || sending || busy > 0;

  const kinds = [
    { id: "letter", label: "A letter", hint: "Delivered as soon as you're connected" },
    { id: "capsule", label: "A time capsule", hint: "Stays sealed until a day you choose" },
  ] as const;

  return (
    <>
      <Modal
        open
        onClose={requestClose}
        title={draft ? "Keep writing" : capsule ? "A letter for later" : "A letter from the heart"}
        description={`To ${toLabel}. Drafts stay on this PC until you send them.`}
        width={680}
        footer={
          <>
            <Button variant="secondary" onClick={requestClose}>
              Cancel
            </Button>
            <Button variant="secondary" onClick={onSaveDraft} disabled={working}>
              Save draft
            </Button>
            <Button onClick={beforeSend} disabled={working} icon={capsule ? <Lock size={14} /> : undefined}>
              {sending ? (capsule ? "Sealing…" : "Sending…") : capsule ? "Seal capsule" : "Send"}
            </Button>
          </>
        }
      >
        <form className="flex flex-col gap-4" onSubmit={(e) => e.preventDefault()}>
          {error && (
            <div role="alert" className="rounded-xl border border-[#e7b9b4] bg-[#fbeceb] px-4 py-3 text-[12.5px] text-[#93322f]">
              {error}
            </div>
          )}

          <fieldset className="flex flex-col gap-1.5">
            <legend className="mb-1.5 text-[11px] font-bold text-ink-soft">What are you writing?</legend>
            <div className="grid grid-cols-2 gap-2">
              {kinds.map((k) => (
                <label
                  key={k.id}
                  className={`cursor-pointer rounded-xl border px-3.5 py-2.5 transition-colors has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent ${
                    kind === k.id ? "border-accent bg-accent-soft" : "border-line bg-paper hover:border-accent"
                  }`}
                >
                  <input
                    type="radio"
                    name="letter-kind"
                    className="sr-only"
                    checked={kind === k.id}
                    onChange={() => {
                      setKind(k.id);
                      changed();
                    }}
                    aria-label={k.label}
                  />
                  <span className="block text-[12.5px] font-bold text-ink">{k.label}</span>
                  <span className="block text-[11px] text-muted">{k.hint}</span>
                </label>
              ))}
            </div>
          </fieldset>

          <Field
            label="Title"
            value={title}
            onChange={(e) => {
              setTitle(e.target.value);
              changed();
            }}
            maxLength={200}
            placeholder={capsule ? "Open on our anniversary" : "For the days you need it"}
            autoFocus
          />

          <div className="flex flex-col gap-1.5">
            <span className="text-[11px] font-bold text-ink-soft">Your words</span>
            <RichTextEditor
              initialJson={bodyJson}
              onChange={(j) => {
                setBodyJson(j);
                changed();
              }}
              label="Write your letter"
            />
          </div>

          {capsule && (
            <div className="flex flex-col gap-3 rounded-xl border border-line bg-paper-2/60 p-4">
              <Field
                label="Opens on"
                type="datetime-local"
                value={unlock}
                min={minLocalInput()}
                onChange={(e) => {
                  setUnlock(e.target.value);
                  changed();
                }}
                hint="Pick a date and time in the future."
              />
              <label className="flex items-start gap-2.5 text-[12.5px] text-ink-soft">
                <input
                  type="checkbox"
                  className="mt-0.5 h-4 w-4 accent-[var(--accent)]"
                  checked={showTitle}
                  onChange={(e) => {
                    setShowTitle(e.target.checked);
                    changed();
                  }}
                />
                <span>
                  Show the title while it's sealed
                  <span className="block text-[11px] text-muted">Turn it off to keep even the title a surprise.</span>
                </span>
              </label>
              <p className="text-[11.5px] leading-relaxed text-muted">
                Your words are locked with the public drand time-lock network, so nobody can read them before the moment, not even {toLabel}. Your own copy stays readable here, marked as sealed. Opening a capsule later needs an internet connection.
              </p>
            </div>
          )}

          <div className="flex flex-col gap-2">
            <span className="text-[11px] font-bold text-ink-soft">Photos</span>
            {media.length > 0 && (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(110px,1fr))] gap-2.5">
                {media.map((m) => (
                  <div key={m.id} className="group relative aspect-[4/3] overflow-hidden rounded-xl border border-line bg-paper-2">
                    <MediaThumb media={m} />
                    <button
                      type="button"
                      onClick={() => {
                        setMedia((x) => x.filter((y) => y.id !== m.id));
                        changed();
                      }}
                      aria-label="Remove photo"
                      className="absolute right-1 top-1 grid h-6 w-6 place-items-center rounded-full bg-black/45 text-white opacity-0 transition-opacity focus:opacity-100 group-hover:opacity-100"
                    >
                      <X size={12} />
                    </button>
                  </div>
                ))}
              </div>
            )}
            <div>
              <Button variant="secondary" size="sm" icon={<ImagePlus size={13} />} onClick={pick} disabled={busy > 0 || media.length >= 12}>
                {busy > 0 ? "Adding…" : media.length ? "Add more photos" : "Add photos"}
              </Button>
            </div>
          </div>
        </form>
      </Modal>

      <ConfirmDialog
        open={confirmSend}
        title={capsule ? "Seal this capsule?" : "Send this letter?"}
        message={
          capsule
            ? `Once sealed it can't be changed, and ${toLabel} can't open it before the day you chose. You'll still be able to read it here.${partner ? "" : " It will reach your person once you're connected."}`
            : `Once sent it can't be edited. ${partner ? `${partner} will receive it` : "It will reach your person"} once you're connected.`
        }
        confirmLabel={capsule ? "Seal it" : "Send it"}
        onConfirm={doSend}
        onCancel={() => setConfirmSend(false)}
      />
      <ConfirmDialog
        open={confirmClose}
        title="Leave without saving?"
        message="Your latest changes haven't been saved as a draft."
        confirmLabel="Discard changes"
        danger
        onConfirm={onClose}
        onCancel={() => setConfirmClose(false)}
      />
    </>
  );
}
