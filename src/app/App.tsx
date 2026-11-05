import { useEffect, useState } from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { CommandPalette } from "./CommandPalette";
import { useNav, restoreLastPage } from "./navigation";
import type { PageId } from "./nav";
import { applyTheme, usePrefs } from "../lib/settings";
import { isDesktop } from "../lib/api";
import { useProfile } from "../lib/queries";
import { HomePage } from "../features/home/HomePage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { ComingSoonPage } from "../features/placeholder/ComingSoonPage";
import { JournalPage } from "../features/journal/JournalPage";
import { TimelinePage } from "../features/timeline/TimelinePage";
import { BucketPage } from "../features/bucket/BucketPage";
import { PlannerPage } from "../features/planner/PlannerPage";
import { QuestionsPage } from "../features/questions/QuestionsPage";
import { LettersPage } from "../features/letters/LettersPage";
import { SpecialPage } from "../features/special/SpecialPage";
import { useReminders } from "../features/special/useReminders";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { invalidateJournal } from "../lib/queries";
import { WelcomePage } from "../features/onboarding/WelcomePage";
import { Cupid } from "../cupid/Cupid";
import { Toasts } from "../components/ui/Toast";
import { EmptyState } from "../components/ui/EmptyState";
import { ErrorBoundary } from "../components/ui/ErrorBoundary";
import { useSyncEvents } from "../features/sync/syncApi";
import { UpdateBanner, useAutoUpdateCheck } from "../features/updates/updates";
import { usePlanEvents } from "../features/plan/planApi";
import { PlanBanner } from "../features/plan/PlanBanner";
import { UpgradeDialog } from "../features/plan/Purchase";

function Page({ page }: { page: PageId }) {
  switch (page) {
    case "home":
      return <HomePage />;
    case "settings":
      return <SettingsPage />;
    case "journal":
      return <JournalPage />;
    case "timeline":
      return <TimelinePage />;
    case "questions":
      return <QuestionsPage />;
    case "bucket":
      return <BucketPage />;
    case "special":
      return <SpecialPage />;
    case "letters":
      return <LettersPage />;
    case "planner":
      return <PlannerPage />;
    default:
      return <ComingSoonPage page={page} />;
  }
}

/** Refresh lists when a background video conversion finishes. */
function useMediaEvents() {
  useEffect(() => {
    if (!isDesktop) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen("media-updated", () => invalidateJournal()).then((u) => (cancelled ? u() : (unlisten = u))),
    );
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

function LeaveGuard() {
  const pending = useNav((s) => s.pending);
  const blocker = useNav((s) => s.blocker);
  const confirmLeave = useNav((s) => s.confirmLeave);
  const cancelLeave = useNav((s) => s.cancelLeave);
  return (
    <ConfirmDialog
      open={!!pending}
      title="Leave without saving?"
      message={`${blocker ?? "You have unsaved changes."} If you leave now, they'll be lost.`}
      confirmLabel="Discard changes"
      danger
      onConfirm={confirmLeave}
      onCancel={cancelLeave}
    />
  );
}

function Shell() {
  const page = useNav((s) => s.page);
  const journalView = useNav((s) => s.journal);
  const [paletteOpen, setPaletteOpen] = useState(false);
  useMediaEvents();
  useReminders();
  useAutoUpdateCheck();
  usePlanEvents();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Scroll back to top when switching pages.
  useEffect(() => {
    const el = document.getElementById("main-scroll");
    if (el) el.scrollTop = 0;
  }, [page, journalView]);

  return (
    <div className="flex h-full">
      <Sidebar />
      <div id="main-scroll" className="flex min-w-0 flex-1 flex-col overflow-y-auto">
        <TopBar onSearch={() => setPaletteOpen(true)} />
        <main className="mx-auto w-full max-w-[1480px] px-10 pt-9 pb-20">
          <PlanBanner />
          <ErrorBoundary resetKey={`${page}-${JSON.stringify(journalView)}`}>
            <Page key={page} page={page} />
          </ErrorBoundary>
        </main>
      </div>
      <Cupid />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
      <LeaveGuard />
      <UpdateBanner />
      <UpgradeDialog />
    </div>
  );
}

function PreviewBanner() {
  if (isDesktop) return null;
  return (
    <div className="fixed top-0 left-1/2 z-[200] -translate-x-1/2 rounded-b-xl bg-[#3d3435] px-4 py-1.5 text-[11px] text-white">
      Browser preview: nothing is saved. Run the desktop app for the real thing.
    </div>
  );
}

export function App() {
  const prefsLoaded = usePrefs((s) => s.loaded);
  const theme = usePrefs((s) => s.prefs["ui.theme"]);
  const loadPrefs = usePrefs((s) => s.load);
  const [bootError, setBootError] = useState<string | null>(null);
  const profile = useProfile();
  useSyncEvents();

  useEffect(() => {
    loadPrefs()
      .then(restoreLastPage)
      .catch((e) => setBootError(String(e)));
  }, [loadPrefs]);

  useEffect(() => applyTheme(theme), [theme]);

  const error = bootError ?? (profile.error ? String(profile.error) : null);
  if (error) {
    return (
      <div className="grid h-full place-items-center p-6">
        <div className="card max-w-md">
          <EmptyState title="Dear Us couldn't open your archive">
            Your memories are untouched. Please restart the app. If it keeps happening, this detail helps: {error}
          </EmptyState>
        </div>
      </div>
    );
  }

  if (!prefsLoaded || profile.isLoading) return <div className="h-full bg-bg" />;

  return (
    <>
      <PreviewBanner />
      {profile.data?.space ? <Shell /> : <WelcomePage />}
      <Toasts />
    </>
  );
}
