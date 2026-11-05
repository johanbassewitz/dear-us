import { CupidIdeas } from "../../cupid/CupidIdeas";
import { useMemo, useState } from "react";
import { CalendarPlus, Dices, Plus, Trash2 } from "lucide-react";
import { api, type IdeaInput } from "../../lib/api";
import { invalidatePlanner, useCreateIdea, useCustomIdeas } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { Field } from "../../components/ui/Field";
import { Modal } from "../../components/ui/Modal";
import { Pill } from "../../components/ui/Pill";
import { toast, toastError } from "../../components/ui/Toast";
import { BUDGETS, CATEGORIES, DURATIONS, MOODS, NO_FILTERS, SETTINGS, allIdeas, filterIdeas, label, pickRandom, type DateIdea, type IdeaFilters } from "./plannerUtils";

const PAGE_SIZE = 24;

function Select({ id, text, value, onChange, options, any }: { id: string; text: string; value: string; onChange: (v: string) => void; options: readonly string[]; any: string }) {
  return (
    <div className="flex min-w-[130px] flex-col gap-1.5">
      <label htmlFor={id} className="text-[11px] font-bold text-ink-soft">
        {text}
      </label>
      <select id={id} className="field-input" value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">{any}</option>
        {options.map((o) => (
          <option key={o} value={o}>
            {label(o)}
          </option>
        ))}
      </select>
    </div>
  );
}

function IdeaMeta({ idea }: { idea: DateIdea }) {
  return (
    <div className="flex flex-wrap gap-1.5">
      <Pill tone="accent">{label(idea.category)}</Pill>
      <Pill tone="gold">{label(idea.budget)}</Pill>
      <Pill tone="gray">{label(idea.duration)}</Pill>
      <Pill tone="green">{label(idea.mood)}</Pill>
    </div>
  );
}

/** Pick a day (optional) and drop the idea into the planner. */
function AddToPlannerDialog({ idea, onClose }: { idea: DateIdea; onClose: () => void }) {
  const [date, setDate] = useState("");
  const [busy, setBusy] = useState(false);
  const add = async () => {
    setBusy(true);
    try {
      await api.plannerCreate({
        title: idea.title,
        description: idea.description,
        date: date || null,
        time: null,
        location: "",
        budgetCents: null,
        currency: "USD",
        status: date ? "planned" : "idea",
        checklist: [],
        ideaRef: idea.id,
      });
      invalidatePlanner();
      toast(date ? "Planned. Can't wait ♡" : "Saved to your planner as an idea ♡");
      onClose();
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open
      onClose={onClose}
      title="Add to our planner"
      description={idea.title}
      width={440}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={add} disabled={busy}>
            {date ? "Plan it" : "Save as an idea"}
          </Button>
        </>
      }
    >
      <Field label="Which day?" type="date" value={date} onChange={(e) => setDate(e.target.value)} hint="Leave empty to keep it as an idea for now." />
    </Modal>
  );
}

