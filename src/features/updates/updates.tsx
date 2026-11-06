// Auto-update (personal edition): checks GitHub Releases for a newer signed version, downloads it
// and restarts. The update is verified against the public key in tauri.conf.json before it's
// installed. The Microsoft Store edition has no self-updater: the Store updates it.
import { useEffect } from "react";
import { create } from "zustand";
import { Download, X } from "lucide-react";
import { isDesktop } from "../../lib/api";
import { useIsStore } from "../../lib/edition";
import { Button } from "../../components/ui/Button";

type Status = "idle" | "checking" | "none" | "available" | "downloading" | "ready" | "error";

interface UpdateState {
  status: Status;
  version: string | null;
  notes: string | null;
  progress: number | null; // 0..1 while downloading
  error: string | null;
  dismissed: boolean;
  check: (manual?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

// The Update object from the plugin; kept outside the store because it isn't serialisable.
let pending: import("@tauri-apps/plugin-updater").Update | null = null;

export const useUpdates = create<UpdateState>((set, get) => ({
  status: "idle",
  version: null,
  notes: null,
  progress: null,
  error: null,
  dismissed: false,
  check: async (manual = false) => {
    if (!isDesktop) return;
    const s = get().status;
    if (s === "checking" || s === "downloading" || s === "ready") return;
    set({ status: "checking", error: null });
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const u = await check();
      if (u) {
        pending = u;
        set({ status: "available", version: u.version, notes: u.body ?? null, dismissed: manual ? false : get().dismissed });
      } else {
        set({ status: "none" });
      }
    } catch (e) {
      // Offline or GitHub unreachable: only worth showing when someone asked.
      set({ status: manual ? "error" : "idle", error: String(e) });
    }
  },
  install: async () => {
    if (!pending) return;
    set({ status: "downloading", progress: 0, error: null });
    try {
      let total = 0;
      let done = 0;
      await pending.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          set({ progress: total ? done / total : null });
        } else if (ev.event === "Finished") set({ progress: 1 });
      });
      set({ status: "ready" });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      set({ status: "error", error: String(e) });
    }
  },
  dismiss: () => set({ dismissed: true }),
}));

const SIX_HOURS = 6 * 60 * 60 * 1000;

/** Checks shortly after launch, then every 6 hours while the app is open. */
export function useAutoUpdateCheck() {
  const check = useUpdates((s) => s.check);
  const store = useIsStore();
  useEffect(() => {
    if (!isDesktop || store) return;
    const first = setTimeout(() => void check(), 8000);
    const every = setInterval(() => void check(), SIX_HOURS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, [check, store]);
}

export function UpdateBanner() {
  const { status, version, progress, error, dismissed, install, dismiss } = useUpdates();
  const show = (status === "available" && !dismissed) || status === "downloading" || status === "ready" || (status === "error" && !!version && !dismissed);
  if (!show) return null;
  return (
    <div role="status" className="card fixed right-6 bottom-6 z-[150] flex w-[340px] items-start gap-3 p-4 shadow-lg">
      <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-accent-soft text-accent-deep">
        <Download size={17} />
      </span>
      <div className="min-w-0 flex-1 text-[12.5px]">
        {status === "available" && (
          <>
            <strong className="block">A new version of Dear Us is ready ({version})</strong>
            <span className="text-muted">Your memories stay exactly as they are. The app restarts once it's updated.</span>
            <div className="mt-3 flex gap-2">
              <Button size="sm" onClick={() => void install()}>Update now</Button>
              <Button size="sm" variant="secondary" onClick={dismiss}>Later</Button>
            </div>
          </>
        )}
        {status === "downloading" && (
          <>
            <strong className="block">Updating to {version}…</strong>
            <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-line">
              <div className="h-full bg-accent transition-all" style={{ width: `${Math.round((progress ?? 0.05) * 100)}%` }} />
            </div>
          </>
        )}
        {status === "ready" && <strong className="block">Restarting Dear Us…</strong>}
        {status === "error" && (
          <>
            <strong className="block">The update didn't finish</strong>
            <span className="text-muted">Nothing was changed. Try again later. ({error})</span>
          </>
        )}
      </div>
      {(status === "available" || status === "error") && (
        <button aria-label="Close" onClick={dismiss} className="text-faint hover:text-ink">
          <X size={15} />
        </button>
      )}
    </div>
  );
}

/** The "Updates" card in Settings. */
export function UpdateSettings({ currentVersion }: { currentVersion?: string }) {
  const { status, version, error, check, install } = useUpdates();
  const store = useIsStore();
  if (!isDesktop) return null;
  if (store)
    return (
      <p className="text-[12.5px]">
        This PC has <b>Dear Us {currentVersion ?? "…"}</b>. The Microsoft Store keeps it up to date automatically.
      </p>
    );
  return (
    <div className="flex flex-col gap-3 text-[12.5px]">
      <p>
        This PC has <b>Dear Us {currentVersion ?? "…"}</b>. New versions are checked automatically when the app opens.
      </p>
      {status === "none" && <p className="rounded-lg bg-sage px-3 py-2 text-[12px] text-[#3f6545]">You're on the latest version ✓</p>}
      {status === "available" && <p className="text-accent-deep">Version {version} is available.</p>}
      {status === "error" && <p className="text-muted">Couldn't check right now (are you online?). {error}</p>}
      <div className="flex gap-2">
        {status === "available" ? (
          <Button size="sm" onClick={() => void install()}>Update now</Button>
        ) : (
          <Button size="sm" variant="secondary" disabled={status === "checking" || status === "downloading"} onClick={() => void check(true)}>
            {status === "checking" ? "Checking…" : "Check for updates"}
          </Button>
        )}
      </div>
    </div>
  );
}
