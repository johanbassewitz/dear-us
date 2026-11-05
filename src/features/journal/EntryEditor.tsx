import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeft, ChevronLeft, ChevronRight, ImagePlus, Loader, Star, Upload, X } from "lucide-react";
import type { EntryDetail, EntryInput, MediaRef, ScrapFrame, ScrapLayout, ScrapPaper } from "../../lib/api";
import { useCapabilities, useEntry, useSaveEntry, useTags } from "../../lib/queries";
import { useNav, type EditPrefill } from "../../app/navigation";
import { linkNewEntry } from "./entryLinks";
import { localIsoDate } from "../../lib/dates";
import { useMediaImport } from "../../lib/useMediaImport";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { TagInput } from "../../components/ui/TagInput";
import { MediaThumb } from "../../components/media/MediaThumb";
import { RichTextEditor } from "../../components/richtext/RichText";
import { toast } from "../../components/ui/Toast";
import { StylePanel } from "./Scrapbook";

interface Item {
  media: MediaRef;
  caption: string;
}

interface Draft {
  title: string;
  bodyJson: string;
  entryDate: string;
  entryTime: string;
  location: string;
  tags: string[];
  items: Item[];
  cover: string | null;
  highlight: boolean;
  layout: ScrapLayout;
  frame: ScrapFrame;
  paper: ScrapPaper;
}

function fromEntry(e: EntryDetail | undefined, prefill?: EditPrefill): Draft {
  const prefillDate = prefill?.entryDate;
  return {
    title: e?.title ?? prefill?.title ?? "",
    bodyJson: e?.bodyJson ?? "",
    entryDate: e?.entryDate ?? prefillDate ?? localIsoDate(),
    entryTime: e?.entryTime ?? "",
    location: e?.location ?? "",
    tags: e?.tags ?? [],
    items: e?.media.map(({ caption, ...m }) => ({ media: m, caption: caption ?? "" })) ?? [],
    cover: e?.coverMediaId ?? null,
    highlight: e?.highlight ?? false,
    layout: e?.layout ?? "auto",
    frame: e?.frame ?? "none",
    paper: e?.paper ?? "none",
  };
}

function toInput(d: Draft): EntryInput {
  return {
    title: d.title,
    bodyJson: d.bodyJson,
    entryDate: d.entryDate,
    entryTime: d.entryTime || null,
    location: d.location || null,
    tags: d.tags,
    media: d.items.map((i) => ({ id: i.media.id, caption: i.caption || null })),
    coverMediaId: d.cover,
    highlight: d.highlight,
    layout: d.layout,
    frame: d.frame,
    paper: d.paper,
  };
}

export function EntryEditor({ id, prefill }: { id: string | null; prefill?: EditPrefill }) {
  const { data: existing, isLoading } = useEntry(id);
  if (id && isLoading) return null;
  if (id && !existing) return <p className="text-muted">This memory isn't here anymore.</p>;
  return <EditorForm key={id ?? "new"} id={id} existing={existing} prefill={prefill} />;
}

