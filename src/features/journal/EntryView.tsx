import { useState } from "react";
import { ArrowLeft, Clock, MapPin, Pencil, Star, Trash2, UserRound } from "lucide-react";
import { api } from "../../lib/api";
import { invalidateJournal, useEntry } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { fmtDate, fmtTime } from "../../lib/format";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { MediaViewer } from "../../components/media/MediaViewer";
import { RichTextView } from "../../components/richtext/RichText";
import { toast, toastError } from "../../components/ui/Toast";
import { Comments, ReactionBar } from "./Engagement";
import { ScrapMedia } from "./Scrapbook";
import { paperClass, resolveLayout } from "./scrapbookStyles";

export function EntryView({ id }: { id: string }) {
  const open = useNav((s) => s.openJournal);
  const { data: e, isLoading, error } = useEntry(id);
  const [viewer, setViewer] = useState<number | null>(null);
  const [confirm, setConfirm] = useState(false);

  const back = (
    <button onClick={() => open({ mode: "list" })} className="mb-5 inline-flex items-center gap-1.5 text-[12.5px] font-semibold text-muted hover:text-ink">
      <ArrowLeft size={15} /> All memories
    </button>
  );

  if (isLoading) return back;
  if (error || !e)
    return (
      <div>
        {back}
        <div className="card">
          <EmptyState title="This memory isn't here anymore">It may have been moved to the recovery bin.</EmptyState>
        </div>
      </div>
    );

  const remove = async () => {
    setConfirm(false);
    try {
      await api.journalDelete(e.id);
      invalidateJournal();
      open({ mode: "list" });
      toast("Moved to the recovery bin", {
        label: "Undo",
        run: () =>
          api
            .journalRestore(e.id)
            .then(() => invalidateJournal())
            .catch((err) => toastError(String(err))),
      });
    } catch (err) {
      toastError(String(err));
    }
  };

  const layout = resolveLayout(e.layout, e.media.length);
  const mediaEl = <ScrapMedia media={e.media} layout={layout} frame={e.frame} onOpen={setViewer} />;
  const topMedia = e.media.length > 0 && (layout === "magazine" || layout === "collage");

  return (
    <div className="animate-fade-up mx-auto max-w-[980px]">
      {back}
      <article className={`card overflow-hidden ${paperClass(e.paper)}`}>
        {topMedia && <div className="bg-paper-2 p-2">{mediaEl}</div>}

        <div className="px-9 pt-8 pb-9">
          <div className={`flex flex-wrap items-start justify-between gap-4 ${layout === "polaroid" ? "text-center [&>div:first-child]:mx-auto" : ""}`}>
            <div className="min-w-0">
              <div className="eyebrow flex items-center gap-2">
                {fmtDate(e.entryDate)}
                {e.highlight && (
                  <span className="inline-flex items-center gap-1 rounded-full bg-accent-soft px-2 py-0.5 tracking-normal normal-case">
                    <Star size={11} className="fill-current" /> milestone
                  </span>
                )}
              </div>
              <h1 className="page-title mt-2">{e.title}</h1>
              <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px] text-muted">
                {e.entryTime && (
                  <span className="inline-flex items-center gap-1.5">
                    <Clock size={13} /> {fmtTime(e.entryTime)}
                  </span>
                )}
                {e.location && (
                  <span className="inline-flex items-center gap-1.5">
                    <MapPin size={13} /> {e.location}
                  </span>
                )}
                {e.authorName && (
                  <span className="inline-flex items-center gap-1.5">
                    <UserRound size={13} /> written by {e.authorName}
                  </span>
                )}
              </div>
            </div>
            <div className="flex gap-2">
              <Button variant="secondary" size="sm" icon={<Pencil size={13} />} onClick={() => open({ mode: "edit", id: e.id })}>
                Edit
              </Button>
              <Button variant="secondary" size="sm" icon={<Trash2 size={13} />} onClick={() => setConfirm(true)} aria-label="Delete memory">
                Delete
              </Button>
            </div>
          </div>

          {layout === "polaroid" && e.media.length > 0 && <div className="-mx-4 mt-4">{mediaEl}</div>}

          {e.bodyJson ? (
            <div className="mt-7">
              <RichTextView json={e.bodyJson} />
            </div>
          ) : null}

          {!!e.tags.length && (
            <div className="mt-7 flex flex-wrap gap-1.5">
              {e.tags.map((t) => (
                <span key={t} className="rounded-full bg-accent-soft px-2.5 py-1 text-[11px] font-semibold text-accent-deep">
                  #{t}
                </span>
              ))}
            </div>
          )}
          {layout === "minimal" && e.media.length > 0 && <div className="mt-8">{mediaEl}</div>}

          <div className="mt-8">
            <ReactionBar entryId={e.id} />
          </div>
          <Comments entryId={e.id} />
        </div>
      </article>

      <MediaViewer items={e.media} index={viewer} onClose={() => setViewer(null)} />
      <ConfirmDialog
        open={confirm}
        title="Move this memory to the recovery bin?"
        message="You can restore it from the recovery bin for 30 days. After that it's removed for good."
        confirmLabel="Move to recovery bin"
        danger
        onConfirm={remove}
        onCancel={() => setConfirm(false)}
      />
    </div>
  );
}
