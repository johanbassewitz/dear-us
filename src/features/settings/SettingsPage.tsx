import { useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { UpdateSettings } from "../updates/updates";
import { Check, FolderOpen, HardDrive } from "lucide-react";
import { SyncSettings } from "../sync/SyncSettings";
import { CupidSettings } from "../../cupid/CupidSettings";
import { BackupSettings, RecoveryBin } from "../backup/BackupSettings";
import { keys, useAppInfo, useCapabilities, useProfile } from "../../lib/queries";
import { THEMES, usePrefs } from "../../lib/settings";
import { api, isDesktop } from "../../lib/api";
import { ProfileForm } from "./ProfileForm";
import { useIsStore } from "../../lib/edition";
import { PlanCard } from "../plan/PlanCard";
import { ensureNotificationPermission } from "../special/useReminders";
import { toast, toastError } from "../../components/ui/Toast";
import { Button } from "../../components/ui/Button";

function Section({ title, desc, children }: { title: string; desc?: string; children: ReactNode }) {
  return (
    <section className="card p-6">
      <h2 className="section-title">{title}</h2>
      {desc && <p className="mt-1 mb-5 text-[12px] text-muted">{desc}</p>}
      {!desc && <div className="mb-4" />}
      {children}
    </section>
  );
}

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={`relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors ${on ? "bg-accent" : "bg-line"}`}
    >
      <span className={`absolute top-[3px] h-4 w-4 rounded-full bg-white shadow transition-all ${on ? "left-[19px]" : "left-[3px]"}`} />
    </button>
  );
}

function ConverterCard() {
  const { data } = useCapabilities();
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number | null } | null>(null);
  if (!isDesktop) return null;

  const install = async () => {
    setBusy(true);
    setProgress(null);
    const { listen } = await import("@tauri-apps/api/event");
    const off = await listen<{ done: number; total: number | null }>("converter-progress", (e) => setProgress(e.payload));
    try {
      await api.mediaInstallConverter();
      await qc.invalidateQueries({ queryKey: keys.capabilities });
      toast("Converter installed ♡ iPhone photos and videos will show now");
    } catch (e) {
      toastError(String(e));
    } finally {
      off();
      setBusy(false);
    }
  };

  if (data?.converter) {
    return <p className="rounded-lg bg-sage px-3 py-2 text-[12px] text-[#3f6545]">Installed ✓ iPhone photos and videos get viewable copies automatically.</p>;
  }
  const mb = (n: number) => Math.round(n / 1024 / 1024);
  return (
    <div className="flex flex-col gap-2 text-[12.5px] text-muted">
      <p>Not on this PC yet. One click downloads the free converter (ffmpeg, about 110 MB) and sets it up. Nothing else on your PC changes.</p>
      <div className="flex items-center gap-3">
        <Button size="sm" disabled={busy} onClick={() => void install()}>
          {busy ? "Downloading…" : "Download converter"}
        </Button>
        {busy && progress && (
          <span className="text-[11.5px]">
            {mb(progress.done)}
            {progress.total ? ` / ${mb(progress.total)}` : ""} MB
          </span>
        )}
      </div>
      <p className="text-[11px] text-faint">Photos and videos you added earlier get their previews automatically afterwards.</p>
    </div>
  );
}

