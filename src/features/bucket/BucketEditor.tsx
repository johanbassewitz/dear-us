import { useCallback, useState } from "react";
import { ImagePlus, Star, X } from "lucide-react";
import type { BucketInput, BucketItem, BucketPriority, BucketStatus, MediaRef } from "../../lib/api";
import { useSaveBucketItem } from "../../lib/queries";
import { useMediaImport } from "../../lib/useMediaImport";
import { Modal } from "../../components/ui/Modal";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { MediaThumb } from "../../components/media/MediaThumb";
import { toast } from "../../components/ui/Toast";
import { CATEGORY_SUGGESTIONS } from "./bucketUtils";

interface Props {
  /** null = a new dream */
  item: BucketItem | null;
  categories: string[];
  defaultStatus?: BucketStatus;
  onClose: () => void;
  /** Called after a successful save with the saved id and whether it just became "done". */
  onSaved: (id: string, becameDone: boolean) => void;
}

export function BucketEditor({ item, categories, defaultStatus = "want", onClose, onSaved }: Props) {
  const save = useSaveBucketItem();
  const [title, setTitle] = useState(item?.title ?? "");
  const [description, setDescription] = useState(item?.description ?? "");
  const [category, setCategory] = useState(item?.category ?? "");
  const [priority, setPriority] = useState<BucketPriority>(item?.priority ?? "medium");
  const [status, setStatus] = useState<BucketStatus>(item?.status ?? defaultStatus);
  const [targetDate, setTargetDate] = useState(item?.targetDate ?? "");
  const [media, setMedia] = useState<MediaRef[]>(item?.media ?? []);
  const [cover, setCover] = useState<string | null>(item?.coverMediaId ?? null);
  const [error, setError] = useState<string | null>(null);

  const onAdded = useCallback((added: MediaRef[]) => {
    setMedia((m) => [...m, ...added.filter((a) => !m.some((x) => x.id === a.id))]);
  }, []);
  const { pick, busy } = useMediaImport(onAdded, { dropEnabled: false });

  const suggestions = [...new Set([...CATEGORY_SUGGESTIONS, ...categories])];
  const coverId = cover && media.some((m) => m.id === cover) ? cover : (media[0]?.id ?? null);

  const submit = async () => {
    setError(null);
    if (!title.trim()) return setError("Give this dream a title ♡");
    const input: BucketInput = {
      title,
      description,
      category,
      priority,
      status,
      targetDate: targetDate || null,
      coverMediaId: coverId,
      mediaIds: media.map((m) => m.id),
    };
    try {
      const id = await save.mutateAsync({ id: item?.id ?? null, input });
      toast(item ? "Changes saved ♡" : "Added to your bucket list ♡");
      onSaved(id, status === "done" && item?.status !== "done");
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={item ? "Edit this dream" : "A new dream"}
      description="Something you'd love to do together."
      width={580}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={save.isPending || busy > 0}>
            {save.isPending ? "Saving…" : item ? "Save changes" : "Add to our list"}
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        {error && <div className="rounded-xl border border-[#e7b9b4] bg-[#fbeceb] px-4 py-3 text-[12.5px] text-[#93322f]">{error}</div>}
        <Field label="What do you dream of?" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} placeholder="Watch the sunrise from a mountain…" autoFocus />
        <div className="flex flex-col gap-1.5">
          <label htmlFor="bucket-desc" className="text-[11px] font-bold text-ink-soft">
            Notes
          </label>
          <textarea id="bucket-desc" className="field-input min-h-[72px] resize-y" value={description} onChange={(e) => setDescription(e.target.value)} maxLength={2000} placeholder="Where, who with, what you'd bring…" />
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-1.5">
            <label htmlFor="bucket-category" className="text-[11px] font-bold text-ink-soft">
              Category
            </label>
            <input id="bucket-category" className="field-input" list="bucket-categories" value={category} onChange={(e) => setCategory(e.target.value)} maxLength={40} placeholder="Travel, Food, At home…" />
            <datalist id="bucket-categories">
              {suggestions.map((c) => (
                <option key={c} value={c} />
              ))}
            </datalist>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="bucket-priority" className="text-[11px] font-bold text-ink-soft">
              Priority
            </label>
            <select id="bucket-priority" className="field-input" value={priority} onChange={(e) => setPriority(e.target.value as BucketPriority)}>
              <option value="high">High</option>
              <option value="medium">Medium</option>
              <option value="low">Low</option>
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="bucket-status" className="text-[11px] font-bold text-ink-soft">
              Where is it?
            </label>
            <select id="bucket-status" className="field-input" value={status} onChange={(e) => setStatus(e.target.value as BucketStatus)}>
              <option value="want">Dreaming of</option>
              <option value="planned">Making plans</option>
              <option value="done">Done together</option>
            </select>
          </div>
          <Field label="Target date" type="date" value={targetDate} onChange={(e) => setTargetDate(e.target.value)} hint="Optional" />
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-[11px] font-bold text-ink-soft">Photos</span>
          {media.length > 0 && (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(110px,1fr))] gap-2.5">
              {media.map((m) => (
                <div key={m.id} className="group relative aspect-[4/3] overflow-hidden rounded-xl border border-line bg-paper-2">
                  <MediaThumb media={m} />
                  <div className="absolute inset-x-0 top-0 flex justify-between p-1 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                    <button type="button" onClick={() => setCover(m.id)} aria-label="Use as cover" title="Use as cover" className={`grid h-6 w-6 place-items-center rounded-full ${coverId === m.id ? "bg-accent text-on-accent" : "bg-black/45 text-white"}`}>
                      <Star size={12} className={coverId === m.id ? "fill-current" : ""} />
                    </button>
                    <button type="button" onClick={() => setMedia((x) => x.filter((y) => y.id !== m.id))} aria-label="Remove photo" className="grid h-6 w-6 place-items-center rounded-full bg-black/45 text-white">
                      <X size={12} />
                    </button>
                  </div>
                  {coverId === m.id && <span className="absolute bottom-1 left-1 rounded-full bg-accent px-1.5 py-0.5 text-[9px] font-bold text-on-accent">cover</span>}
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
        <button type="submit" className="hidden" aria-hidden="true" tabIndex={-1} />
      </form>
    </Modal>
  );
}
