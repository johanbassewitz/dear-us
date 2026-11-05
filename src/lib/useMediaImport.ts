import { useCallback, useEffect, useState } from "react";
import { api, isDesktop, type MediaRef } from "./api";
import { toast, toastError } from "../components/ui/Toast";

export const MEDIA_EXTENSIONS = ["jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "avif", "mp4", "m4v", "mov", "webm", "mkv", "avi"];

/**
 * Adds photos/videos to the permanent archive via the file picker or drag & drop.
 * `onAdded` receives the archived media (duplicates included — they're stored once).
 */
export function useMediaImport(onAdded: (media: MediaRef[]) => void, opts: { dropEnabled?: boolean; multiple?: boolean } = {}) {
  const { dropEnabled = true, multiple = true } = opts;
  const [busy, setBusy] = useState(0);
  const [dragging, setDragging] = useState(false);

  const importPaths = useCallback(
    async (paths: string[]) => {
      if (!paths.length) return;
      setBusy(paths.length);
      try {
        const results = await api.mediaImport(multiple ? paths : paths.slice(0, 1));
        const ok = results.filter((r) => r.media).map((r) => r.media!);
        const failed = results.filter((r) => r.error);
        if (ok.length) onAdded(ok);
        if (failed.length) {
          const names = failed.map((f) => f.path.split(/[\\/]/).pop()).slice(0, 3).join(", ");
          toastError(`${failed.length} file${failed.length > 1 ? "s" : ""} couldn't be added (${names}): ${failed[0].error}`);
        } else if (ok.length > 1) {
          toast(`${ok.length} added to your archive ♡`);
        }
      } catch (e) {
        toastError(String(e));
      } finally {
        setBusy(0);
      }
    },
    [onAdded, multiple],
  );

  const pick = useCallback(async () => {
    if (!isDesktop) return toastError("Adding photos only works in the desktop app.");
    const { open } = await import("@tauri-apps/plugin-dialog");
    const chosen = await open({ multiple, directory: false, filters: [{ name: "Photos & videos", extensions: MEDIA_EXTENSIONS }] });
    if (!chosen) return;
    await importPaths(Array.isArray(chosen) ? chosen : [chosen]);
  }, [importPaths, multiple]);

  useEffect(() => {
    if (!isDesktop || !dropEnabled) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setDragging(true);
          else if (p.type === "leave") setDragging(false);
          else if (p.type === "drop") {
            setDragging(false);
            importPaths(p.paths);
          }
        })
        .then((u) => (cancelled ? u() : (unlisten = u))),
    );
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [dropEnabled, importPaths]);

  return { pick, importPaths, busy, dragging };
}
