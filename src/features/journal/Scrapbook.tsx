import type { ReactNode } from "react";
import type { EntryMedia, ScrapFrame, ScrapLayout, ScrapPaper } from "../../lib/api";
import { MediaThumb } from "../../components/media/MediaThumb";
import { FRAME_OPTIONS, LAYOUT_OPTIONS, PAPER_OPTIONS, frameClass, paperClass, resolveLayout, type ResolvedLayout } from "./scrapbookStyles";

const TILTS = ["-rotate-[1.6deg]", "rotate-[1.2deg]", "-rotate-[0.8deg]", "rotate-[1.8deg]"];

function Framed({ frame, caption, className = "", children }: { frame: ScrapFrame; caption?: string | null; className?: string; children: ReactNode }) {
  return (
    <div className={`${frameClass(frame)} ${className}`}>
      {children}
      {frame === "polaroid" && caption && <div className="mt-1.5 truncate text-center font-hand text-[17px] leading-tight text-ink-soft">{caption}</div>}
    </div>
  );
}

function Tile({
  m,
  i,
  frame,
  onOpen,
  className = "",
  size,
}: {
  m: EntryMedia;
  i: number;
  frame: ScrapFrame;
  onOpen: (i: number) => void;
  className?: string;
  size?: "display";
}) {
  return (
    <Framed frame={frame} caption={m.caption} className={`flex min-h-0 flex-col ${className}`}>
      <button onClick={() => onOpen(i)} className="relative min-h-0 flex-1 overflow-hidden rounded-xl" aria-label={m.caption ? `Open photo: ${m.caption}` : "Open photo"}>
        <MediaThumb media={m} size={size} />
      </button>
    </Framed>
  );
}

