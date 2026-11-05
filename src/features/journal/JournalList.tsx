import { useEffect, useMemo, useState } from "react";
import { ArrowDownUp, BookHeart, Grid2x2, LayoutList, Plus, Search, Star, Trash2, X } from "lucide-react";
import { useEntries, useTags, useTrash } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import type { EntryQuery } from "../../lib/api";
import { Button } from "../../components/ui/Button";
import { EmptyState } from "../../components/ui/EmptyState";
import { usePrefs } from "../../lib/settings";
import { EntryCard, EntryTile } from "./EntryCard";

function useDebounced<T>(value: T, ms = 250): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

export function JournalList() {
  const open = useNav((s) => s.openJournal);
  const view = usePrefs((s) => s.prefs["journal.view"]);
  const setPref = usePrefs((s) => s.set);
  const [text, setText] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [oldestFirst, setOldestFirst] = useState(false);
  const [highlightsOnly, setHighlightsOnly] = useState(false);
  const debounced = useDebounced(text);
  const [limit, setLimit] = useState(120);

  const query: EntryQuery = useMemo(
    () => ({ text: debounced || undefined, tag: tag ?? undefined, from: from || undefined, to: to || undefined, oldestFirst, highlightsOnly, limit }),
    [debounced, tag, from, to, oldestFirst, highlightsOnly, limit],
  );
  const { data: entries, isLoading, error } = useEntries(query);
  const { data: tags } = useTags();
  const { data: trash } = useTrash();
  const filtering = !!(debounced || tag || from || to || highlightsOnly);

  const clear = () => {
    setText("");
    setTag(null);
    setFrom("");
    setTo("");
    setHighlightsOnly(false);
  };

  const chip = (active: boolean) =>
    `inline-flex items-center gap-1.5 rounded-[10px] border px-3 py-2 text-[11.5px] font-semibold transition-colors ${
      active ? "border-accent bg-accent text-on-accent" : "border-line bg-paper text-muted hover:border-accent hover:text-accent-deep"
    }`;

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="eyebrow">Keep the small things</div>
          <h1 className="page-title mt-2">Journal</h1>
          <p className="mt-2 text-[13px] text-muted">
            {entries && !filtering ? `${entries.length} ${entries.length === 1 ? "memory" : "memories"} saved` : "Every moment worth remembering."}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" icon={<Trash2 size={14} />} onClick={() => open({ mode: "trash" })}>
            Recovery bin{trash?.length ? ` (${trash.length})` : ""}
          </Button>
          <Button icon={<Plus size={15} />} onClick={() => open({ mode: "edit", id: null })}>
            New memory
          </Button>
        </div>
      </div>

      {/* filters */}
      <div className="mb-5 flex flex-wrap items-center gap-2">
        <div className="relative min-w-[240px] flex-1">
          <Search size={15} className="absolute top-1/2 left-3 -translate-y-1/2 text-faint" />
          <input
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Search titles, words, places, tags…"
            aria-label="Search memories"
            className="field-input pl-9!"
          />
        </div>
        <label className="flex items-center gap-1.5 text-[11.5px] text-muted">
          From
          <input type="date" value={from} max={to || undefined} onChange={(e) => setFrom(e.target.value)} className="field-input w-auto! py-2!" />
        </label>
        <label className="flex items-center gap-1.5 text-[11.5px] text-muted">
          to
          <input type="date" value={to} min={from || undefined} onChange={(e) => setTo(e.target.value)} className="field-input w-auto! py-2!" />
        </label>
        <button className={chip(highlightsOnly)} onClick={() => setHighlightsOnly(!highlightsOnly)} aria-pressed={highlightsOnly}>
          <Star size={13} /> Milestones
        </button>
        <button className={chip(false)} onClick={() => setOldestFirst(!oldestFirst)} title="Change order">
          <ArrowDownUp size={13} /> {oldestFirst ? "Oldest first" : "Newest first"}
        </button>
        <div className="flex overflow-hidden rounded-[10px] border border-line" role="group" aria-label="Layout">
          <button aria-label="Feed" aria-pressed={view === "feed"} onClick={() => setPref("journal.view", "feed")} className={`px-2.5 py-2 ${view === "feed" ? "bg-accent-soft text-accent-deep" : "bg-paper text-muted"}`}>
            <LayoutList size={15} />
          </button>
          <button aria-label="Gallery" aria-pressed={view === "gallery"} onClick={() => setPref("journal.view", "gallery")} className={`px-2.5 py-2 ${view === "gallery" ? "bg-accent-soft text-accent-deep" : "bg-paper text-muted"}`}>
            <Grid2x2 size={15} />
          </button>
        </div>
      </div>
      {!!tags?.length && (
        <div className="mb-6 flex flex-wrap items-center gap-1.5">
          {tags.slice(0, 16).map((t) => (
            <button
              key={t.tag}
              onClick={() => setTag(tag === t.tag ? null : t.tag)}
              aria-pressed={tag === t.tag}
              className={`rounded-full px-2.5 py-1 text-[11px] font-semibold transition-colors ${tag === t.tag ? "bg-accent text-on-accent" : "bg-accent-soft text-accent-deep hover:bg-accent/20"}`}
            >
              #{t.tag} <span className="opacity-60">{t.count}</span>
            </button>
          ))}
          {filtering && (
            <button onClick={clear} className="ml-1 inline-flex items-center gap-1 text-[11px] text-muted hover:text-ink">
              <X size={12} /> Clear filters
            </button>
          )}
        </div>
      )}

      {error ? (
        <div className="card">
          <EmptyState title="Couldn't load memories">{String(error)}</EmptyState>
        </div>
      ) : isLoading ? null : !entries?.length ? (
        <div className="card">
          {filtering ? (
            <EmptyState icon={<Search size={22} />} title="No memories match" action={<Button variant="secondary" onClick={clear}>Clear filters</Button>}>
              Try a different word, tag or date range.
            </EmptyState>
          ) : (
            <EmptyState
              icon={<BookHeart size={22} />}
              title="Your story starts here"
              action={
                <Button icon={<Plus size={15} />} onClick={() => open({ mode: "edit", id: null })}>
                  Write your first memory
                </Button>
              }
            >
              Your first date, a lazy Sunday, a silly inside joke: save it once and it's yours to keep.
            </EmptyState>
          )}
        </div>
      ) : view === "gallery" ? (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-6 px-1 pt-1">
          {entries.map((e) => (
            <EntryTile key={e.id} e={e} />
          ))}
        </div>
      ) : (
        <div className="grid gap-5 lg:grid-cols-2 2xl:grid-cols-3">
          {entries.map((e) => (
            <EntryCard key={e.id} e={e} />
          ))}
        </div>
      )}
      {entries && entries.length >= limit && (
        <div className="mt-6 flex justify-center">
          <Button variant="secondary" onClick={() => setLimit((l) => l + 120)}>
            Show more memories
          </Button>
        </div>
      )}
    </div>
  );
}
