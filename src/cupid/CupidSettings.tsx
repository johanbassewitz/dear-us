import { useState } from "react";
import { KeyRound, Loader, ShieldAlert, Trash2 } from "lucide-react";
import { cupidApi, cupidKeys, useCupidStatus } from "./cupidApi";
import { queryClient } from "../lib/queries";
import { isDesktop } from "../lib/api";
import { Button } from "../components/ui/Button";
import { Field } from "../components/ui/Field";
import { toast, toastError } from "../components/ui/Toast";

function Toggle({ label, desc, on, onChange }: { label: string; desc: string; on: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex items-center justify-between gap-4 py-1">
      <span>
        <strong className="block text-[12.5px]">{label}</strong>
        <span className="text-[11.5px] text-muted">{desc}</span>
      </span>
      <input type="checkbox" checked={on} onChange={(e) => onChange(e.target.checked)} className="h-4 w-4 shrink-0 accent-[var(--accent)]" />
    </label>
  );
}

/** Each person adds their OWN free Gemini key. Nothing is built into the app. */
export function CupidSettings() {
  const { data: st } = useCupidStatus();
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  if (!isDesktop) return <p className="text-[12.5px] text-muted">Cupid's brain works in the desktop app.</p>;
  if (!st) return null;
  const update = (s: unknown) => queryClient.setQueryData(cupidKeys.status, s);
  const opt = async (name: string, value: boolean | string) => update(await cupidApi.setOption(name, typeof value === "boolean" ? (value ? "1" : "0") : value));

  if (!st.privacyAck)
    return (
      <div className="flex flex-col gap-3">
        <div className="flex gap-3 rounded-xl bg-gold p-4 text-[12.5px] leading-relaxed text-ink-soft">
          <ShieldAlert size={18} className="mt-0.5 shrink-0 text-[#9a7448]" />
          <div>
            <strong className="block">Before you turn Cupid's brain on</strong>
            Cupid uses Google Gemini with <b>your own free API key</b>. On Gemini's free tier, Google may use what you send to improve its products, and people at
            Google may read some of it. So Cupid only ever sees what you type to him, a shortlist of questions, or your date-idea filters. Never your journal,
            letters, time capsules, photos or videos.
          </div>
        </div>
        <div>
          <Button onClick={() => opt("privacyAck", true)}>I understand, continue</Button>
        </div>
      </div>
    );

  if (!st.hasKey)
    return (
      <div className="flex flex-col gap-3">
        <p className="text-[12.5px] text-muted">
          Get a free key at <b>aistudio.google.com</b> → <b>Get API key</b>, using your own Google account. Free-tier availability depends on your country. The key is kept in
          Windows Credential Manager on this PC only, and never synced.
        </p>
        <Field
          label="Gemini API key"
          type="password"
          value={key}
          onChange={(e) => {
            setKey(e.target.value);
            setKeyError(null);
          }}
          autoComplete="off"
          placeholder="Paste your key"
          error={keyError}
          hint={busy ? "Checking the key with Google…" : undefined}
        />
        <div className="flex gap-2">
          <Button
            disabled={busy || key.trim().length < 20}
            icon={busy ? <Loader size={14} className="animate-spin" /> : <KeyRound size={14} />}
            onClick={async () => {
              setBusy(true);
              setKeyError(null);
              try {
                update(await cupidApi.setKey(key));
                setKey("");
                toast("Cupid is awake ♡");
              } catch (e) {
                setKeyError(String(e).replace(/^Error:\s*/, ""));
              } finally {
                setBusy(false);
              }
            }}
          >
            {busy ? "Checking…" : "Save & test key"}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => opt("privacyAck", false)}>
            Not now
          </Button>
        </div>
      </div>
    );

  return (
    <div className="flex flex-col gap-3">
      <p className="rounded-lg bg-sage px-3 py-2 text-[12px] text-[#3f6545]">Cupid's brain is on, with your own Gemini key. Click Cupid → Ask Cupid to chat.</p>
      <Toggle label="Cupid picks the daily question" desc="From a shortlist of our questions. Both PCs still see the same one." on={st.dailyQuestion} onChange={(v) => opt("dailyQuestion", v)} />
      <Toggle label="Date ideas from Cupid" desc="In the Date planner → Date inspiration." on={st.dateIdeas} onChange={(v) => opt("dateIdeas", v)} />
      <Toggle
        label="Let Cupid see open bucket-list titles"
        desc="Only the titles, to inspire date ideas. Off by default."
        on={st.shareBucketTitles}
        onChange={(v) => opt("shareBucketTitles", v)}
      />
      <div className="flex flex-wrap items-end gap-2 border-t border-line-soft pt-3">
        <div className="min-w-[200px] flex-1">
          <Field label="Model" defaultValue={st.model} onBlur={(e) => e.target.value !== st.model && opt("model", e.target.value).catch((err) => toastError(String(err)))} hint="Default: gemini-flash-latest" />
        </div>
        <Button
          variant="secondary"
          size="sm"
          icon={<Trash2 size={13} />}
          onClick={async () => {
            update(await cupidApi.removeKey());
            toast("Key removed from this PC");
          }}
        >
          Remove key
        </Button>
      </div>
    </div>
  );
}
