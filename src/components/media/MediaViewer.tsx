import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { ChevronLeft, ChevronRight, ImageOff, Loader, X } from "lucide-react";
import { mediaUrl, type EntryMedia } from "../../lib/api";

interface Props {
  items: EntryMedia[];
  index: number | null;
  onClose: () => void;
}

/** Full-screen photo/video viewer. ← → to move, Esc to close. */
export function MediaViewer({ items, index, onClose }: Props) {
  const [i, setI] = useState(index ?? 0);
  useEffect(() => setI(index ?? 0), [index]);

  useEffect(() => {
    if (index === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowRight") setI((x) => Math.min(x + 1, items.length - 1));
      if (e.key === "ArrowLeft") setI((x) => Math.max(x - 1, 0));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, items.length, onClose]);

  if (index === null || !items[i]) return null;
  const m = items[i];
  const src = mediaUrl(m.id, "display");

  let body: React.ReactNode;
  if (m.availability !== "local") body = <Note>This one is still on its way from your partner's PC.</Note>;
  else if (m.preview === "needed")
    body = (
      <Note>
        <Loader className="mb-2 animate-spin" /> Getting this video ready to play. It only takes a moment the first time.
      </Note>
    );
  else if (m.preview === "failed")
    body = (
      <Note>
        <ImageOff className="mb-2" /> This format can't be shown on this PC, but the original is saved safely in your archive.
      </Note>
    );
  else if (m.kind === "video")
    body = <video key={m.id} src={src ?? undefined} controls autoPlay className="max-h-[82vh] max-w-[92vw] rounded-lg bg-black" />;
  else body = <img key={m.id} src={src ?? undefined} alt={m.caption ?? ""} className="max-h-[82vh] max-w-[92vw] rounded-lg object-contain shadow-2xl" />;

  const nav = "absolute top-1/2 grid h-12 w-12 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white hover:bg-white/20 disabled:opacity-20";

  return createPortal(
    <div role="dialog" aria-modal="true" aria-label="Photo viewer" className="fixed inset-0 z-[100] flex flex-col items-center justify-center bg-[#140f11]/95" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <button onClick={onClose} aria-label="Close" className="absolute top-5 right-5 grid h-11 w-11 place-items-center rounded-full bg-white/10 text-white hover:bg-white/20">
        <X size={20} />
      </button>
      <div className="absolute top-6 left-6 text-[12px] text-white/60">
        {i + 1} / {items.length}
      </div>
      {body}
      {m.caption && <p className="mt-4 max-w-xl text-center font-serif text-[16px] text-white/85 italic">{m.caption}</p>}
      {items.length > 1 && (
        <>
          <button className={`${nav} left-5`} onClick={() => setI(i - 1)} disabled={i === 0} aria-label="Previous">
            <ChevronLeft />
          </button>
          <button className={`${nav} right-5`} onClick={() => setI(i + 1)} disabled={i === items.length - 1} aria-label="Next">
            <ChevronRight />
          </button>
        </>
      )}
    </div>,
    document.body,
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <div className="flex max-w-sm flex-col items-center text-center text-[14px] leading-relaxed text-white/80">{children}</div>;
}
