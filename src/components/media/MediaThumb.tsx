import { useState } from "react";
import { CloudDownload, Heart, ImageOff, Loader, Play } from "lucide-react";
import { mediaUrl, type MediaRef } from "../../lib/api";

interface Props {
  media: MediaRef | null | undefined;
  /** thumb for grids/cards, display for large views */
  size?: "thumb" | "display";
  className?: string;
  alt?: string;
}

function Placeholder({ icon, text, className = "" }: { icon: React.ReactNode; text?: string; className?: string }) {
  return (
    <div className={`flex h-full w-full flex-col items-center justify-center gap-1.5 bg-blush p-3 text-center text-accent ${className}`}>
      {icon}
      {text && <span className="max-w-[180px] text-[10.5px] leading-snug text-muted">{text}</span>}
    </div>
  );
}

export function formatDuration(ms: number | null): string | null {
  if (!ms) return null;
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** A photo or video poster that degrades gracefully (missing, converting, unsupported). */
export function MediaThumb({ media, size = "thumb", className = "", alt = "" }: Props) {
  const [broken, setBroken] = useState(false);
  if (!media) return <Placeholder icon={<Heart size={26} strokeWidth={1.4} />} className={className} />;
  if (media.availability !== "local")
    return <Placeholder icon={<CloudDownload size={22} />} text="Still on its way from your partner's PC" className={className} />;

  const isVideo = media.kind === "video";
  let src: string | null = null;
  if (size === "thumb" && media.hasThumb) src = mediaUrl(media.id, "thumb");
  else if (!isVideo && media.preview !== "failed") src = mediaUrl(media.id, "display");
  else if (isVideo && media.hasThumb) src = mediaUrl(media.id, "thumb");

  const badge = isVideo && (
    <span className="absolute right-2 bottom-2 flex items-center gap-1 rounded-full bg-black/55 px-2 py-0.5 text-[10px] font-semibold text-white">
      {media.preview === "needed" ? <Loader size={10} className="animate-spin" /> : <Play size={10} fill="currentColor" />}
      {formatDuration(media.durationMs) ?? "video"}
    </span>
  );

  if (!src || broken) {
    if (isVideo && !broken && media.preview !== "failed" && media.preview !== "needed") {
      // No poster (no converter on this PC): let the browser show the first frame.
      const v = mediaUrl(media.id, "display");
      if (v)
        return (
          <div className={`relative h-full w-full bg-black ${className}`}>
            <video src={`${v}#t=0.5`} preload="metadata" muted className="h-full w-full object-cover" onError={() => setBroken(true)} />
            {badge}
          </div>
        );
    }
    return (
      <div className={`relative h-full w-full ${className}`}>
        <Placeholder
          icon={<ImageOff size={20} />}
          text={media.preview === "failed" ? "Can't preview this format on this PC. The original is saved safely." : undefined}
        />
        {badge}
      </div>
    );
  }
  return (
    <div className={`relative h-full w-full overflow-hidden bg-paper-2 ${className}`}>
      <img src={src} alt={alt} loading="lazy" draggable={false} className="h-full w-full object-cover" onError={() => setBroken(true)} />
      {badge}
    </div>
  );
}
