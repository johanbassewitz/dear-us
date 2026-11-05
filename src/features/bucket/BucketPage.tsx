import { useMemo, useState } from "react";
import { ArrowRight, CalendarDays, Check, ListChecks, Pencil, Plus, RotateCcw, Search, Trash2, X } from "lucide-react";
import { api, type BucketItem, type BucketStatus } from "../../lib/api";
import { invalidateBucket, useBucket } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { fmtShort } from "../../lib/format";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { Pill } from "../../components/ui/Pill";
import { MediaThumb } from "../../components/media/MediaThumb";
import { toast, toastError } from "../../components/ui/Toast";
import { BucketEditor } from "./BucketEditor";
import { CompleteDialog } from "./CompleteDialog";
import { COLUMNS, categoriesOf, filterItems, progressLabel, sortColumn } from "./bucketUtils";

function ProgressRing({ done, total }: { done: number; total: number }) {
  const r = 26;
  const c = 2 * Math.PI * r;
  const pct = total ? done / total : 0;
  return (
    <svg width="64" height="64" viewBox="0 0 64 64" role="img" aria-label={progressLabel(done, total)}>
      <circle cx="32" cy="32" r={r} fill="none" stroke="var(--line)" strokeWidth="6" />
      <circle
        cx="32"
        cy="32"
        r={r}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="6"
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - pct)}
        transform="rotate(-90 32 32)"
      />
      <text x="32" y="36" textAnchor="middle" className="fill-ink font-serif" fontSize="14">
        {Math.round(pct * 100)}%
      </text>
    </svg>
  );
}

const PRIORITY_TONE = { high: "accent", medium: "gold", low: "gray" } as const;
const PRIORITY_LABEL = { high: "High", medium: "Medium", low: "Low" } as const;

function Card({
  item,
  onEdit,
  onDelete,
  onStatus,
}: {
  item: BucketItem;
  onEdit: () => void;
  onDelete: () => void;
  onStatus: (s: BucketStatus) => void;
}) {
  const openJournal = useNav((s) => s.openJournal);
  const done = item.status === "done";
  return (
    <article className="card overflow-hidden" aria-label={item.title}>
      {item.cover && (
        <div className="aspect-[16/9] w-full">
          <MediaThumb media={item.cover} />
        </div>
      )}
      <div className="p-4">
        <div className="mb-2 flex flex-wrap items-center gap-1.5">
          {item.category && <Pill tone="accent">{item.category}</Pill>}
          <Pill tone={PRIORITY_TONE[item.priority]}>{PRIORITY_LABEL[item.priority]} priority</Pill>
        </div>
        <h3 className={`font-serif text-[17px] leading-snug ${done ? "text-ink-soft" : "text-ink"}`}>{item.title}</h3>
        {item.description && <p className="mt-1 line-clamp-3 text-[12px] leading-relaxed text-muted">{item.description}</p>}
        <div className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11.5px] text-muted">
          {done && item.completedAt && (
            <span className="inline-flex items-center gap-1 text-accent-deep">
              <Check size={12} /> Done {fmtShort(item.completedAt)}
            </span>
          )}
          {item.targetDate && (
            <span className="inline-flex items-center gap-1">
              <CalendarDays size={12} /> {done ? "Aimed for" : "Target"} {fmtShort(item.targetDate)}
            </span>
          )}
        </div>
        {item.linkedEntryId && (
          <button onClick={() => openJournal({ mode: "view", id: item.linkedEntryId! })} className="mt-2.5 inline-flex items-center gap-1 text-[12px] font-bold text-accent hover:text-accent-deep" aria-label={`Open memory: ${item.linkedEntryTitle ?? item.title}`}>
            Open memory <ArrowRight size={12} />
          </button>
        )}
        <div className="mt-3 flex items-center justify-between gap-2 border-t border-line-soft pt-3">
          <div>
            {item.status === "want" && (
              <Button size="sm" variant="secondary" onClick={() => onStatus("planned")}>
                Start planning
              </Button>
            )}
            {item.status === "planned" && (
              <Button size="sm" icon={<Check size={13} />} onClick={() => onStatus("done")}>
                Mark done
              </Button>
            )}
            {done && (
              <Button size="sm" variant="secondary" icon={<RotateCcw size={13} />} onClick={() => onStatus("want")}>
                Reopen
              </Button>
            )}
          </div>
          <div className="flex items-center gap-1">
            {item.status === "want" && (
              <button onClick={() => onStatus("done")} aria-label={`Mark "${item.title}" as done`} title="Mark done" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
                <Check size={15} />
              </button>
            )}
            <button onClick={onEdit} aria-label={`Edit "${item.title}"`} title="Edit" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
              <Pencil size={14} />
            </button>
            <button onClick={onDelete} aria-label={`Delete "${item.title}"`} title="Delete" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
              <Trash2 size={14} />
            </button>
          </div>
        </div>
      </div>
    </article>
  );
}