/** The photo area of a memory, arranged by layout. Returns null when there is no media. */
export function ScrapMedia({
  media,
  layout,
  frame,
  onOpen,
}: {
  media: EntryMedia[];
  layout: ResolvedLayout;
  frame: ScrapFrame;
  onOpen: (i: number) => void;
}) {
  if (!media.length) return null;

  if (layout === "polaroid") {
    const f = frame === "none" ? "polaroid" : frame;
    return (
      <div className="flex flex-wrap justify-center gap-x-6 gap-y-8 px-6 py-8" data-testid="layout-polaroid">
        {media.map((m, i) => (
          <Tile key={m.id} m={m} i={i} frame={f} onOpen={onOpen} className={`h-[240px] w-[min(100%,260px)] transition-transform hover:rotate-0 ${TILTS[i % TILTS.length]}`} />
        ))}
      </div>
    );
  }

  if (layout === "magazine") {
    const [first, ...rest] = media;
    return (
      <div data-testid="layout-magazine">
        <Tile m={first} i={0} frame={frame} onOpen={onOpen} className="mx-auto h-[440px] w-full" size="display" />
        {rest.length > 0 && (
          <div className="mt-2 grid grid-cols-[repeat(auto-fill,minmax(130px,1fr))] gap-2">
            {rest.map((m, i) => (
              <Tile key={m.id} m={m} i={i + 1} frame={frame} onOpen={onOpen} className="h-[100px]" />
            ))}
          </div>
        )}
      </div>
    );
  }

  if (layout === "minimal") {
    return (
      <div className="flex flex-wrap gap-2.5" data-testid="layout-minimal">
        {media.map((m, i) => (
          <Tile key={m.id} m={m} i={i} frame={frame} onOpen={onOpen} className="h-[96px] w-[96px]" />
        ))}
      </div>
    );
  }

  // collage: one big tile and a mosaic
  const [first, ...rest] = media;
  const shown = rest.slice(0, 4);
  return (
    <div data-testid="layout-collage">
      <div className={`grid gap-2 ${rest.length ? "grid-cols-4 grid-rows-2" : ""}`} style={{ height: rest.length ? 420 : 460 }}>
        <Tile m={first} i={0} frame={frame} onOpen={onOpen} size="display" className={rest.length ? "col-span-2 row-span-2" : "h-full"} />
        {shown.map((m, i) => (
          <Tile
            key={m.id}
            m={m}
            i={i + 1}
            frame={frame}
            onOpen={onOpen}
            className={rest.length === 1 ? "col-span-2 row-span-2" : rest.length === 2 ? "col-span-2" : ""}
          />
        ))}
      </div>
      {rest.length > 4 && (
        <div className="mt-2 grid grid-cols-[repeat(auto-fill,minmax(110px,1fr))] gap-2">
          {rest.slice(4).map((m, i) => (
            <Tile key={m.id} m={m} i={i + 5} frame={frame} onOpen={onOpen} className="aspect-square" />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------- editor panel ----------

function Swatches<T extends string>({
  label,
  value,
  options,
  onChange,
  draw,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  draw: (v: T) => ReactNode;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="flex flex-col gap-1.5">
      <span className="text-[11px] font-bold text-ink-soft">{label}</span>
      <div className="grid grid-cols-5 gap-1.5">
        {options.map((o) => {
          const on = o.value === value;
          return (
            <button
              key={o.value}
              type="button"
              role="radio"
              aria-checked={on}
              aria-label={`${label}: ${o.label}`}
              onClick={() => onChange(o.value)}
              className={`flex flex-col items-center gap-1 rounded-lg border p-1 text-[9.5px] font-semibold ${on ? "border-accent bg-accent-soft text-accent-deep" : "border-line bg-paper text-muted hover:border-accent"}`}
            >
              <span className="grid h-9 w-full place-items-center overflow-hidden rounded-md bg-paper-2">{draw(o.value)}</span>
              {o.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

const block = "rounded-[2px] bg-accent/35";

function LayoutArt({ v }: { v: ScrapLayout }) {
  switch (v) {
    case "polaroid":
      return (
        <span className="flex gap-1">
          <span className="h-5 w-4 -rotate-6 border border-line bg-paper p-[2px] pb-1.5"><span className={`block h-full ${block}`} /></span>
          <span className="h-5 w-4 rotate-6 border border-line bg-paper p-[2px] pb-1.5"><span className={`block h-full ${block}`} /></span>
        </span>
      );
    case "collage":
      return (
        <span className="grid h-6 w-8 grid-cols-3 grid-rows-2 gap-[2px]">
          <span className={`col-span-2 row-span-2 ${block}`} />
          <span className={block} />
          <span className={block} />
        </span>
      );
    case "magazine":
      return (
        <span className="flex w-8 flex-col gap-[2px]">
          <span className={`h-3.5 ${block}`} />
          <span className="h-[2px] w-6 bg-faint" />
          <span className="h-[2px] w-4 bg-faint" />
        </span>
      );
    case "minimal":
      return (
        <span className="flex w-8 flex-col gap-[2px]">
          <span className="h-[3px] w-5 bg-ink-soft" />
          <span className="h-[2px] w-8 bg-faint" />
          <span className="h-[2px] w-7 bg-faint" />
          <span className="mt-0.5 flex gap-[2px]">
            <span className={`h-2 w-2 ${block}`} />
            <span className={`h-2 w-2 ${block}`} />
          </span>
        </span>
      );
    default:
      return <span className="font-hand text-[18px] leading-none text-accent">auto</span>;
  }
}

/** Sidebar panel in the memory editor. */
export function StylePanel({
  layout,
  frame,
  paper,
  mediaCount,
  onChange,
}: {
  layout: ScrapLayout;
  frame: ScrapFrame;
  paper: ScrapPaper;
  mediaCount: number;
  onChange: (patch: { layout?: ScrapLayout; frame?: ScrapFrame; paper?: ScrapPaper }) => void;
}) {
  const picked = resolveLayout(layout, mediaCount);
  return (
    <fieldset className="flex flex-col gap-3.5 border-t border-line pt-4">
      <legend className="sr-only">Style</legend>
      <div className="tiny-label">Style</div>
      <Swatches label="Layout" value={layout} options={LAYOUT_OPTIONS} onChange={(v) => onChange({ layout: v })} draw={(v) => <LayoutArt v={v} />} />
      {layout === "auto" && (
        <p className="-mt-1.5 text-[11px] text-muted">
          Auto picks a <strong className="font-semibold">{picked}</strong> look for{" "}
          {mediaCount === 0 ? "a memory without photos" : mediaCount === 1 ? "one photo" : `${mediaCount} photos`}.
        </p>
      )}
      <Swatches
        label="Photo frame"
        value={frame}
        options={FRAME_OPTIONS}
        onChange={(v) => onChange({ frame: v })}
        draw={(v) => (
          <span className={`block h-6 w-6 ${frameClass(v)}`} style={v === "none" || v === "rounded" ? undefined : { padding: 2 }}>
            <span className={`block h-full w-full ${block}`} />
          </span>
        )}
      />
      <Swatches
        label="Paper"
        value={paper}
        options={PAPER_OPTIONS}
        onChange={(v) => onChange({ paper: v })}
        draw={(v) => <span className={`block h-full w-full ${paperClass(v)}`} />}
      />
    </fieldset>
  );
}
