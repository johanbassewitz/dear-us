import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { CheckCircle2, ClipboardCopy, Cloud, KeyRound, Laptop, Loader, RefreshCw, ShieldCheck, Unplug, UserPlus } from "lucide-react";
import setupSql from "../../../supabase/setup.sql?raw";
import { describe, syncApi, syncKeys, useSyncStatus, type SyncConflict } from "./syncApi";
import { JoinForm } from "./JoinForm";
import { useProfile, queryClient } from "../../lib/queries";
import { useIsStore } from "../../lib/edition";
import { isDesktop } from "../../lib/api";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { Modal } from "../../components/ui/Modal";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { Pill } from "../../components/ui/Pill";
import { toast, toastError } from "../../components/ui/Toast";

const refresh = () => queryClient.invalidateQueries({ queryKey: ["sync"] });

async function copy(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast(`${what} copied`);
  } catch {
    toastError("Couldn't copy. Select the text and copy it by hand.");
  }
}

function Step({ n, title, done, children }: { n: number; title: string; done?: boolean; children: React.ReactNode }) {
  return (
    <li className="flex gap-3.5">
      <span className={`mt-0.5 grid h-7 w-7 shrink-0 place-items-center rounded-full text-[12px] font-bold ${done ? "bg-sage text-[#4f7a55]" : "bg-accent-soft text-accent-deep"}`}>
        {done ? <CheckCircle2 size={15} /> : n}
      </span>
      <div className="min-w-0 flex-1 pb-5">
        <strong className="block text-[13px]">{title}</strong>
        <div className="mt-1.5 text-[12px] leading-relaxed text-muted">{children}</div>
      </div>
    </li>
  );
}

