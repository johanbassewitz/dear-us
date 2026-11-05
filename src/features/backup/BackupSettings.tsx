import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArchiveRestore, FolderOpen, HardDriveDownload, Loader, RotateCcw, ShieldCheck, Trash2 } from "lucide-react";
import { call, isDesktop } from "../../lib/api";
import { queryClient } from "../../lib/queries";
import { daysUntilPurge } from "../../lib/format";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { toast, toastError } from "../../components/ui/Toast";

interface BackupStatus {
  folder: string | null;
  auto: boolean;
  lastAt: string | null;
  lastOk: boolean | null;
  lastError: string | null;
  lastCopied: number | null;
  lastTotal: number | null;
  localSnapshots: number;
}
interface VerifyReport {
  ok: boolean;
  checkedFiles: number;
  missing: string[];
  damaged: string[];
  dbOk: boolean;
}
interface RestoreReport {
  addedRows: number;
  addedFiles: number;
  skippedExistingFiles: number;
}
interface BackupInfo {
  createdAt: string;
  spaceName: string | null;
  entries: number;
  files: number;
}
interface BinItem {
  entity: string;
  id: string;
  label: string;
  title: string;
  deletedAt: string;
}

export const backupApi = {
  status: () => call<BackupStatus>("backup_status"),
  setFolder: (path: string) => call<void>("backup_set_folder", { path }),
  setAuto: (on: boolean) => call<void>("backup_set_auto", { on }),
  run: () => call<{ copiedFiles: number; totalFiles: number; verify: VerifyReport }>("backup_run"),
  inspect: (path: string) => call<BackupInfo>("backup_inspect", { path }),
  verify: (path: string) => call<VerifyReport>("backup_verify", { path }),
  verifyArchive: () => call<VerifyReport>("archive_verify"),
  restore: (path: string) => call<RestoreReport>("backup_restore", { path }),
  binList: () => call<BinItem[]>("bin_list"),
  binRestore: (entity: string, id: string) => call<void>("bin_restore", { entity, id }),
  binPurge: (entity: string, id: string) => call<void>("bin_purge", { entity, id }),
};

export async function pickFolder(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === "string" ? r : null;
}

function useProgress() {
  const [p, setP] = useState<{ done: number; total: number } | null>(null);
  useEffect(() => {
    if (!isDesktop) return;
    let off: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen<{ done: number; total: number }>("backup-progress", (e) => setP(e.payload)).then((u) => (cancelled ? u() : (off = u))),
    );
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);
  return [p, setP] as const;
}

function when(iso: string | null) {
  return iso ? new Date(iso).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "never";
}

function verifyText(v: VerifyReport) {
  if (v.ok) return `All good ✓ ${v.checkedFiles} photo${v.checkedFiles === 1 ? "" : "s/videos"} checked, database healthy.`;
  return `${v.missing.length} missing, ${v.damaged.length} damaged${v.dbOk ? "" : ", database needs attention"}.`;
}