export function BucketPage() {
  const { data: items, isLoading, error } = useBucket();
  const [text, setText] = useState("");
  const [status, setStatus] = useState<BucketStatus | null>(null);
  const [category, setCategory] = useState<string | null>(null);
  const [editing, setEditing] = useState<{ item: BucketItem | null; status?: BucketStatus } | null>(null);
  const [celebrate, setCelebrate] = useState<string | null>(null);
  const [removing, setRemoving] = useState<BucketItem | null>(null);

  const all = useMemo(() => items ?? [], [items]);
  const categories = useMemo(() => categoriesOf(all), [all]);
  const shown = useMemo(() => filterItems(all, { text, status, category }), [all, text, status, category]);
  const doneCount = all.filter((i) => i.status === "done").length;
  const filtering = !!(text.trim() || status || category);
  const celebrating = celebrate ? all.find((i) => i.id === celebrate) : undefined;

  const move = async (item: BucketItem, to: BucketStatus) => {
    try {
      await api.bucketSetStatus(item.id, to);
      invalidateBucket();
      if (to === "done") setCelebrate(item.id);
    } catch (e) {
      toastError(String(e));
    }
  };

  const remove = async (item: BucketItem) => {
    setRemoving(null);
    try {
      await api.bucketDelete(item.id);
      invalidateBucket();
      toast("Removed from your list", {
        label: "Undo",
        run: () =>
          api
            .bucketRestore(item.id)
            .then(invalidateBucket)
            .catch((e) => toastError(String(e))),
      });
    } catch (e) {
      toastError(String(e));
    }
  };

  const chip = (active: boolean) =>
    `inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-[11.5px] font-semibold transition-colors ${
      active ? "bg-accent text-on-accent" : "border border-line bg-paper text-muted hover:border-accent hover:text-accent-deep"
    }`;
  const visible = COLUMNS.filter((c) => !status || c.status === status);

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="eyebrow">Dream a little bigger</div>
          <h1 className="page-title mt-2">Bucket list</h1>
          <p className="mt-2 text-[13px] text-muted">Everything you want to do together, and everything you already have.</p>
        </div>
        <div className="flex items-center gap-4">
          {all.length > 0 && (
            <div className="flex items-center gap-3">
              <ProgressRing done={doneCount} total={all.length} />
              <div>
                <div className="font-serif text-[17px] leading-tight">{progressLabel(doneCount, all.length)}</div>
                <div className="text-[11px] text-muted">{all.length - doneCount} still to go</div>
              </div>
            </div>
          )}
          <Button icon={<Plus size={15} />} onClick={() => setEditing({ item: null })}>
            New dream
          </Button>
        </div>
      </div>

      {all.length > 0 && (
        <div className="mb-5 flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-[240px] flex-1">
              <Search size={15} className="absolute top-1/2 left-3 -translate-y-1/2 text-faint" />
              <input value={text} onChange={(e) => setText(e.target.value)} placeholder="Search your dreams…" aria-label="Search the bucket list" className="field-input pl-9!" />
            </div>
            <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter by status">
              <button className={chip(!status)} aria-pressed={!status} onClick={() => setStatus(null)}>
                All
              </button>
              {COLUMNS.map((c) => (
                <button key={c.status} className={chip(status === c.status)} aria-pressed={status === c.status} onClick={() => setStatus(status === c.status ? null : c.status)}>
                  {c.title}
                </button>
              ))}
            </div>
          </div>
          {categories.length > 0 && (
            <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label="Filter by category">
              {categories.map((c) => (
                <button key={c} className={chip(category?.toLowerCase() === c.toLowerCase())} aria-pressed={category?.toLowerCase() === c.toLowerCase()} onClick={() => setCategory(category?.toLowerCase() === c.toLowerCase() ? null : c)}>
                  {c}
                </button>
              ))}
              {filtering && (
                <button
                  onClick={() => {
                    setText("");
                    setStatus(null);
                    setCategory(null);
                  }}
                  className="ml-1 inline-flex items-center gap-1 text-[11px] text-muted hover:text-ink"
                >
                  <X size={12} /> Clear filters
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {error ? (
        <div className="card">
          <EmptyState title="Couldn't load your bucket list">{String(error)}</EmptyState>
        </div>
      ) : isLoading ? null : all.length === 0 ? (
        <div className="card">
          <EmptyState
            icon={<ListChecks size={22} />}
            title="What do you want to do together?"
            action={
              <Button icon={<Plus size={15} />} onClick={() => setEditing({ item: null })}>
                Add your first dream
              </Button>
            }
          >
            A trip, a recipe, a silly challenge. Write it down once, and cross it off together.
          </EmptyState>
        </div>
      ) : (
        <div className={`grid gap-5 ${visible.length === 1 ? "" : "lg:grid-cols-3"}`}>
          {visible.map((col) => {
            const list = sortColumn(shown.filter((i) => i.status === col.status), col.status);
            const total = all.filter((i) => i.status === col.status).length;
            return (
              <section key={col.status} aria-label={col.title} className="min-w-0">
                <div className="mb-3 flex items-center justify-between gap-2">
                  <div>
                    <h2 className="section-title flex items-center gap-2">
                      <span className={`inline-block h-2.5 w-2.5 rounded-full ${col.tone}`} aria-hidden="true" /> {col.title}
                      <span className="text-[12px] font-normal text-faint">{filtering ? `${list.length} of ${total}` : total}</span>
                    </h2>
                    <p className="mt-0.5 text-[11.5px] text-muted">{col.hint}</p>
                  </div>
                  <button onClick={() => setEditing({ item: null, status: col.status })} aria-label={`Add to ${col.title}`} title={`Add to ${col.title}`} className="grid h-8 w-8 place-items-center rounded-lg border border-line text-muted hover:border-accent hover:text-accent-deep">
                    <Plus size={14} />
                  </button>
                </div>
                <div className="flex flex-col gap-3.5">
                  {list.map((it) => (
                    <Card key={it.id} item={it} onEdit={() => setEditing({ item: it })} onDelete={() => setRemoving(it)} onStatus={(s) => move(it, s)} />
                  ))}
                  {list.length === 0 && (
                    <div className="rounded-2xl border border-dashed border-line px-4 py-8 text-center text-[12px] text-muted">
                      {filtering ? "Nothing matches here." : col.status === "done" ? "Nothing crossed off yet. Soon ♡" : "Nothing here yet."}
                    </div>
                  )}
                </div>
              </section>
            );
          })}
        </div>
      )}

      {editing && (
        <BucketEditor
          key={editing.item?.id ?? "new"}
          item={editing.item}
          categories={categories}
          defaultStatus={editing.status}
          onClose={() => setEditing(null)}
          onSaved={(id, becameDone) => {
            setEditing(null);
            if (becameDone) setCelebrate(id);
          }}
        />
      )}
      {celebrating && <CompleteDialog key={celebrating.id} item={celebrating} onClose={() => setCelebrate(null)} />}
      <ConfirmDialog
        open={!!removing}
        title="Remove this dream?"
        message={`"${removing?.title ?? ""}" will leave your list. You can undo right after.`}
        confirmLabel="Remove"
        danger
        onConfirm={() => removing && remove(removing)}
        onCancel={() => setRemoving(null)}
      />
    </div>
  );
}
