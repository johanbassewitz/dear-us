import { useState } from "react";
import type { Recurrence, SpecialDate, SpecialInput } from "../../lib/api";
import { useSaveSpecialDate } from "../../lib/queries";
import { Modal } from "../../components/ui/Modal";
import { Button } from "../../components/ui/Button";
import { Field } from "../../components/ui/Field";
import { toast } from "../../components/ui/Toast";

interface Props {
  /** null = a new special date */
  item: SpecialDate | null;
  onClose: () => void;
}

const REMIND_OPTIONS = [
  { value: "", label: "No reminder" },
  { value: "0", label: "On the day" },
  { value: "1", label: "1 day before" },
  { value: "3", label: "3 days before" },
  { value: "7", label: "A week before" },
  { value: "14", label: "2 weeks before" },
  { value: "30", label: "A month before" },
];

export function SpecialEditor({ item, onClose }: Props) {
  const save = useSaveSpecialDate();
  const [title, setTitle] = useState(item?.title ?? "");
  const [date, setDate] = useState(item?.date ?? "");
  const [time, setTime] = useState(item?.time ?? "");
  const [description, setDescription] = useState(item?.description ?? "");
  const [recurrence, setRecurrence] = useState<Recurrence>(item?.recurrence ?? "yearly");
  const [remind, setRemind] = useState(item?.remindDaysBefore == null ? "" : String(item.remindDaysBefore));
  const [error, setError] = useState<string | null>(null);

  const remindOptions =
    remind && !REMIND_OPTIONS.some((o) => o.value === remind) ? [...REMIND_OPTIONS, { value: remind, label: `${remind} days before` }] : REMIND_OPTIONS;

  const submit = async () => {
    setError(null);
    if (!title.trim()) return setError("Give this date a name ♡");
    if (!date) return setError("Pick the date");
    const input: SpecialInput = {
      title,
      date,
      time: time || null,
      description,
      recurrence,
      remindDaysBefore: remind === "" ? null : Number(remind),
      linkedEntryId: item?.linkedEntryId ?? null,
    };
    try {
      await save.mutateAsync({ id: item?.id ?? null, input });
      toast(item ? "Changes saved ♡" : "Added to your calendar ♡");
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={item ? "Edit this date" : "A new special date"}
      description="Anniversaries, birthdays, trips, anything worth counting down to."
      width={540}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={save.isPending}>
            {save.isPending ? "Saving…" : item ? "Save changes" : "Add to our calendar"}
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
        {error && <div className="rounded-xl border border-[#e7b9b4] bg-[#fbeceb] px-4 py-3 text-[12.5px] text-[#93322f]">{error}</div>}
        <Field label="What are we celebrating?" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} placeholder="Our anniversary, Sam's birthday…" autoFocus />
        <div className="grid grid-cols-2 gap-4">
          <Field label="Date" type="date" value={date} onChange={(e) => setDate(e.target.value)} />
          <Field label="Time" type="time" value={time} onChange={(e) => setTime(e.target.value)} hint="Optional" />
          <div className="flex flex-col gap-1.5">
            <label htmlFor="special-repeat" className="text-[11px] font-bold text-ink-soft">
              Repeats
            </label>
            <select id="special-repeat" className="field-input" value={recurrence} onChange={(e) => setRecurrence(e.target.value as Recurrence)}>
              <option value="none">Just once</option>
              <option value="yearly">Every year</option>
              <option value="monthly">Every month</option>
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="special-remind" className="text-[11px] font-bold text-ink-soft">
              Remind me
            </label>
            <select id="special-remind" className="field-input" value={remind} onChange={(e) => setRemind(e.target.value)}>
              {remindOptions.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor="special-notes" className="text-[11px] font-bold text-ink-soft">
            Notes
          </label>
          <textarea id="special-notes" className="field-input min-h-[72px] resize-y" value={description} onChange={(e) => setDescription(e.target.value)} maxLength={2000} placeholder="Plans, ideas, a little reminder to yourself…" />
        </div>
        <p className="text-[11px] text-muted">Reminders show up as a desktop notification while Dear Us is open, if you turn on Desktop reminders in Settings.</p>
      </form>
    </Modal>
  );
}