function AddIdeaDialog({ onClose }: { onClose: () => void }) {
  const create = useCreateIdea();
  const [f, setF] = useState<IdeaInput>({ title: "", description: "", category: "romantic", budget: "low", duration: "short", setting: "either", mood: "cozy" });
  const [error, setError] = useState<string | null>(null);
  const set = (k: keyof IdeaInput) => (v: string) => setF((s) => ({ ...s, [k]: v }));
  const submit = async () => {
    setError(null);
    if (!f.title.trim()) return setError("Give your idea a name ♡");
    try {
      await create.mutateAsync(f);
      toast("Idea added to your collection ♡");
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };
  const pick = (id: string, text: string, key: keyof IdeaInput, options: readonly string[]) => (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-[11px] font-bold text-ink-soft">
        {text}
      </label>
      <select id={id} className="field-input" value={f[key]} onChange={(e) => set(key)(e.target.value)}>
        {options.map((o) => (
          <option key={o} value={o}>
            {label(o)}
          </option>
        ))}
      </select>
    </div>
  );
  return (
    <Modal
      open
      onClose={onClose}
      title="Add your own idea"
      description="Something only the two of you would think of."
      width={540}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={create.isPending}>
            Add idea
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
        <Field label="Idea name" value={f.title} onChange={(e) => set("title")(e.target.value)} maxLength={200} autoFocus placeholder="Midnight pancakes" />
        <div className="flex flex-col gap-1.5">
          <label htmlFor="idea-desc" className="text-[11px] font-bold text-ink-soft">
            A few words about it
          </label>
          <textarea id="idea-desc" className="field-input min-h-[64px] resize-y" value={f.description} onChange={(e) => set("description")(e.target.value)} maxLength={2000} />
        </div>
        <div className="grid grid-cols-2 gap-4">
          {pick("idea-category", "Category", "category", CATEGORIES)}
          {pick("idea-budget", "Budget", "budget", BUDGETS)}
          {pick("idea-duration", "How long", "duration", DURATIONS)}
          {pick("idea-setting", "Where", "setting", SETTINGS)}
          {pick("idea-mood", "Mood", "mood", MOODS)}
        </div>
      </form>
    </Modal>
  );
}

export function IdeaGenerator() {
  const { data: custom } = useCustomIdeas();
  const [filters, setFilters] = useState<IdeaFilters>(NO_FILTERS);
  const [spot, setSpot] = useState<DateIdea | null>(null);
  const [adding, setAdding] = useState<DateIdea | null>(null);
  const [creating, setCreating] = useState(false);
  const [removing, setRemoving] = useState<DateIdea | null>(null);
  const [limit, setLimit] = useState(PAGE_SIZE);

  const ideas = useMemo(() => allIdeas(custom ?? []), [custom]);
  const matching = useMemo(() => filterIdeas(ideas, filters), [ideas, filters]);
  const set = (k: keyof IdeaFilters) => (v: string) => {
    setFilters((f) => ({ ...f, [k]: v }));
    setSpot(null);
    setLimit(PAGE_SIZE);
  };
  const filtering = Object.values(filters).some(Boolean);
  const shuffle = () => setSpot(pickRandom(matching, spot?.id));

  const remove = async () => {
    if (!removing) return;
    try {
      await api.ideaDelete(removing.id);
      invalidatePlanner();
      toast("Idea removed");
    } catch (e) {
      toastError(String(e));
    }
    setRemoving(null);
  };

  return (
    <div>
      <div className="card mb-5 flex flex-wrap items-end gap-3 p-4" role="group" aria-label="Find a date idea">
        <Select id="f-budget" text="Budget (up to)" value={filters.budget} onChange={set("budget")} options={BUDGETS} any="Any budget" />
        <Select id="f-duration" text="Time we have" value={filters.duration} onChange={set("duration")} options={DURATIONS} any="Any length" />
        <Select id="f-category" text="Category" value={filters.category} onChange={set("category")} options={CATEGORIES} any="Anything" />
        <Select id="f-setting" text="Indoors or out" value={filters.setting} onChange={set("setting")} options={["indoor", "outdoor"]} any="Either" />
        <Select id="f-mood" text="Mood" value={filters.mood} onChange={set("mood")} options={MOODS} any="Any mood" />
        {filtering && (
          <Button variant="ghost" size="sm" onClick={() => { setFilters(NO_FILTERS); setSpot(null); }}>
            Clear
          </Button>
        )}
      </div>
      <CupidIdeas filters={filters as unknown as Record<string, string>} />

      <div className="card relative mb-6 overflow-hidden bg-hero p-6" aria-live="polite">
        <span className="tiny-label flex items-center gap-1.5">
          <Dices size={12} /> Surprise us
        </span>
        {spot ? (
          <div className="mt-2">
            <h3 className="font-serif text-[24px] leading-tight">{spot.title}</h3>
            <p className="mt-1.5 mb-3 max-w-[560px] text-[13px] leading-relaxed text-ink-soft">{spot.description}</p>
            <IdeaMeta idea={spot} />
          </div>
        ) : (
          <p className="mt-2 max-w-[460px] text-[13px] leading-relaxed text-muted">
            {matching.length ? "Can't decide? Let us pick one for tonight, from the ideas that fit what you've chosen above." : "Nothing fits those filters yet. Loosen one of them, or add an idea of your own."}
          </p>
        )}
        <div className="mt-4 flex flex-wrap gap-2">
          <Button icon={<Dices size={15} />} onClick={shuffle} disabled={!matching.length}>
            {spot ? "Another one" : "Surprise us"}
          </Button>
          {spot && (
            <Button variant="secondary" icon={<CalendarPlus size={15} />} onClick={() => setAdding(spot)}>
              Add to planner
            </Button>
          )}
        </div>
      </div>

      <div className="mb-3 flex items-end justify-between gap-3">
        <div>
          <h2 className="section-title">{filtering ? "Ideas that fit" : "All our ideas"}</h2>
          <p className="mt-1 text-[12px] text-muted">
            {matching.length} {matching.length === 1 ? "idea" : "ideas"}
          </p>
        </div>
        <Button variant="secondary" size="sm" icon={<Plus size={13} />} onClick={() => setCreating(true)}>
          Add your own idea
        </Button>
      </div>

      {matching.length ? (
        <>
          <div className="grid gap-3.5 md:grid-cols-2 xl:grid-cols-3">
            {matching.slice(0, limit).map((i) => (
              <article key={i.id} className="card flex flex-col p-4" aria-label={i.title}>
                <div className="mb-2 flex items-start justify-between gap-2">
                  <h3 className="font-serif text-[16.5px] leading-snug">{i.title}</h3>
                  {i.custom && <Pill tone="gray">Ours</Pill>}
                </div>
                <p className="mb-3 line-clamp-3 text-[12px] leading-relaxed text-muted">{i.description}</p>
                <div className="mt-auto">
                  <IdeaMeta idea={i} />
                  <div className="mt-3 flex items-center justify-between border-t border-line-soft pt-3">
                    <Button size="sm" variant="secondary" icon={<CalendarPlus size={13} />} onClick={() => setAdding(i)} aria-label={`Add "${i.title}" to planner`}>
                      Add to planner
                    </Button>
                    {i.custom && (
                      <button onClick={() => setRemoving(i)} aria-label={`Remove "${i.title}"`} title="Remove" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                </div>
              </article>
            ))}
          </div>
          {matching.length > limit && (
            <div className="mt-4 flex justify-center">
              <Button variant="secondary" onClick={() => setLimit((l) => l + PAGE_SIZE)}>
                Show more
              </Button>
            </div>
          )}
        </>
      ) : (
        <div className="card">
          <EmptyState icon={<Dices size={22} />} title="No ideas fit just right" compact>
            Try a bigger budget or more time, or add an idea that is all yours.
          </EmptyState>
        </div>
      )}

      {adding && <AddToPlannerDialog idea={adding} onClose={() => setAdding(null)} />}
      {creating && <AddIdeaDialog onClose={() => setCreating(false)} />}
      <ConfirmDialog
        open={!!removing}
        title="Remove this idea?"
        message={`"${removing?.title ?? ""}" will leave your collection. Plans you already made from it stay.`}
        confirmLabel="Remove"
        danger
        onConfirm={remove}
        onCancel={() => setRemoving(null)}
      />
    </div>
  );
}