/** Store edition: a Dear Us account (email + password). Nothing else to set up. */
function CloudSetup() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [haveAccount, setHaveAccount] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [joining, setJoining] = useState(false);

  if (joining)
    return (
      <div>
        <p className="mb-4 text-[12.5px] text-muted">Paste the invite code your partner sent you, and create your own Dear Us account (or sign in to it).</p>
        <JoinForm needName={false} onJoined={() => toast("You're connected ♡ Your memories will start syncing.")} />
        <button onClick={() => setJoining(false)} className="mt-4 text-[12px] text-muted hover:text-ink">
          ← I'm the first of us to set this up
        </button>
      </div>
    );

  const connect = async () => {
    setError(null);
    setBusy(true);
    try {
      await syncApi.connectAccount("", "", email, password, !haveAccount);
      refresh();
    } catch (e) {
      setError(String(e).replace(/^Error:\s*/, ""));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-xl bg-paper-2 p-4 text-[12.5px] leading-relaxed text-ink-soft">
        <b>Did your partner already set this up?</b> Then you only need their invite code.{" "}
        <button onClick={() => setJoining(true)} className="font-bold text-accent hover:underline">
          Join with an invite code →
        </button>
      </div>
      <p className="text-[12.5px] text-muted">
        Each of you has your own Dear Us account. Your memories are locked with a key that only exists on your two PCs, so not even we can read them.
      </p>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Your email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="email" />
        <Field label="Password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete={haveAccount ? "current-password" : "new-password"} hint="At least 8 characters" />
      </div>
      <label className="flex items-center gap-2 text-[12px] text-ink-soft">
        <input type="checkbox" checked={haveAccount} onChange={(e) => setHaveAccount(e.target.checked)} className="accent-[var(--accent)]" /> I already have a Dear Us account
      </label>
      {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
      <div>
        <Button onClick={connect} disabled={busy || !email || password.length < 8} icon={busy ? <Loader size={15} className="animate-spin" /> : <Cloud size={15} />}>
          {haveAccount ? "Sign in" : "Create my account"}
        </Button>
      </div>
    </div>
  );
}

/** First-time setup: the couple's own free Supabase project. Nothing is built into the app. */
function SetupWizard() {
  const [url, setUrl] = useState("");
  const [anon, setAnon] = useState("");
  const [checked, setChecked] = useState<null | "ok" | "needs-sql">(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [haveAccount, setHaveAccount] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [joining, setJoining] = useState(false);

  const check = async () => {
    setError(null);
    setBusy(true);
    try {
      await syncApi.checkProject(url, anon);
      setChecked("ok");
    } catch (e) {
      const m = String(e);
      if (m.includes("setup script")) setChecked("needs-sql");
      else setError(m);
    } finally {
      setBusy(false);
    }
  };
  const connect = async () => {
    setError(null);
    setBusy(true);
    try {
      const r = await syncApi.connectAccount(url, anon, email, password, !haveAccount);
      if (r.needsEmailConfirmation) {
        setConfirm(true);
        setHaveAccount(true);
      } else refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (joining)
    return (
      <div>
        <p className="mb-4 text-[12.5px] text-muted">Your partner already set up Dear Us? Their invite code contains everything needed, no Supabase setup on your side.</p>
        <JoinForm needName={false} onJoined={() => toast("You're connected ♡ Your memories will start syncing.")} />
        <button onClick={() => setJoining(false)} className="mt-4 text-[12px] text-muted hover:text-ink">
          ← Set up our own project instead
        </button>
      </div>
    );

  return (
    <div>
      <div className="mb-5 rounded-xl bg-paper-2 p-4 text-[12.5px] leading-relaxed text-ink-soft">
        <b>Did your partner already set this up?</b> Then you only need their invite code.{" "}
        <button onClick={() => setJoining(true)} className="font-bold text-accent hover:underline">
          Join with an invite code →
        </button>
      </div>
      <ol>
        <Step n={1} title="Create a free Supabase project (one per couple)" done={checked === "ok"}>
          Sign up at <b>supabase.com</b> with your own account, then create a new project. Any name and region works; pick a strong database password and keep it. Dear Us
          never uses anyone else's account.
        </Step>
        <Step n={2} title="Paste the project URL and the public (anon) key" done={checked === "ok"}>
          In your project: <b>Project Settings → API</b> (or <b>Connect</b>). Copy the <b>Project URL</b> and the <b>anon / public</b> key. Never paste the
          <i> service_role</i> key here.
          <div className="mt-3 grid gap-3">
            <Field label="Project URL" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://abcdefgh.supabase.co" />
            <Field label="Anon / public key" value={anon} onChange={(e) => setAnon(e.target.value)} placeholder="eyJhbGciOi…" />
          </div>
        </Step>
        <Step n={3} title="Run the setup script once" done={checked === "ok"}>
          Open <b>SQL Editor → New query</b>, paste the script, press <b>Run</b>. It creates the private tables and the rules that keep each couple's data to themselves.
          <div className="mt-3 flex flex-wrap gap-2">
            <Button variant="secondary" size="sm" icon={<ClipboardCopy size={13} />} onClick={() => copy(setupSql, "Setup script")}>
              Copy setup script
            </Button>
            <Button size="sm" onClick={check} disabled={busy || !url || !anon} icon={busy ? <Loader size={13} className="animate-spin" /> : undefined}>
              Check connection
            </Button>
          </div>
          {checked === "needs-sql" && <p className="mt-2 text-[12px] text-[#9a7448]">Connected, but the setup script hasn't run yet. Run it, then check again.</p>}
          {checked === "ok" && <p className="mt-2 text-[12px] text-[#4f7a55]">All set ✓</p>}
        </Step>
        <Step n={4} title="Create your sign-in for this project">
          Tip: in <b>Authentication → Sign In / Providers → Email</b> you can switch off <b>Confirm email</b> for your private project, or simply confirm the email Supabase sends.
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            <Field label="Your email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="email" />
            <Field label="Password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} hint="At least 8 characters" />
          </div>
          <label className="mt-2 flex items-center gap-2 text-[12px] text-ink-soft">
            <input type="checkbox" checked={haveAccount} onChange={(e) => setHaveAccount(e.target.checked)} className="accent-[var(--accent)]" /> I already created this sign-in
          </label>
          {confirm && <p className="mt-2 rounded-lg bg-gold px-3 py-2 text-[12px] text-ink-soft">Confirm the email Supabase sent you, then press the button again.</p>}
          <div className="mt-3">
            <Button onClick={connect} disabled={busy || checked !== "ok" || !email || !password}>
              {haveAccount ? "Sign in" : "Create sign-in"}
            </Button>
          </div>
        </Step>
      </ol>
      {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
    </div>
  );
}

function PhraseModal({ phrase, onClose, first }: { phrase: string; onClose: () => void; first?: boolean }) {
  const [ok, setOk] = useState(!first);
  return (
    <Modal
      open
      onClose={() => ok && onClose()}
      title="Your recovery phrase"
      description="These 24 words are the key to your shared space. Write them on paper and keep them somewhere safe."
      footer={
        <Button onClick={onClose} disabled={!ok}>
          Done
        </Button>
      }
    >
      <ol className="grid grid-cols-3 gap-2 rounded-xl bg-paper-2 p-4 font-mono text-[12.5px]">
        {phrase.split(" ").map((w, i) => (
          <li key={i} className="flex gap-2">
            <span className="w-5 text-right text-faint">{i + 1}</span>
            {w}
          </li>
        ))}
      </ol>
      <ul className="mt-4 list-disc space-y-1 pl-5 text-[12px] text-muted">
        <li>You'll need it if this PC is lost and you set Dear Us up again from a backup.</li>
        <li>Don't share it by email or chat, and never with anyone but your partner.</li>
      </ul>
      {first && (
        <label className="mt-4 flex items-center gap-2 text-[12.5px] font-semibold text-ink-soft">
          <input type="checkbox" checked={ok} onChange={(e) => setOk(e.target.checked)} className="h-4 w-4 accent-[var(--accent)]" /> I've written it down
        </label>
      )}
    </Modal>
  );
}

function InvitePanel() {
  const [code, setCode] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const make = async () => {
    setBusy(true);
    try {
      setCode(await syncApi.createInvite());
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="rounded-xl border border-dashed border-line p-4">
      <strong className="flex items-center gap-2 text-[13px]">
        <UserPlus size={15} className="text-accent" /> Invite your partner
      </strong>
      <p className="mt-1 text-[12px] text-muted">
        Send the code privately (in person is best). It contains your space's key, works once, and expires in 48 hours. On their PC: <b>Join my partner</b>.
      </p>
      {code ? (
        <div className="mt-3">
          <textarea readOnly value={code} rows={3} className="field-input resize-none font-mono text-[11px]" onFocus={(e) => e.target.select()} />
          <Button className="mt-2" size="sm" icon={<ClipboardCopy size={13} />} onClick={() => copy(code, "Invite code")}>
            Copy invite code
          </Button>
        </div>
      ) : (
        <Button className="mt-3" size="sm" onClick={make} disabled={busy}>
          {busy ? "Creating…" : "Create invite code"}
        </Button>
      )}
    </div>
  );
}

function ConflictList() {
  const { data } = useQuery({ queryKey: syncKeys.conflicts, queryFn: syncApi.conflicts });
  if (!data?.length) return null;
  const preview = (v: string) => {
    try {
      const out: string[] = [];
      const walk = (n: { text?: string; content?: unknown[] }) => {
        if (n.text) out.push(n.text);
        (n.content as { text?: string }[] | undefined)?.forEach(walk);
      };
      walk(JSON.parse(v));
      return out.join(" ").slice(0, 400) || "(empty)";
    } catch {
      return v.slice(0, 400) || "(empty)";
    }
  };
  const resolve = async (c: SyncConflict, useOther: boolean) => {
    try {
      await syncApi.resolve(c.id, useOther);
      queryClient.invalidateQueries();
      toast(useOther ? "Switched to the other version" : "Kept the current version");
    } catch (e) {
      toastError(String(e));
    }
  };
  return (
    <div className="rounded-xl border border-[var(--accent-ring)] bg-accent-soft/40 p-4">
      <strong className="text-[13px]">Two versions to look at</strong>
      <p className="mt-1 mb-3 text-[12px] text-muted">You both changed the same words while apart. Nothing was lost: pick the version to keep.</p>
      <div className="flex flex-col gap-3">
        {data.map((c) => (
          <div key={c.id} className="rounded-lg bg-paper p-3">
            <div className="text-[11px] font-bold text-faint uppercase">{c.title ?? c.entity.replace("_", " ")}</div>
            <div className="mt-2 grid gap-2 sm:grid-cols-2">
              <div className="rounded-lg bg-paper-2 p-2.5 text-[12px]">
                <div className="mb-1 text-[10.5px] font-bold text-muted">Current</div>
                {preview(c.keptValue)}
              </div>
              <div className="rounded-lg bg-paper-2 p-2.5 text-[12px]">
                <div className="mb-1 text-[10.5px] font-bold text-muted">Other version</div>
                {preview(c.otherValue)}
              </div>
            </div>
            <div className="mt-2 flex gap-2">
              <Button size="sm" variant="secondary" onClick={() => resolve(c, false)}>
                Keep current
              </Button>
              <Button size="sm" variant="secondary" onClick={() => resolve(c, true)}>
                Use the other version
              </Button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function SignInAgain() {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <div className="flex items-end gap-2">
      <div className="flex-1">
        <Field label="Password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
      </div>
      <Button
        disabled={busy || !password}
        onClick={async () => {
          setBusy(true);
          try {
            await syncApi.signIn(password);
            refresh();
            toast("Signed in ♡");
          } catch (e) {
            toastError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        Sign in
      </Button>
    </div>
  );
}

function fmtWhen(iso: string | null) {
  if (!iso) return "never";
  const d = new Date(iso);
  const mins = Math.round((Date.now() - d.getTime()) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function Dashboard() {
  const { data: s } = useSyncStatus();
  const store = useIsStore();
  const { data: profile } = useProfile();
  const [phrase, setPhrase] = useState<string | null>(null);
  const [askPhrase, setAskPhrase] = useState(false);
  const [revoke, setRevoke] = useState<string | null>(null);
  const [disconnect, setDisconnect] = useState(false);
  if (!s) return null;
  const d = describe(s, profile?.partner?.displayName);
  const partnerDevices = s.devices.filter((x) => !x.mine && !x.revoked);
  const used = s.relayBytes ?? 0;
  const tile = (label: string, v: number) => (
    <div className="rounded-xl bg-paper-2 px-3 py-2.5">
      <div className="font-serif text-[20px]">{v}</div>
      <div className="text-[10.5px] text-muted">{label}</div>
    </div>
  );
  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-start gap-3 rounded-xl bg-paper-2 p-4">
        <ShieldCheck size={20} className="mt-0.5 shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <strong className="text-[13.5px]">{d.label}</strong>
            <span className="text-[11px] text-faint">Last sync {fmtWhen(s.lastSyncAt)}</span>
          </div>
          <p className="mt-1 text-[12.5px] leading-relaxed text-muted">{d.sentence}</p>
        </div>
        <Button
          size="sm"
          variant="secondary"
          icon={<RefreshCw size={13} />}
          onClick={() => {
            syncApi.now();
            toast("Syncing…");
          }}
        >
          Sync now
        </Button>
      </div>
      {s.state === "signed_out" && <SignInAgain />}
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {tile("waiting to send", s.counts.pendingOps)}
        {tile("waiting for partner", s.counts.awaitingDelivery)}
        {tile("files sending", s.counts.uploadsPending)}
        {tile("files arriving", s.counts.downloadsPending)}
      </div>
      {s.counts.failedUpdates > 0 && (
        <p className="text-[12px] text-[#b0413e]">
          {s.counts.failedUpdates} update{s.counts.failedUpdates > 1 ? "s" : ""} from your partner couldn't be opened and {s.counts.failedUpdates > 1 ? "were" : "was"} left untouched on the relay.
        </p>
      )}
      <ConflictList />
      {!store && (
      <div>
        <div className="mb-1.5 flex justify-between text-[11px] text-muted">
          <span>Temporary cloud space in use</span>
          <span>{(used / 1024 / 1024).toFixed(1)} MB of 1 GB</span>
        </div>
        <div className="h-2 overflow-hidden rounded-full bg-paper-2">
          <div className="h-full rounded-full bg-accent" style={{ width: `${Math.min(100, (used / (1024 * 1024 * 1024)) * 100)}%` }} />
        </div>
        <p className="mt-1.5 text-[11px] text-faint">Files only wait here until your partner's PC has them, then they're deleted.</p>
      </div>
      )}
      {store && <p className="text-[11px] text-faint">The cloud is only a mailbox: everything is deleted from it as soon as your partner's PC has it.</p>}
      <div>
        <strong className="mb-2 block text-[12.5px]">Devices</strong>
        <ul className="divide-y divide-line-soft rounded-xl border border-line">
          {s.devices.map((x) => (
            <li key={x.id} className="flex items-center gap-3 px-3.5 py-2.5 text-[12.5px]">
              <Laptop size={15} className="text-muted" />
              <span className="flex-1">
                {x.name} <span className="text-faint">· {x.mine ? "you" : (profile?.partner?.displayName ?? "partner")}</span>
              </span>
              {x.thisPc && <Pill tone="green">this PC</Pill>}
              {x.revoked ? (
                <Pill tone="gray">removed</Pill>
              ) : (
                !x.thisPc && (
                  <button onClick={() => setRevoke(x.id)} className="text-[11.5px] text-muted hover:text-[#b0413e]">
                    Remove
                  </button>
                )
              )}
            </li>
          ))}
        </ul>
      </div>
      <label className="flex items-center justify-between gap-4 rounded-xl bg-paper-2 px-3.5 py-3">
        <span>
          <strong className="block text-[12.5px]">Direct PC-to-PC transfers</strong>
          <span className="text-[11.5px] text-muted">
            {s.directEnabled
              ? s.directPeer
                ? "Active: your partner's PC is on this network, so photos and videos skip the cloud."
                : "When you're both on the same network, photos and videos go straight between your PCs. Otherwise they use the cloud."
              : "Off: everything goes through the cloud."}
          </span>
          {s.directEnabled && <span className="mt-0.5 block text-[11px] text-faint">Windows may ask once to allow Dear Us on private networks.</span>}
        </span>
        <input
          type="checkbox"
          checked={s.directEnabled}
          onChange={async (e) => {
            await syncApi.setDirect(e.target.checked);
            refresh();
          }}
          className="h-4 w-4 shrink-0 accent-[var(--accent)]"
          aria-label="Direct PC-to-PC transfers"
        />
      </label>
      {partnerDevices.length === 0 && <InvitePanel />}
      <div className="flex flex-wrap gap-2 border-t border-line-soft pt-4">
        <Button variant="secondary" size="sm" icon={<KeyRound size={13} />} onClick={() => setAskPhrase(true)}>
          Show recovery phrase
        </Button>
        {partnerDevices.length > 0 && <InvitePanelButton />}
        <Button variant="ghost" size="sm" icon={<Unplug size={13} />} onClick={() => setDisconnect(true)} className="ml-auto text-muted!">
          Stop syncing on this PC
        </Button>
      </div>
      <p className="text-[11px] text-faint">
        {store ? s.email : `${s.email} · ${s.projectUrl}`}
      </p>

      <ConfirmDialog
        open={askPhrase}
        title="Show the recovery phrase?"
        message="Make sure nobody is looking at your screen."
        confirmLabel="Show it"
        onCancel={() => setAskPhrase(false)}
        onConfirm={async () => {
          setAskPhrase(false);
          try {
            setPhrase(await syncApi.recoveryPhrase());
          } catch (e) {
            toastError(String(e));
          }
        }}
      />
      {phrase && <PhraseModal phrase={phrase} onClose={() => setPhrase(null)} />}
      <ConfirmDialog
        open={!!revoke}
        title="Remove this device?"
        message="It won't receive new updates from now on. Memories already saved on that PC stay there; this can't wipe it remotely."
        confirmLabel="Remove device"
        danger
        onCancel={() => setRevoke(null)}
        onConfirm={async () => {
          const id = revoke!;
          setRevoke(null);
          try {
            await syncApi.revokeDevice(id);
            refresh();
          } catch (e) {
            toastError(String(e));
          }
        }}
      />
      <ConfirmDialog
        open={disconnect}
        title="Stop syncing on this PC?"
        message="Your memories stay on this computer. Nothing is deleted. You can connect again later with a new invite code."
        confirmLabel="Stop syncing"
        danger
        onCancel={() => setDisconnect(false)}
        onConfirm={async () => {
          setDisconnect(false);
          await syncApi.disconnect();
          refresh();
        }}
      />
    </div>
  );
}

function InvitePanelButton() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="secondary" size="sm" icon={<UserPlus size={13} />} onClick={() => setOpen(true)}>
        New invite code
      </Button>
      <Modal open={open} onClose={() => setOpen(false)} title="Invite code">
        <InvitePanel />
      </Modal>
    </>
  );
}

function CreateOrJoin() {
  const [phrase, setPhrase] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [join, setJoin] = useState(false);
  const create = async () => {
    setBusy(true);
    try {
      setPhrase(await syncApi.createSpace());
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-4">
      <p className="text-[12.5px] text-muted">Signed in ✓ One of you creates the shared space; the other joins it with an invite code.</p>
      {join ? (
        <JoinForm needName={false} onJoined={() => toast("You're connected ♡")} />
      ) : (
        <div className="flex flex-wrap gap-2">
          <Button onClick={create} disabled={busy} icon={busy ? <Loader size={15} className="animate-spin" /> : <Cloud size={15} />}>
            Create our shared space
          </Button>
          <Button variant="secondary" onClick={() => setJoin(true)}>
            My partner already did: join
          </Button>
        </div>
      )}
      {phrase && (
        <PhraseModal
          first
          phrase={phrase}
          onClose={() => {
            setPhrase(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

export function SyncSettings() {
  const { data: s, isLoading } = useSyncStatus();
  const store = useIsStore();
  if (!isDesktop) return <p className="text-[12.5px] text-muted">Partner sync works in the desktop app.</p>;
  if (isLoading || !s) return null;
  if (!s.configured) return store ? <CloudSetup /> : <SetupWizard />;
  if (!s.paired && !s.signedIn)
    return (
      <div className="flex flex-col gap-3">
        <p className="text-[12.5px] text-muted">
          {store ? (
            <>
              Sign in again as <b>{s.email}</b>.
            </>
          ) : (
            <>
              Your project is connected. Now sign in as <b>{s.email}</b>. If you just created this sign-in, click the confirmation link Supabase emailed you first.
            </>
          )}
        </p>
        <SignInAgain />
        <button
          onClick={async () => {
            await syncApi.disconnect();
            refresh();
          }}
          className="self-start text-[11.5px] text-muted hover:text-ink"
        >
          {store ? "Use a different email" : "Use a different project or email"}
        </button>
      </div>
    );
  if (!s.paired) return <CreateOrJoin />;
  return <Dashboard />;
}
