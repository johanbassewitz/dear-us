import { useMemo, useRef, useState } from "react";
import { Heart, ImagePlus, Pencil, Plus, Star, Trash2, X } from "lucide-react";
import { api, type MediaRef, type TimelineItem } from "../../lib/api";
import { invalidateJournal, useEntries, useSaveMilestone, useTimeline } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { fmtDate, fmtMonth } from "../../lib/format";
import { localIsoDate } from "../../lib/dates";
import { useMediaImport } from "../../lib/useMediaImport";
import { Button } from "../../components/ui/Button";
import { EmptyState } from "../../components/ui/EmptyState";
import { Field } from "../../components/ui/Field";
import { Modal } from "../../components/ui/Modal";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { MediaThumb } from "../../components/media/MediaThumb";
import { toast, toastError } from "../../components/ui/Toast";

interface Group {
  year: string;
  months: { key: string; label: string; items: TimelineItem[] }[];
}

export function groupTimeline(items: TimelineItem[]): Group[] {
  const years: Group[] = [];
  for (const it of items) {
    const y = it.date.slice(0, 4);
    const m = it.date.slice(0, 7);
    let g = years.find((x) => x.year === y);
    if (!g) years.push((g = { year: y, months: [] }));
    let mm = g.months.find((x) => x.key === m);
    if (!mm) g.months.push((mm = { key: m, label: fmtMonth(`${m}-01`), items: [] }));
    mm.items.push(it);
  }
  return years;
}

export function TimelinePage() {
  const [major, setMajor] = useState(false);
  const { data: items, isLoading } = useTimeline(major);
  const openJournal = useNav((s) => s.openJournal);
  const [editing, setEditing] = useState<TimelineItem | "new" | null>(null);
  const [deleting, setDeleting] = useState<TimelineItem | null>(null);
  const groups = useMemo(() => groupTimeline(items ?? []), [items]);
  const yearRefs = useRef<Record<string, HTMLDivElement | null>>({});
  let side = 0;

  const remove = async () => {
    const m = deleting!;
    setDeleting(null);
    try {
      await api.milestoneDelete(m.id);
      invalidateJournal();
      toast("Milestone removed");
    } catch (e) {
      toastError(String(e));
    }
  };

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="eyebrow">Every chapter</div>
          <h1 className="page-title mt-2">Our timeline</h1>
          <p className="mt-2 text-[13px] text-muted">Your story, milestone by milestone.</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex overflow-hidden rounded-[11px] border border-line" role="group" aria-label="Show">
            {[
              [false, "All memories"],
              [true, "Milestones"],
            ].map(([v, label]) => (
              <button
                key={String(v)}
                aria-pressed={major === v}
                onClick={() => setMajor(v as boolean)}
                className={`px-3.5 py-2 text-[12px] font-semibold ${major === v ? "bg-accent text-on-accent" : "bg-paper text-muted hover:text-ink"}`}
              >
                {label as string}
              </button>
            ))}
          </div>
          <Button icon={<Plus size={15} />} onClick={() => setEditing("new")}>
            Add milestone
          </Button>
        </div>
      </div>

      {groups.length > 1 && (
        <nav aria-label="Jump to year" className="sticky top-[72px] z-20 -mx-2 mb-4 flex flex-wrap gap-1.5 bg-bg/90 px-2 py-2 backdrop-blur">
          {groups.map((g) => (
            <button
              key={g.year}
              onClick={() => yearRefs.current[g.year]?.scrollIntoView({ behavior: "smooth", block: "start" })}
              className="rounded-full border border-line bg-paper px-3 py-1 text-[11.5px] font-bold text-muted hover:border-accent hover:text-accent-deep"
            >
              {g.year}
            </button>
          ))}
        </nav>
      )}

      {isLoading ? null : !groups.length ? (
        <div className="card">
          <EmptyState
            icon={<Heart size={22} />}
            title={major ? "No milestones yet" : "Your timeline is waiting"}
            action={
              <Button icon={<Plus size={15} />} onClick={() => setEditing("new")}>
                Add your first milestone
              </Button>
            }
          >
            Add the day you met, your first date, your first trip… Memories you save in the journal appear here too.
          </EmptyState>
        </div>
      ) : (
        <div className="relative mx-auto max-w-[920px] pb-6">
          <div className="absolute top-0 bottom-0 left-1/2 border-l-2 border-dashed border-[var(--hero-line)]" aria-hidden="true" />
          {groups.map((g) => (
            <div key={g.year} ref={(el) => void (yearRefs.current[g.year] = el)} className="scroll-mt-32">
              <div className="relative z-10 my-7 flex justify-center">
                <span className="rounded-full bg-accent px-5 py-1.5 text-[12px] font-bold text-on-accent shadow">{g.year}</span>
              </div>
              {g.months.map((m) => (
                <div key={m.key}>
                  <div className="relative z-10 mb-5 flex justify-center">
                    <span className="rounded-full border border-line bg-paper px-3 py-0.5 text-[10.5px] font-semibold tracking-wide text-muted uppercase">{m.label}</span>
                  </div>
                  {m.items.map((it) => {
                    const left = side++ % 2 === 0;
                    const card = (
                      <div className="card overflow-hidden text-left">
                        {it.cover && (
                          <div className="h-[140px]">
                            <MediaThumb media={it.cover} />
                          </div>
                        )}
                        <div className="p-4">
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-[10px] font-bold tracking-wide text-faint uppercase">{fmtDate(it.date)}</span>
                            {it.kind === "milestone" && (
                              <span className="flex gap-1">
                                <button onClick={() => setEditing(it)} aria-label="Edit milestone" className="rounded p-1 text-faint hover:text-ink">
                                  <Pencil size={13} />
                                </button>
                                <button onClick={() => setDeleting(it)} aria-label="Delete milestone" className="rounded p-1 text-faint hover:text-[#b0413e]">
                                  <Trash2 size={13} />
                                </button>
                              </span>
                            )}
                          </div>
                          <h3 className="mt-1 font-serif text-[19px] leading-snug">{it.title}</h3>
                          {it.text && <p className="mt-1.5 line-clamp-3 text-[12px] leading-relaxed text-muted">{it.text}</p>}
                          {it.linkedEntryId && (
                            <button onClick={() => openJournal({ mode: "view", id: it.linkedEntryId! })} className="mt-2.5 text-[11.5px] font-bold text-accent hover:text-accent-deep">
                              Open memory →
                            </button>
                          )}
                        </div>
                      </div>
                    );
                    return (
                      <div key={`${it.kind}-${it.id}`} className="mb-7 grid grid-cols-[1fr_56px_1fr] items-center">
                        <div>{left && card}</div>
                        <div className="z-10 flex justify-center">
                          <span
                            className={`grid h-9 w-9 place-items-center rounded-full border-2 ${it.major ? "border-accent bg-accent text-on-accent" : "border-[var(--hero-line)] bg-hero text-accent"}`}
                            title={it.kind === "milestone" ? "Milestone" : "Memory"}
                          >
                            {it.major ? <Star size={14} className="fill-current" /> : <Heart size={14} />}
                          </span>
                        </div>
                        <div>{!left && card}</div>
                      </div>
                    );
                  })}
                </div>
              ))}
            </div>
          ))}
        </div>
      )}

      {editing && <MilestoneModal item={editing === "new" ? null : editing} onClose={() => setEditing(null)} />}
      <ConfirmDialog
        open={!!deleting}
        title="Remove this milestone?"
        message="Linked memories stay in your journal. Only the milestone marker is removed."
        confirmLabel="Remove"
        danger
        onConfirm={remove}
        onCancel={() => setDeleting(null)}
      />
    </div>
  );
}

