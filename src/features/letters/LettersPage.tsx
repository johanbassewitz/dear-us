import { useState } from "react";
import { Heart, Inbox, Lock, Mail, Pencil, Plus, Send, Trash2, Unlock } from "lucide-react";
import { api, type LetterDetail, type LetterKind, type LetterSummary } from "../../lib/api";
import { invalidateLetters, useLetters, useProfile } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { MediaThumb } from "../../components/media/MediaThumb";
import { toast, toastError } from "../../components/ui/Toast";
import { Envelope } from "./Envelope";
import { LetterEditor } from "./LetterEditor";
import { LetterView } from "./LetterView";
import { countdown, fmtDay, fmtMoment, group, nextToOpen, readyToOpen, stillSealed, toName, useNow, type LetterTab } from "./lettersUtils";

const TABS: { id: LetterTab; label: string }[] = [
  { id: "letters", label: "Letters" },
  { id: "capsules", label: "Time capsules" },
  { id: "drafts", label: "Drafts" },
];

function LetterCard({
  l,
  now,
  onOpen,
  onEdit,
  onDelete,
}: {
  l: LetterSummary;
  now: number;
  onOpen: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const sealed = stillSealed(l, now) || (!l.mine && l.kind === "capsule" && !l.unlockedAt);
  const ready = readyToOpen(l, now);
  const draft = l.status === "draft";
  const title = l.title ?? `A letter from ${l.senderName}`;
  const label = draft
    ? `DRAFT · ${l.kind === "capsule" ? "TIME CAPSULE" : "LETTER"}`
    : l.kind === "capsule"
      ? `TIME CAPSULE${l.unlockAt ? ` · OPENS ${fmtDay(l.unlockAt).toUpperCase()}` : ""}`
      : l.mine
        ? `TO ${toName(l).toUpperCase()}`
        : `FROM ${l.senderName.toUpperCase()}`;

  return (
    <article className={`card flex flex-col p-5 ${sealed ? "bg-paper-2" : ""}`} aria-label={title || "Untitled"}>
      {l.cover && !sealed ? (
        <div className="-mx-5 -mt-5 mb-4 aspect-[16/8] overflow-hidden rounded-t-[inherit]">
          <MediaThumb media={l.cover} />
        </div>
      ) : (
        <div className={`grid h-11 w-11 place-items-center rounded-full ${sealed ? "bg-gold text-[#9a7448]" : "bg-blush text-accent"}`}>
          {sealed ? <Lock size={19} /> : draft ? <Pencil size={18} /> : <Heart size={19} />}
        </div>
      )}
      <div className="tiny-label mt-3.5">{label}</div>
      <h3 className="mt-1 font-serif text-[18px] leading-snug text-ink">{title || "Untitled"}</h3>
      <p className="mt-1.5 line-clamp-3 min-h-[3.2em] text-[12px] leading-relaxed text-muted">
        {sealed
          ? l.mine
            ? `Sealed until ${l.unlockAt ? fmtMoment(l.unlockAt) : "its day"}. You can still read your own copy.`
            : ready
              ? "The moment has come. Open it whenever you're ready."
              : "A surprise waiting for a future day."
          : l.excerpt || (draft ? "Nothing written yet." : "")}
      </p>
      {sealed && l.unlockAt && !ready && (
        <div className="mt-1 inline-flex items-center gap-1.5 text-[11.5px] font-bold text-accent-deep">
          <Lock size={12} /> Opens {countdown(l.unlockAt, now)}
        </div>
      )}
      {l.mine && l.status === "sent" && <div className="mt-1 text-[11px] text-muted">Will reach {toName(l)} once you're connected.</div>}
      <div className="mt-4 flex items-center justify-between gap-2 border-t border-line-soft pt-3 text-[11px] text-muted">
        <span>{l.sentAt ? fmtDay(l.sentAt) : `Edited ${fmtDay(l.updatedAt)}`}</span>
        <div className="flex items-center gap-1">
          {ready ? (
            <Button size="sm" icon={<Unlock size={13} />} onClick={onOpen}>
              Open
            </Button>
          ) : (
            <Button size="sm" variant="secondary" onClick={draft ? onEdit : onOpen}>
              {draft ? "Keep writing" : sealed && !l.mine ? "Look" : "Read letter"}
            </Button>
          )}
          <button
            onClick={onDelete}
            aria-label={`Delete "${title || "Untitled"}"`}
            title="Delete"
            className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep"
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>
    </article>
  );
}

export function LettersPage() {
  const { data: letters, isLoading, error } = useLetters();
  const { data: profile } = useProfile();
  const now = useNow();
  const [tab, setTab] = useState<LetterTab>("letters");
  const [composing, setComposing] = useState<{ draft: LetterDetail | null; kind: LetterKind } | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const [removing, setRemoving] = useState<{ id: string; title: string; sent: boolean } | null>(null);

  const all = letters ?? [];
  const partner = profile?.partner?.displayName ?? null;
  const shelves = group(all, tab);
  const upcoming = nextToOpen(all, now);
  const draftCount = all.filter((l) => l.status === "draft").length;

  const edit = async (id: string) => {
    try {
      const draft = await api.letterGet(id);
      setViewing(null);
      setComposing({ draft, kind: draft.kind });
    } catch (e) {
      toastError(String(e));
    }
  };

  const askDelete = (id: string) => {
    const l = all.find((x) => x.id === id);
    if (l) setRemoving({ id, title: l.title ?? `A letter from ${l.senderName}`, sent: l.status === "sent" && l.mine });
    else setRemoving({ id, title: "this letter", sent: false });
  };

  const remove = async () => {
    const r = removing;
    setRemoving(null);
    if (!r) return;
    try {
      await api.letterDelete(r.id);
      setViewing(null);
      invalidateLetters();
      toast("Letter removed", {
        label: "Undo",
        run: () =>
          api
            .letterRestore(r.id)
            .then(invalidateLetters)
            .catch((e) => toastError(String(e))),
      });
    } catch (e) {
      toastError(String(e));
    }
  };

  const card = (l: LetterSummary) => (
    <LetterCard
      key={l.id}
      l={l}
      now={now}
      onOpen={() => setViewing(l.id)}
      onEdit={() => edit(l.id)}
      onDelete={() => askDelete(l.id)}
    />
  );

  const tabNoun = tab === "letters" ? "letters" : "capsules";

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="eyebrow">Words to keep forever</div>
          <h1 className="page-title mt-2">Letters, from the heart.</h1>
          <p className="mt-2 text-[13px] text-muted">Sometimes love is a few words you never want to forget.</p>
        </div>
        <Button icon={<Plus size={15} />} onClick={() => setComposing({ draft: null, kind: "letter" })}>
          Write a letter
        </Button>
      </div>

      <section className="card mb-8 grid items-center overflow-hidden bg-hero md:grid-cols-[1.1fr_0.9fr]" aria-label="Time capsules">
        <div className="p-7">
          <div className="eyebrow">A note to the future</div>
          <h2 className="mt-2 font-serif text-[27px] leading-tight text-ink">Some words are worth waiting for.</h2>
          <p className="mt-2 max-w-sm text-[12.5px] leading-relaxed text-muted">
            Write something today, tuck it away, and let it open when the moment feels just right.
          </p>
          {upcoming?.unlockAt && (
            <p className="mt-3 inline-flex items-center gap-1.5 text-[12px] font-bold text-accent-deep">
              <Lock size={13} /> Next to open: {upcoming.title ?? "a surprise"} {countdown(upcoming.unlockAt, now)}
            </p>
          )}
          <div className="mt-4">
            <Button variant="secondary" icon={<Heart size={14} />} onClick={() => setComposing({ draft: null, kind: "capsule" })}>
              Create time capsule
            </Button>
          </div>
        </div>
        <Envelope className="min-h-[200px]" />
      </section>

      <div className="mb-4">
        <h2 className="section-title">Our little collection</h2>
        <p className="mt-1 text-[12px] text-muted">Every letter has a story behind it.</p>
      </div>

      <div role="tablist" aria-label="Letters" className="mb-5 flex flex-wrap gap-2">
        {TABS.map((t) => {
          const active = tab === t.id;
          return (
            <button
              key={t.id}
              role="tab"
              id={`letters-tab-${t.id}`}
              aria-selected={active}
              aria-controls="letters-panel"
              onClick={() => setTab(t.id)}
              className={`inline-flex items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[12px] font-semibold transition-colors ${
                active ? "bg-accent text-on-accent" : "border border-line bg-paper text-muted hover:border-accent hover:text-accent-deep"
              }`}
            >
              {t.label}
              {t.id === "drafts" && draftCount > 0 && <span className="rounded-full bg-black/10 px-1.5 text-[10px]">{draftCount}</span>}
            </button>
          );
        })}
      </div>

      <div id="letters-panel" role="tabpanel" aria-labelledby={`letters-tab-${tab}`}>
        {error ? (
          <p className="text-[13px] text-muted">{String(error)}</p>
        ) : isLoading ? (
          <p className="text-[13px] text-muted">Opening your letters…</p>
        ) : tab === "drafts" ? (
          shelves.drafts.length ? (
            <>
              <p className="mb-3 text-[12px] text-muted">Drafts stay on this PC. Nobody sees them until you send them.</p>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">{shelves.drafts.map(card)}</div>
            </>
          ) : (
            <div className="card">
              <EmptyState icon={<Pencil size={22} />} title="No drafts waiting" action={<Button onClick={() => setComposing({ draft: null, kind: "letter" })}>Start a letter</Button>}>
                Begin something, save it for later, and it will wait for you here.
              </EmptyState>
            </div>
          )
        ) : (
          <div className="flex flex-col gap-8">
            <section aria-label={`Inbox: ${tabNoun} for you`}>
              <h3 className="mb-3 flex items-center gap-2 font-serif text-[17px] text-ink">
                <Inbox size={16} className="text-accent" /> For you
              </h3>
              {shelves.received.length ? (
                <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">{shelves.received.map(card)}</div>
              ) : (
                <div className="card">
                  <EmptyState compact icon={<Mail size={20} />} title={tab === "letters" ? "Nothing in your inbox yet" : "No capsules waiting for you yet"}>
                    {partner ? `What ${partner} writes to you` : "What your person writes to you"} will arrive here once you're connected.
                  </EmptyState>
                </div>
              )}
            </section>

            <section aria-label={`Sent: ${tabNoun} from you`}>
              <h3 className="mb-3 flex items-center gap-2 font-serif text-[17px] text-ink">
                <Send size={16} className="text-accent" /> From you
              </h3>
              {shelves.sent.length ? (
                <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">{shelves.sent.map(card)}</div>
              ) : (
                <div className="card">
                  <EmptyState
                    compact
                    icon={<Heart size={20} />}
                    title={tab === "letters" ? "No letters sent yet" : "No capsules sealed yet"}
                    action={
                      <Button size="sm" onClick={() => setComposing({ draft: null, kind: tab === "letters" ? "letter" : "capsule" })}>
                        {tab === "letters" ? "Write the first one" : "Create a capsule"}
                      </Button>
                    }
                  >
                    {tab === "letters" ? "A few kind words can brighten a whole week." : "Seal something today for a day far away."}
                  </EmptyState>
                </div>
              )}
            </section>
          </div>
        )}
      </div>

      {composing && <LetterEditor draft={composing.draft} defaultKind={composing.kind} onClose={() => setComposing(null)} />}
      {viewing && <LetterView id={viewing} onClose={() => setViewing(null)} onEdit={edit} onDelete={askDelete} />}
      <ConfirmDialog
        open={!!removing}
        title="Delete this letter?"
        message={
          removing?.sent
            ? `"${removing.title}" will move out of your collection. Anything already delivered stays with ${partner ?? "your person"}. You can undo this for a few moments, and it's kept for 30 days.`
            : `"${removing?.title ?? ""}" will be removed. You can undo this for a few moments.`
        }
        confirmLabel="Delete"
        danger
        onConfirm={remove}
        onCancel={() => setRemoving(null)}
      />
    </div>
  );
}
