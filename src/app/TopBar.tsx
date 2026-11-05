import { Plus, Search } from "lucide-react";
import { ALL_PAGES } from "./nav";
import { useNav } from "./navigation";
import { useProfile } from "../lib/queries";
import { Avatar } from "../components/ui/Avatar";
import { Button } from "../components/ui/Button";
import { SyncBadge } from "../components/ui/SyncBadge";

export function TopBar({ onSearch }: { onSearch: () => void }) {
  const page = useNav((s) => s.page);
  const go = useNav((s) => s.go);
  const openJournal = useNav((s) => s.openJournal);
  const { data: profile } = useProfile();
  const label = ALL_PAGES.find((p) => p.id === page)?.label ?? "";

  return (
    <header className="sticky top-0 z-30 flex h-[72px] shrink-0 items-center justify-between gap-4 border-b border-line bg-[var(--topbar)]/95 px-10 backdrop-blur">
      <div className="flex items-center gap-2 text-[11.5px] font-semibold text-faint">
        <span>{profile?.space?.name ?? "Our space"}</span>
        <span aria-hidden="true">›</span>
        <b className="text-ink-soft">{label}</b>
      </div>
      <div className="flex items-center gap-3">
        <SyncBadge />
        <button
          onClick={onSearch}
          className="flex min-w-[200px] items-center gap-2.5 rounded-[11px] border border-line bg-paper px-3 py-2 text-left text-[11.5px] text-muted transition-colors hover:border-accent"
        >
          <Search size={15} />
          <span className="flex-1">Search memories…</span>
          <kbd className="rounded border border-line bg-paper-2 px-1.5 py-0.5 text-[9.5px] text-faint">Ctrl K</kbd>
        </button>
        <Button icon={<Plus size={15} />} onClick={() => openJournal({ mode: "edit", id: null })}>
          New memory
        </Button>
        <button onClick={() => go("settings")} aria-label="Your profile and settings" className="rounded-full">
          <Avatar name={profile?.me?.displayName} size={36} />
        </button>
      </div>
    </header>
  );
}
