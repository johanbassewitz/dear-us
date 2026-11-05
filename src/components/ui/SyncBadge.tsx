import { Cloud, CloudOff, HardDrive, Loader, TriangleAlert } from "lucide-react";
import { describe, useSyncStatus } from "../../features/sync/syncApi";
import { useProfile } from "../../lib/queries";
import { useNav } from "../../app/navigation";

const tones = {
  green: "bg-sage text-[#4f7a55]",
  gold: "bg-gold text-[#9a7448]",
  accent: "bg-accent-soft text-accent-deep",
  gray: "bg-paper-2 text-muted",
} as const;

/** The real sync state, in plain words. Click → Settings & Sync. */
export function SyncBadge({ compact }: { compact?: boolean }) {
  const { data: s } = useSyncStatus();
  const { data: profile } = useProfile();
  const go = useNav((st) => st.go);
  const d = describe(s, profile?.partner?.displayName);
  const icon =
    !s || s.state === "off" ? <HardDrive size={12} /> : s.state === "offline" ? <CloudOff size={12} /> : s.state === "syncing" ? <Loader size={12} className="animate-spin" /> : s.state === "error" || s.state === "signed_out" ? <TriangleAlert size={12} /> : <Cloud size={12} />;
  return (
    <button
      onClick={() => go("settings")}
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1.5 text-[10.5px] font-bold ${tones[d.tone]}`}
      title={d.sentence}
      aria-label={`Sync: ${d.label}. ${d.sentence}`}
    >
      {icon}
      {compact && (!s || s.state === "off") ? "On this PC" : d.label}
      {!!s?.counts.openConflicts && <span className="ml-0.5 rounded-full bg-accent px-1.5 text-[9.5px] text-on-accent">{s.counts.openConflicts}</span>}
    </button>
  );
}
