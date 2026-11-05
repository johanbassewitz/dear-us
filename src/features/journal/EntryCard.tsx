import { Clapperboard, Heart, Image as ImageIcon, MapPin, MessageCircle, Star } from "lucide-react";
import type { EntrySummary } from "../../lib/api";
import { fmtDate } from "../../lib/format";
import { MediaThumb } from "../../components/media/MediaThumb";
import { useNav } from "../../app/navigation";

export function EngagementCounts({ e }: { e: EntrySummary }) {
  if (!e.reactionCount && !e.commentCount) return null;
  return (
    <span className="inline-flex items-center gap-2.5 text-[11px] text-faint">
      {e.reactionCount > 0 && (
        <span className="inline-flex items-center gap-1" aria-label={`${e.reactionCount} reactions`}>
          <Heart size={12} /> {e.reactionCount}
        </span>
      )}
      {e.commentCount > 0 && (
        <span className="inline-flex items-center gap-1" aria-label={`${e.commentCount} comments`}>
          <MessageCircle size={12} /> {e.commentCount}
        </span>
      )}
    </span>
  );
}

export function MediaCounts({ e }: { e: EntrySummary }) {
  if (!e.photoCount && !e.videoCount) return null;
  return (
    <span className="inline-flex items-center gap-2.5 text-[11px] text-faint">
      {e.photoCount > 0 && (
        <span className="inline-flex items-center gap-1">
          <ImageIcon size={12} /> {e.photoCount}
        </span>
      )}
      {e.videoCount > 0 && (
        <span className="inline-flex items-center gap-1">
          <Clapperboard size={12} /> {e.videoCount}
        </span>
      )}
    </span>
  );
}

/** Feed card: big cover photo, date, title, excerpt. */
export function EntryCard({ e }: { e: EntrySummary }) {
  const open = useNav((s) => s.openJournal);
  return (
    <button
      onClick={() => open({ mode: "view", id: e.id })}
      className="card group flex w-full flex-col overflow-hidden text-left transition-all hover:-translate-y-0.5 hover:shadow-[0_14px_30px_rgba(80,45,45,0.09)]"
    >
      {e.cover && (
        <div className="h-[210px] w-full overflow-hidden">
          <div className="h-full w-full transition-transform duration-500 group-hover:scale-[1.03]">
            <MediaThumb media={e.cover} />
          </div>
        </div>
      )}
      <div className="flex flex-1 flex-col p-5">
        <div className="flex items-center gap-2 text-[10px] font-bold tracking-[0.08em] text-faint uppercase">
          {fmtDate(e.entryDate)}
          {e.highlight && <Star size={11} className="fill-current text-accent" aria-label="Milestone" />}
        </div>
        <h3 className="mt-1.5 font-serif text-[21px] leading-tight text-ink">{e.title}</h3>
        {e.excerpt && <p className="mt-2 line-clamp-3 text-[12.5px] leading-relaxed text-muted">{e.excerpt}</p>}
        <div className="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1.5 pt-4">
          {e.location && (
            <span className="inline-flex items-center gap-1 text-[11px] text-muted">
              <MapPin size={12} /> {e.location}
            </span>
          )}
          {e.tags.slice(0, 3).map((t) => (
            <span key={t} className="rounded-full bg-accent-soft px-2 py-0.5 text-[10.5px] font-semibold text-accent-deep">
              #{t}
            </span>
          ))}
          <span className="ml-auto inline-flex items-center gap-3">
            <EngagementCounts e={e} />
            <MediaCounts e={e} />
          </span>
        </div>
      </div>
    </button>
  );
}

/** Gallery tile: polaroid with handwritten caption. */
export function EntryTile({ e }: { e: EntrySummary }) {
  const open = useNav((s) => s.openJournal);
  return (
    <button
      onClick={() => open({ mode: "view", id: e.id })}
      className="group border border-line bg-paper p-2.5 pb-3 text-left shadow-[0_10px_18px_rgba(80,50,50,0.08)] transition-transform even:rotate-[1.2deg] odd:-rotate-[1deg] hover:z-10 hover:scale-[1.03] hover:rotate-0"
    >
      <div className="aspect-square w-full overflow-hidden">
        <MediaThumb media={e.cover} />
      </div>
      <div className="mt-2 truncate text-center font-hand text-[19px] leading-tight text-ink-soft">{e.title}</div>
      <div className="text-center text-[10px] text-faint">{fmtDate(e.entryDate)}</div>
    </button>
  );
}
