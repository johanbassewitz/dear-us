import { useState } from "react";
import { CloudRain, Leaf, Lock, Moon, Smile, Sun, Wind, type LucideIcon } from "lucide-react";
import { api, type MoodId } from "../../lib/api";
import { invalidateQuestions, useMoods } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { toast, toastError } from "../../components/ui/Toast";
import { fmtShort } from "../../lib/format";

export const MOODS: { id: MoodId; label: string; icon: LucideIcon }[] = [
  { id: "glowing", label: "Glowing", icon: Sun },
  { id: "happy", label: "Happy", icon: Smile },
  { id: "calm", label: "Calm", icon: Leaf },
  { id: "tired", label: "Tired", icon: Moon },
  { id: "low", label: "A bit low", icon: CloudRain },
  { id: "stressed", label: "Stressed", icon: Wind },
];

/** Private to this PC: never shared with your partner and never synced. */
export function MoodCheckIn({ today }: { today: string }) {
  const { data: moods } = useMoods();
  const current = moods?.find((m) => m.date === today);
  const [mood, setMood] = useState<MoodId | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const shownMood = mood ?? current?.mood ?? null;
  const shownNote = note ?? current?.note ?? "";
  const dirty = !!shownMood && (shownMood !== current?.mood || shownNote.trim() !== (current?.note ?? ""));
  const recent = (moods ?? []).filter((m) => m.date !== today).slice(0, 5);

  const save = async () => {
    if (!shownMood) return;
    try {
      await api.moodSave(today, shownMood, shownNote);
      invalidateQuestions();
      setMood(null);
      setNote(null);
      toast("Noted, just for you");
    } catch (e) {
      toastError(String(e));
    }
  };
  const clear = async () => {
    try {
      await api.moodDelete(today);
      invalidateQuestions();
      setMood(null);
      setNote(null);
    } catch (e) {
      toastError(String(e));
    }
  };

  return (
    <section className="card p-5" aria-labelledby="mood-title">
      <span className="tiny-label">Check in with yourself</span>
      <h2 id="mood-title" className="section-title mt-1">How's your heart feeling today?</h2>
      <p className="mt-1.5 inline-flex items-center gap-1.5 rounded-full bg-paper-2 px-2.5 py-1 text-[11px] font-bold text-muted">
        <Lock size={11} aria-hidden="true" /> Only you can see this
      </p>
      <div role="radiogroup" aria-label="Your mood today" className="mt-3.5 grid grid-cols-3 gap-2">
        {MOODS.map((m) => {
          const on = shownMood === m.id;
          return (
            <button
              key={m.id}
              type="button"
              role="radio"
              aria-checked={on}
              onClick={() => setMood(m.id)}
              className={`flex flex-col items-center gap-1 rounded-xl border px-2 py-2.5 text-[11.5px] font-semibold transition-colors ${
                on ? "border-accent bg-accent-soft text-accent-deep" : "border-line bg-paper text-ink-soft hover:border-accent"
              }`}
            >
              <m.icon size={18} aria-hidden="true" />
              {m.label}
            </button>
          );
        })}
      </div>
      <textarea
        className="field-input mt-3 resize-y"
        rows={2}
        value={shownNote}
        onChange={(e) => setNote(e.target.value)}
        aria-label="A private note about today (optional)"
        placeholder="A private note, if you'd like…"
      />
      <div className="mt-3 flex items-center justify-between gap-2">
        {current ? (
          <button type="button" onClick={clear} className="text-[11.5px] font-semibold text-muted hover:text-ink">
            Clear today
          </button>
        ) : (
          <span />
        )}
        <Button size="sm" disabled={!dirty} onClick={save}>
          {current ? "Update check-in" : "Save check-in"}
        </Button>
      </div>
      {recent.length > 0 && (
        <ul className="mt-4 flex flex-col gap-1.5 border-t border-line pt-3" aria-label="Your recent check-ins">
          {recent.map((m) => (
            <li key={m.date} className="flex items-baseline gap-2 text-[11.5px] text-muted">
              <span className="w-12 shrink-0 text-faint">{fmtShort(m.date)}</span>
              <span className="font-semibold text-ink-soft">{MOODS.find((x) => x.id === m.mood)?.label}</span>
              {m.note && <span className="truncate">· {m.note}</span>}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