/** Restore from a backup folder (merge: adds what's missing, never overwrites). */
export function RestorePanel({ onDone }: { onDone?: () => void }) {
  const [path, setPath] = useState<string | null>(null);
  const [info, setInfo] = useState<BackupInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useProgress();
  const choose = async () => {
    const p = await pickFolder("Choose the backup folder (DearUsBackup)");
    if (!p) return;
    try {
      setInfo(await backupApi.inspect(p));
      setPath(p);
    } catch (e) {
      toastError(String(e));
    }
  };
  const restore = async () => {
    setBusy(true);
    setProgress(null);
    try {
      const r = await backupApi.restore(path!);
      queryClient.invalidateQueries();
      toast(r.addedRows || r.addedFiles ? `Restored ${r.addedRows} items and ${r.addedFiles} photos/videos ♡` : "Everything in that backup is already here ♡");
      onDone?.();
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-3">
      {info && path ? (
        <div className="rounded-xl bg-paper-2 p-3.5 text-[12.5px]">
          <strong>{info.spaceName ?? "Dear Us backup"}</strong>
          <div className="text-muted">
            Saved {when(info.createdAt)} · {info.entries} memories · {info.files} photos/videos
          </div>
          <div className="mt-1 truncate text-[11px] text-faint">{path}</div>
        </div>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <Button variant="secondary" size="sm" icon={<FolderOpen size={13} />} onClick={choose} disabled={busy}>
          {path ? "Choose another folder" : "Choose backup folder"}
        </Button>
        {path && (
          <Button size="sm" icon={busy ? <Loader size={13} className="animate-spin" /> : <ArchiveRestore size={13} />} onClick={restore} disabled={busy}>
            {busy ? `Restoring${progress ? ` ${progress.done}/${progress.total}` : "…"}` : "Restore"}
          </Button>
        )}
      </div>
      <p className="text-[11px] text-faint">Restoring only adds what's missing on this PC. Nothing here is overwritten or deleted, and every file is checked first.</p>
    </div>
  );
}

export function BackupSettings() {
  const { data: st } = useQuery({ queryKey: ["backup", "status"], queryFn: backupApi.status, enabled: isDesktop });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["backup"] });
  const [busy, setBusy] = useState<string | null>(null);
  const [report, setReport] = useState<string | null>(null);
  const [progress, setProgress] = useProgress();

  if (!isDesktop) return <p className="text-[12.5px] text-muted">Backups work in the desktop app.</p>;
  if (!st) return null;

  const choose = async () => {
    const p = await pickFolder("Choose where to keep backups");
    if (!p) return;
    try {
      await backupApi.setFolder(p);
      refresh();
      toast("Backup folder saved. Daily backups are on.");
    } catch (e) {
      toastError(String(e));
    }
  };
  const run = async () => {
    setBusy("run");
    setProgress(null);
    try {
      const r = await backupApi.run();
      setReport(`Backup complete and verified ✓ ${r.copiedFiles} new file${r.copiedFiles === 1 ? "" : "s"} copied (${r.totalFiles} in total).`);
    } catch (e) {
      setReport(null);
      toastError(String(e));
    } finally {
      setBusy(null);
      refresh();
    }
  };
  const verify = async (what: "backup" | "archive") => {
    setBusy(what);
    try {
      const v = what === "archive" ? await backupApi.verifyArchive() : await backupApi.verify(st.folder!);
      setReport(`${what === "archive" ? "This PC's archive" : "Backup"}: ${verifyText(v)}`);
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-start gap-3 rounded-xl bg-paper-2 p-4">
        <HardDriveDownload size={20} className="mt-0.5 shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <strong className="block text-[13px]">{st.folder ? "Backups go to" : "No backup folder yet"}</strong>
          <div className="truncate text-[12px] text-muted" title={st.folder ?? ""}>
            {st.folder ?? "Pick an external drive, or your own OneDrive / Google Drive folder."}
          </div>
          {st.lastAt && (
            <div className={`mt-1 text-[11.5px] ${st.lastOk ? "text-[#4f7a55]" : "text-[#b0413e]"}`}>
              {st.lastOk ? `Last backup ${when(st.lastAt)} ✓ verified` : `Last attempt ${when(st.lastAt)} failed: ${st.lastError}`}
            </div>
          )}
        </div>
        <Button variant="secondary" size="sm" icon={<FolderOpen size={13} />} onClick={choose}>
          {st.folder ? "Change" : "Choose folder"}
        </Button>
      </div>

      {st.folder && (
        <label className="flex items-center justify-between gap-4">
          <span>
            <strong className="block text-[12.5px]">Back up automatically every day</strong>
            <span className="text-[11.5px] text-muted">While Dear Us is open. Photos are copied once; each backup is checked file by file.</span>
          </span>
          <input
            type="checkbox"
            checked={st.auto}
            onChange={async (e) => {
              await backupApi.setAuto(e.target.checked);
              refresh();
            }}
            className="h-4 w-4 accent-[var(--accent)]"
          />
        </label>
      )}

      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={run} disabled={!st.folder || !!busy} icon={busy === "run" ? <Loader size={13} className="animate-spin" /> : undefined}>
          {busy === "run" ? `Backing up${progress ? ` ${progress.done}/${progress.total}` : "…"}` : "Back up now"}
        </Button>
        <Button variant="secondary" size="sm" icon={<ShieldCheck size={13} />} onClick={() => verify("backup")} disabled={!st.folder || !!busy}>
          Verify backup
        </Button>
        <Button variant="secondary" size="sm" icon={<ShieldCheck size={13} />} onClick={() => verify("archive")} disabled={!!busy}>
          Verify this PC's archive
        </Button>
      </div>
      {report && <p className="rounded-lg bg-sage px-3 py-2 text-[12px] text-[#3f6545]">{report}</p>}
      <p className="text-[11px] text-faint">
        Dear Us also keeps a daily safety copy of the database inside its own folder ({st.localSnapshots} kept). Sign-ins and your space key are never stored in
        backups. Keep your recovery phrase separately.
      </p>

      <div className="border-t border-line-soft pt-4">
        <strong className="mb-2 block text-[12.5px]">Restore from a backup</strong>
        <RestorePanel />
      </div>
    </div>
  );
}

export function RecoveryBin() {
  const { data: items } = useQuery({ queryKey: ["backup", "bin"], queryFn: backupApi.binList, enabled: isDesktop });
  const [forever, setForever] = useState<BinItem | null>(null);
  const refresh = () => queryClient.invalidateQueries();
  if (!isDesktop) return null;
  if (!items?.length) return <p className="text-[12.5px] text-muted">Nothing deleted recently. Deleted things wait here for 30 days.</p>;
  return (
    <>
      <ul className="divide-y divide-line-soft rounded-xl border border-line">
        {items.map((i) => (
          <li key={`${i.entity}-${i.id}`} className="flex items-center gap-3 px-3.5 py-2.5 text-[12.5px]">
            <span className="w-[86px] shrink-0 text-[10.5px] font-bold tracking-wide text-faint uppercase">{i.label}</span>
            <span className="min-w-0 flex-1 truncate">{i.title}</span>
            <span className="shrink-0 text-[11px] text-faint">gone in {daysUntilPurge(i.deletedAt)}d</span>
            <button
              onClick={async () => {
                try {
                  await backupApi.binRestore(i.entity, i.id);
                  refresh();
                  toast("Restored ♡");
                } catch (e) {
                  toastError(String(e));
                }
              }}
              className="inline-flex items-center gap-1 text-[11.5px] font-semibold text-accent hover:text-accent-deep"
            >
              <RotateCcw size={12} /> Restore
            </button>
            <button onClick={() => setForever(i)} aria-label={`Delete ${i.title} forever`} className="text-faint hover:text-[#b0413e]">
              <Trash2 size={13} />
            </button>
          </li>
        ))}
      </ul>
      <ConfirmDialog
        open={!!forever}
        title="Delete forever?"
        message={`“${forever?.title ?? ""}” will be permanently removed from this PC (and your partner's, once synced). This can't be undone.`}
        confirmLabel="Delete forever"
        danger
        onCancel={() => setForever(null)}
        onConfirm={async () => {
          const i = forever!;
          setForever(null);
          try {
            await backupApi.binPurge(i.entity, i.id);
            refresh();
          } catch (e) {
            toastError(String(e));
          }
        }}
      />
    </>
  );
}