function EditorForm({ id, existing, prefill }: { id: string | null; existing?: EntryDetail; prefill?: EditPrefill }) {
  const prefillDate = prefill?.entryDate;
  const open = useNav((s) => s.openJournal);
  const setBlocker = useNav((s) => s.setBlocker);
  const save = useSaveEntry();
  const { data: tagList } = useTags();
  const { data: caps } = useCapabilities();
  const initial = useMemo(() => fromEntry(existing, prefill), [existing, prefill]);
  const [d, setD] = useState<Draft>(initial);
  const [error, setError] = useState<string | null>(null);
  const dateTouched = useRef(!!id || !!prefillDate);

  const dirty = JSON.stringify(toInput(d)) !== JSON.stringify(toInput(initial));
  useEffect(() => {
    setBlocker(dirty ? "You have unsaved changes to this memory." : null);
    return () => setBlocker(null);
  }, [dirty, setBlocker]);

  const set = <K extends keyof Draft>(k: K, v: Draft[K]) => setD((x) => ({ ...x, [k]: v }));

  const onAdded = useCallback((media: MediaRef[]) => {
    setD((x) => {
      const have = new Set(x.items.map((i) => i.media.id));
      const items = [...x.items, ...media.filter((m) => !have.has(m.id)).map((m) => ({ media: m, caption: "" }))];
      // New memory: use the first photo's date if the date wasn't chosen by hand.
      const taken = media.find((m) => m.takenAt)?.takenAt?.slice(0, 10);
      const entryDate = !dateTouched.current && taken ? taken : x.entryDate;
      return { ...x, items, entryDate };
    });
  }, []);
  const { pick, busy, dragging } = useMediaImport(onAdded);

  const move = (i: number, dir: -1 | 1) =>
    setD((x) => {
      const items = [...x.items];
      const j = i + dir;
      if (j < 0 || j >= items.length) return x;
      [items[i], items[j]] = [items[j], items[i]];
      return { ...x, items };
    });
  const removeItem = (mid: string) => setD((x) => ({ ...x, items: x.items.filter((i) => i.media.id !== mid), cover: x.cover === mid ? null : x.cover }));
  const coverId = d.cover ?? d.items[0]?.media.id ?? null;

  const submit = async () => {
    setError(null);
    if (!d.title.trim()) return setError("Give this memory a title ♡");
    try {
      const saved = await save.mutateAsync({ id, input: toInput(d) });
      setBlocker(null);
      // A new memory written "for" something (a bucket-list dream, a date) gets linked back to it.
      let linked = false;
      if (!id && prefill?.linkTo) linked = await linkNewEntry(prefill.linkTo, saved.id);
      open({ mode: "view", id: saved.id });
      toast(id ? "Changes saved ♡" : linked ? "Memory saved and linked ♡" : "Memory saved on this PC ♡");
    } catch (e) {
      setError(String(e));
    }
  };

  const cancel = () => open(id ? { mode: "view", id } : { mode: "list" });
  const hasHeicOrHevc = d.items.some((i) => i.media.preview === "failed");

  return (
    <div className="animate-fade-up">
      <div className="mb-5 flex items-center justify-between gap-4">
        <button onClick={cancel} className="inline-flex items-center gap-1.5 text-[12.5px] font-semibold text-muted hover:text-ink">
          <ArrowLeft size={15} /> {id ? "Back to memory" : "All memories"}
        </button>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={cancel}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={save.isPending || busy > 0}>
            {save.isPending ? "Saving…" : id ? "Save changes" : "Save memory"}
          </Button>
        </div>
      </div>
      {error && <div className="mb-4 rounded-xl border border-[#e7b9b4] bg-[#fbeceb] px-4 py-3 text-[12.5px] text-[#93322f]">{error}</div>}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
        <div className="min-w-0">
          <input
            value={d.title}
            onChange={(e) => set("title", e.target.value)}
            placeholder="What happened?"
            aria-label="Title"
            maxLength={200}
            autoFocus={!id}
            className="mb-5 w-full bg-transparent font-serif text-[34px] leading-tight tracking-[-0.02em] text-ink outline-none placeholder:text-faint"
          />

          {/* photos & videos */}
          <section
            className={`relative mb-5 rounded-2xl border-2 border-dashed p-4 transition-colors ${dragging ? "border-accent bg-accent-soft" : "border-line bg-paper"}`}
            aria-label="Photos and videos"
          >
            {d.items.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-6 text-center">
                <ImagePlus size={26} className="text-accent" />
                <p className="text-[13px] text-ink-soft">Drop photos and videos here</p>
                <p className="text-[11.5px] text-muted">Originals are kept in full quality on this PC.</p>
                <Button variant="secondary" size="sm" icon={<Upload size={13} />} onClick={pick} disabled={busy > 0}>
                  Choose files
                </Button>
              </div>
            ) : (
              <>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-3">
                  {d.items.map((it, i) => (
                    <div key={it.media.id} className="group overflow-hidden rounded-xl border border-line bg-paper-2">
                      <div className="relative aspect-[4/3]">
                        <MediaThumb media={it.media} />
                        <div className="absolute inset-x-0 top-0 flex justify-between p-1.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                          <button
                            type="button"
                            onClick={() => set("cover", it.media.id)}
                            title="Use as cover"
                            aria-label="Use as cover"
                            className={`grid h-7 w-7 place-items-center rounded-full ${coverId === it.media.id ? "bg-accent text-on-accent" : "bg-black/45 text-white"}`}
                          >
                            <Star size={13} className={coverId === it.media.id ? "fill-current" : ""} />
                          </button>
                          <button type="button" onClick={() => removeItem(it.media.id)} aria-label="Remove from memory" className="grid h-7 w-7 place-items-center rounded-full bg-black/45 text-white">
                            <X size={13} />
                          </button>
                        </div>
                        {coverId === it.media.id && (
                          <span className="absolute bottom-1.5 left-1.5 rounded-full bg-accent px-2 py-0.5 text-[9.5px] font-bold text-on-accent">cover</span>
                        )}
                        <div className="absolute inset-x-0 bottom-0 flex justify-end gap-1 p-1.5 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
                          <button type="button" onClick={() => move(i, -1)} disabled={i === 0} aria-label="Move earlier" className="grid h-6 w-6 place-items-center rounded-full bg-black/45 text-white disabled:opacity-30">
                            <ChevronLeft size={13} />
                          </button>
                          <button type="button" onClick={() => move(i, 1)} disabled={i === d.items.length - 1} aria-label="Move later" className="grid h-6 w-6 place-items-center rounded-full bg-black/45 text-white disabled:opacity-30">
                            <ChevronRight size={13} />
                          </button>
                        </div>
                      </div>
                      <input
                        value={it.caption}
                        onChange={(e) => setD((x) => ({ ...x, items: x.items.map((y) => (y.media.id === it.media.id ? { ...y, caption: e.target.value } : y)) }))}
                        placeholder="Caption…"
                        aria-label="Caption"
                        maxLength={500}
                        className="w-full bg-transparent px-2.5 py-2 font-hand text-[16px] text-ink-soft outline-none placeholder:text-faint"
                      />
                    </div>
                  ))}
                  <button type="button" onClick={pick} disabled={busy > 0} className="flex aspect-[4/3] flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line text-[12px] text-muted hover:border-accent hover:text-accent">
                    <ImagePlus size={20} /> Add more
                  </button>
                </div>
                <p className="mt-3 text-[11px] text-faint">Hover a photo to set the cover ★, reorder or remove it. Removing only takes it out of this memory.</p>
              </>
            )}
            {busy > 0 && (
              <div className="absolute inset-0 grid place-items-center rounded-2xl bg-paper/80 backdrop-blur-[1px]">
                <span className="inline-flex items-center gap-2 text-[13px] text-ink-soft">
                  <Loader size={16} className="animate-spin text-accent" /> Adding {busy} file{busy > 1 ? "s" : ""} to your archive…
                </span>
              </div>
            )}
          </section>
          {hasHeicOrHevc && !caps?.converter && (
            <p className="-mt-2 mb-4 text-[11.5px] text-muted">
              Some files (like iPhone HEIC photos) can't be previewed on this PC yet. They're saved safely in full quality. To show them, go to Settings &amp; Sync → Photo &amp; video converter → Download converter.
            </p>
          )}

          <RichTextEditor initialJson={initial.bodyJson} onChange={(j) => set("bodyJson", j)} />
        </div>

        <aside className="card flex h-fit flex-col gap-4 p-5">
          <Field
            label="Date"
            type="date"
            value={d.entryDate}
            onChange={(e) => {
              dateTouched.current = true;
              set("entryDate", e.target.value);
            }}
            required
          />
          <Field label="Time" type="time" value={d.entryTime} onChange={(e) => set("entryTime", e.target.value)} hint="Optional" />
          <Field label="Where" value={d.location} onChange={(e) => set("location", e.target.value)} placeholder="A café, a city, our couch…" maxLength={200} />
          <div className="flex flex-col gap-1.5">
            <label htmlFor="tags" className="text-[11px] font-bold text-ink-soft">
              Tags
            </label>
            <TagInput id="tags" tags={d.tags} onChange={(t) => set("tags", t)} suggestions={tagList?.map((t) => t.tag)} />
          </div>
          <label className="flex cursor-pointer items-start gap-3 rounded-xl bg-paper-2 p-3">
            <input type="checkbox" checked={d.highlight} onChange={(e) => set("highlight", e.target.checked)} className="mt-0.5 h-4 w-4 accent-[var(--accent)]" />
            <span>
              <strong className="block text-[12.5px]">Mark as a milestone ★</strong>
              <span className="text-[11.5px] text-muted">Shows up in the timeline's milestones view.</span>
            </span>
          </label>
          <StylePanel
            layout={d.layout}
            frame={d.frame}
            paper={d.paper}
            mediaCount={d.items.length}
            onChange={(patch) => setD((x) => ({ ...x, ...patch }))}
          />
        </aside>
      </div>
    </div>
  );
}