export function SettingsPage() {
  const { data: profile, isLoading } = useProfile();
  const { data: info } = useAppInfo();
  const store = useIsStore();
  const prefs = usePrefs((s) => s.prefs);
  const setPref = usePrefs((s) => s.set);

  return (
    <div className="animate-fade-up">
      <div className="eyebrow">Just how we like it</div>
      <h1 className="page-title mt-2">Settings & Sync</h1>
      <p className="mt-2 mb-7 text-[13px] text-muted">Your space, your look, and where your memories live.</p>

      <div className="grid gap-5 lg:grid-cols-2">
        <Section title="Our space" desc="Shared with your partner once you're connected.">
          {!isLoading && <ProfileForm profile={profile} submitLabel="Save changes" onSaved={() => toast("Saved ♡")} />}
        </Section>

        <div className="flex flex-col gap-5">
          <Section title="Look & feel" desc="Only changes this PC. Your partner keeps their own look.">
            <div className="flex flex-wrap gap-3" role="radiogroup" aria-label="Theme">
              {THEMES.map((t) => {
                const active = prefs["ui.theme"] === t.id;
                return (
                  <button
                    key={t.id}
                    role="radio"
                    aria-checked={active}
                    onClick={() => setPref("ui.theme", t.id)}
                    className="flex flex-col items-center gap-1.5"
                  >
                    <span
                      className={`grid h-11 w-11 place-items-center rounded-xl border-[3px] border-paper text-white transition-shadow ${
                        active ? "shadow-[0_0_0_2px_var(--accent)]" : "shadow-[0_0_0_1px_var(--line)]"
                      }`}
                      style={{ background: t.swatch }}
                    >
                      {active && <Check size={16} />}
                    </span>
                    <span className={`text-[11px] ${active ? "font-bold text-accent-deep" : "text-muted"}`}>{t.name}</span>
                  </button>
                );
              })}
            </div>
            <div className="mt-6 flex items-center justify-between gap-4 border-t border-line-soft pt-4">
              <div>
                <strong className="block text-[12.5px]">Show Cupid</strong>
                <span className="text-[11.5px] text-muted">Your little floating companion. Drag him anywhere.</span>
              </div>
              <Toggle on={prefs["cupid.visible"]} onChange={(v) => setPref("cupid.visible", v)} label="Show Cupid" />
            </div>
            <div className="mt-4 flex items-center justify-between gap-4 border-t border-line-soft pt-4">
              <div>
                <strong className="block text-[12.5px]">Desktop reminders</strong>
                <span className="text-[11.5px] text-muted">
                  A gentle nudge for special dates that have a reminder set. Reminders only appear while Dear Us is open, and only on this PC.
                  {!isDesktop && " Available in the desktop app."}
                </span>
              </div>
              <Toggle
                on={prefs["reminders.enabled"]}
                onChange={async (v) => {
                  if (!v) return setPref("reminders.enabled", false);
                  try {
                    if (await ensureNotificationPermission()) setPref("reminders.enabled", true);
                    else toastError(isDesktop ? "Windows isn't allowing notifications for Dear Us right now." : "Reminders need the desktop app.");
                  } catch (e) {
                    toastError(String(e));
                  }
                }}
                label="Desktop reminders"
              />
            </div>
          </Section>

          <Section title="Your archive" desc="Everything is stored permanently on this computer.">
            <div className="flex items-start gap-3 rounded-xl bg-paper-2 p-3.5">
              <HardDrive size={18} className="mt-0.5 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-[12px] font-semibold">Data folder</div>
                <div className="truncate text-[11.5px] text-muted" title={info?.dataDir}>
                  {info?.dataDir ?? "…"}
                </div>
              </div>
              {isDesktop && (
                <Button
                  variant="secondary"
                  size="sm"
                  icon={<FolderOpen size={14} />}
                  onClick={() => api.openDataDir().catch((e) => toastError(`Couldn't open folder: ${e}`))}
                >
                  Open
                </Button>
              )}
            </div>
            <p className="mt-3 text-[11px] text-faint">
              Dear Us {info?.version ?? ""} · archive format v{info?.schemaVersion ?? "?"}
            </p>
          </Section>
        </div>
      </div>

      <div className="mt-5">
        {store ? (
          <div className="grid gap-5 lg:grid-cols-[1fr_1.4fr]">
            <Section title="Your plan" desc="One plan covers both of you.">
              <PlanCard />
            </Section>
            <Section title="Partner sync" desc="Your memories travel end-to-end encrypted through the Dear Us cloud and stay permanently on both PCs. We can't read them.">
              <SyncSettings />
            </Section>
          </div>
        ) : (
          <Section title="Partner sync" desc="Your memories travel encrypted through your own free Supabase project and stay permanently on both PCs.">
            <SyncSettings />
          </Section>
        )}
      </div>

      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <Section title="Backups & recovery" desc="An extra copy of everything, somewhere you choose.">
          <BackupSettings />
        </Section>
        <Section title="Recovery bin" desc="Deleted memories, plans, dates and more wait here for 30 days.">
          <RecoveryBin />
        </Section>
      </div>

      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <Section
          title="Cupid's brain"
          desc={store ? "Optional. Cupid can chat, pick the daily question and suggest dates." : "Optional. Cupid can chat, pick the daily question and suggest dates using your own free Gemini key."}
        >
          <CupidSettings />
        </Section>
        {!store && (
          <Section title="Photo & video converter" desc="Needed to show iPhone HEIC photos and HEVC/.mov videos. Originals are always saved, either way.">
            <ConverterCard />
          </Section>
        )}
        <Section title="Updates" desc={store ? "Your memories are never touched by updates." : "New versions install themselves in a minute. Your memories are never touched."}>
          <UpdateSettings currentVersion={info?.version} />
        </Section>
      </div>
    </div>
  );
}