function MilestoneModal({ item, onClose }: { item: TimelineItem | null; onClose: () => void }) {
  const save = useSaveMilestone();
  const { data: entries } = useEntries({ limit: 1000 });
  const [title, setTitle] = useState(item?.title ?? "");
  const [date, setDate] = useState(item?.date ?? localIsoDate());
  const [description, setDescription] = useState(item?.text ?? "");
  const [linked, setLinked] = useState(item?.linkedEntryId ?? "");
  const [cover, setCover] = useState<MediaRef | null>(item?.cover ?? null);
  const [error, setError] = useState<string | null>(null);
  const { pick, busy } = useMediaImport((m) => setCover(m[0]), { dropEnabled: false, multiple: false });

  const submit = async () => {
    setError(null);
    try {
      await save.mutateAsync({
        id: item?.id ?? null,
        input: { title, date, description, linkedEntryId: linked || null, coverMediaId: cover?.id ?? null },
      });
      toast(item ? "Milestone updated" : "Milestone added ♡");
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={item ? "Edit milestone" : "A new milestone"}
      description="First meeting, first date, first trip, big moments."
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={save.isPending || busy > 0}>
            {item ? "Save" : "Add milestone"}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <Field label="Title" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="The day we met" maxLength={200} autoFocus />
        <Field label="Date" type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        <div className="flex flex-col gap-1.5">
          <label htmlFor="ms-desc" className="text-[11px] font-bold text-ink-soft">
            A few words
          </label>
          <textarea id="ms-desc" value={description} onChange={(e) => setDescription(e.target.value)} maxLength={2000} rows={3} className="field-input resize-y" />
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor="ms-link" className="text-[11px] font-bold text-ink-soft">
            Link a memory (optional)
          </label>
          <select id="ms-link" value={linked} onChange={(e) => setLinked(e.target.value)} className="field-input">
            <option value="">None</option>
            {entries?.map((e) => (
              <option key={e.id} value={e.id}>
                {fmtDate(e.entryDate)} · {e.title}
              </option>
            ))}
          </select>
        </div>
        <div className="flex items-center gap-3">
          <div className="h-16 w-16 shrink-0 overflow-hidden rounded-lg border border-line">
            <MediaThumb media={cover} />
          </div>
          <Button variant="secondary" size="sm" icon={<ImagePlus size={13} />} onClick={pick} disabled={busy > 0}>
            {cover ? "Change photo" : "Add a photo"}
          </Button>
          {cover && (
            <button onClick={() => setCover(null)} className="inline-flex items-center gap-1 text-[11.5px] text-muted hover:text-ink">
              <X size={12} /> Remove
            </button>
          )}
        </div>
        {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
      </div>
    </Modal>
  );
}
