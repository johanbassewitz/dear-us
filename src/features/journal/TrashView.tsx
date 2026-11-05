import { useState } from "react";
import { ArrowLeft, RotateCcw, Trash2 } from "lucide-react";
import { api, type TrashItem } from "../../lib/api";
import { invalidateJournal, useTrash } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { daysUntilPurge, fmtDate } from "../../lib/format";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { MediaThumb } from "../../components/media/MediaThumb";
import { toast, toastError } from "../../components/ui/Toast";

export function TrashView() {
  const open = useNav((s) => s.openJournal);
  const { data: items } = useTrash();
  const [forever, setForever] = useState<TrashItem | null>(null);

  const restore = async (t: TrashItem) => {
    try {
      await api.journalRestore(t.id);
      invalidateJournal();
      toast(`“${t.title}” is back ♡`, { label: "Open", run: () => open({ mode: "view", id: t.id }) });
    } catch (e) {
      toastError(String(e));
    }
  };
  const purge = async () => {
    const t = forever!;
    setForever(null);
    try {
      await api.journalPurge(t.id);
      invalidateJournal();
      toast("Removed for good");
    } catch (e) {
      toastError(String(e));
    }
  };

  return (
    <div className="animate-fade-up mx-auto max-w-[860px]">
      <button onClick={() => open({ mode: "list" })} className="mb-5 inline-flex items-center gap-1.5 text-[12.5px] font-semibold text-muted hover:text-ink">
        <ArrowLeft size={15} /> All memories
      </button>
      <div className="eyebrow">Nothing is lost by accident</div>
      <h1 className="page-title mt-2">Recovery bin</h1>
      <p className="mt-2 mb-6 text-[13px] text-muted">Deleted memories wait here for 30 days, then they're removed for good.</p>

      <div className="card divide-y divide-line-soft">
        {!items?.length ? (
          <EmptyState icon={<Trash2 size={22} />} title="The bin is empty">
            Nothing deleted recently.
          </EmptyState>
        ) : (
          items.map((t) => {
            const days = daysUntilPurge(t.deletedAt);
            return (
              <div key={t.id} className="flex items-center gap-4 p-4">
                <div className="h-14 w-14 shrink-0 overflow-hidden rounded-lg">
                  <MediaThumb media={t.cover} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate font-serif text-[17px]">{t.title}</div>
                  <div className="text-[11.5px] text-muted">
                    {fmtDate(t.entryDate)} · removed for good in {days} day{days === 1 ? "" : "s"}
                  </div>
                </div>
                <Button variant="secondary" size="sm" icon={<RotateCcw size={13} />} onClick={() => restore(t)}>
                  Restore
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setForever(t)} className="text-[#b0413e]! hover:text-[#93322f]!">
                  Delete forever
                </Button>
              </div>
            );
          })
        )}
      </div>

      <ConfirmDialog
        open={!!forever}
        title="Delete this memory forever?"
        message={`“${forever?.title ?? ""}” will be permanently removed from this PC. This can't be undone.`}
        confirmLabel="Delete forever"
        danger
        onConfirm={purge}
        onCancel={() => setForever(null)}
      />
    </div>
  );
}
