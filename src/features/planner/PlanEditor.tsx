import { useState } from "react";
import { Plus, X } from "lucide-react";
import type { ChecklistItem, Plan, PlanInput, PlanStatus } from "../../lib/api";
import { useSavePlan } from "../../lib/queries";
import { Modal } from "../../components/ui/Modal";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { toast } from "../../components/ui/Toast";
import { CURRENCIES, moneyToText, parseMoney } from "./plannerUtils";

interface Props {
  /** null = a brand new plan */
  plan: Plan | null;
  onClose: () => void;
}

let rowSeq = 0;
const rowId = () => `new-${++rowSeq}`;

export function PlanEditor({ plan, onClose }: Props) {
  const save = useSavePlan();
  const [title, setTitle] = useState(plan?.title ?? "");
  const [description, setDescription] = useState(plan?.description ?? "");
  const [date, setDate] = useState(plan?.date ?? "");
  const [time, setTime] = useState(plan?.time ?? "");
  const [location, setLocation] = useState(plan?.location ?? "");
  const [currency, setCurrency] = useState(plan?.currency ?? "USD");
  const [budget, setBudget] = useState(moneyToText(plan?.budgetCents ?? null, plan?.currency ?? "USD"));
  const [status, setStatus] = useState<PlanStatus>(plan?.status ?? "idea");
  const [checklist, setChecklist] = useState<ChecklistItem[]>(plan?.checklist ?? []);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const addDraft = () => {
    const text = draft.trim();
    if (!text) return;
    setChecklist((c) => [...c, { id: rowId(), text, done: false }]);
    setDraft("");
  };

  const submit = async () => {
    setError(null);
    if (!title.trim()) return setError("Give this date a name ♡");
    if (status === "planned" && !date) return setError("Pick a day, or keep it as an idea for now");
    const cents = parseMoney(budget, currency);
    if (cents === undefined) return setError("The budget should be a number, like 40 or 12.50");
    const items = draft.trim() ? [...checklist, { id: rowId(), text: draft.trim(), done: false }] : checklist;
    const input: PlanInput = {
      title,
      description,
      date: date || null,
      time: time || null,
      location,
      budgetCents: cents,
      currency,
      status,
      // new rows get their real id from the app when saved
      checklist: items.map((c) => (c.id.startsWith("new-") ? { ...c, id: "" } : c)),
      ideaRef: plan?.ideaRef ?? null,
    };
    try {
      await save.mutateAsync({ id: plan?.id ?? null, input });
      toast(plan ? "Changes saved ♡" : "Added to your planner ♡");
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={plan ? "Edit this date" : "Plan a date"}
      description="Keep it simple: a name, maybe a day, and whatever you want to remember."
      width={580}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={save.isPending}>
            {save.isPending ? "Saving…" : plan ? "Save changes" : "Add to our planner"}
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        {error && <div role="alert" className="rounded-xl border border-[#e7b9b4] bg-[#fbeceb] px-4 py-3 text-[12.5px] text-[#93322f]">{error}</div>}
        <Field label="What's the plan?" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} placeholder="Pasta night, a walk by the sea…" autoFocus />
        <div className="grid grid-cols-2 gap-4">
          <Field label="Day" type="date" value={date} onChange={(e) => setDate(e.target.value)} hint="Optional until it's planned" />
          <Field label="Time" type="time" value={time} onChange={(e) => setTime(e.target.value)} hint="Optional" />
          <Field label="Where" value={location} onChange={(e) => setLocation(e.target.value)} maxLength={200} placeholder="Home, the old harbour…" />
          <div className="flex flex-col gap-1.5">
            <label htmlFor="plan-status" className="text-[11px] font-bold text-ink-soft">
              Status
            </label>
            <select id="plan-status" className="field-input" value={status} onChange={(e) => setStatus(e.target.value as PlanStatus)}>
              <option value="idea">Just an idea</option>
              <option value="planned">Planned</option>
              <option value="done">Done</option>
              <option value="cancelled">Cancelled</option>
            </select>
          </div>
          <Field label="Budget" inputMode="decimal" value={budget} onChange={(e) => setBudget(e.target.value)} placeholder="0.00" hint="Optional" />
          <div className="flex flex-col gap-1.5">
            <label htmlFor="plan-currency" className="text-[11px] font-bold text-ink-soft">
              Currency
            </label>
            <select id="plan-currency" className="field-input" value={currency} onChange={(e) => setCurrency(e.target.value)}>
              {(CURRENCIES.includes(currency) ? CURRENCIES : [currency, ...CURRENCIES]).map((c) => (
                <option key={c}>{c}</option>
              ))}
            </select>
          </div>
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor="plan-notes" className="text-[11px] font-bold text-ink-soft">
            Notes
          </label>
          <textarea id="plan-notes" className="field-input min-h-[72px] resize-y" value={description} onChange={(e) => setDescription(e.target.value)} maxLength={2000} placeholder="Ideas, reservations, a little surprise…" />
        </div>
        <div className="flex flex-col gap-2">
          <span className="text-[11px] font-bold text-ink-soft">Checklist</span>
          {checklist.map((c, i) => (
            <div key={c.id} className="flex items-center gap-2">
              <input
                type="checkbox"
                aria-label={`Done: ${c.text}`}
                checked={c.done}
                onChange={(e) => setChecklist((l) => l.map((x, j) => (j === i ? { ...x, done: e.target.checked } : x)))}
                className="h-4 w-4 accent-[var(--accent)]"
              />
              <input
                aria-label={`Checklist item ${i + 1}`}
                className="field-input flex-1"
                value={c.text}
                maxLength={200}
                onChange={(e) => setChecklist((l) => l.map((x, j) => (j === i ? { ...x, text: e.target.value } : x)))}
              />
              <button type="button" aria-label={`Remove "${c.text}"`} onClick={() => setChecklist((l) => l.filter((_, j) => j !== i))} className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
                <X size={14} />
              </button>
            </div>
          ))}
          <div className="flex items-center gap-2">
            <input
              aria-label="Add a checklist item"
              className="field-input flex-1"
              value={draft}
              maxLength={200}
              placeholder="Book a table, pack a blanket…"
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addDraft();
                }
              }}
            />
            <Button type="button" variant="secondary" size="sm" icon={<Plus size={13} />} onClick={addDraft}>
              Add
            </Button>
          </div>
        </div>
      </form>
    </Modal>
  );
}
