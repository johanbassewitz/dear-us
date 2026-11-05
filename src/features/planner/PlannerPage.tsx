import { useMemo, useState } from "react";
import { ArrowRight, Ban, CalendarHeart, Check, MapPin, Pencil, Plus, RotateCcw, Trash2, Wallet } from "lucide-react";
import { api, type Plan, type PlanStatus } from "../../lib/api";
import { invalidatePlanner, usePlans } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { fmtDate, fmtTime } from "../../lib/format";
import { localIsoDate } from "../../lib/dates";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { Pill } from "../../components/ui/Pill";
import { toast, toastError } from "../../components/ui/Toast";
import { DoneDialog } from "./DoneDialog";
import { IdeaGenerator } from "./IdeaGenerator";
import { PlanEditor } from "./PlanEditor";
import { checklistProgress, formatMoney, groupPlans } from "./plannerUtils";

const STATUS: Record<PlanStatus, { text: string; tone: "accent" | "green" | "gold" | "gray" }> = {
  idea: { text: "Idea", tone: "gold" },
  planned: { text: "Planned", tone: "accent" },
  done: { text: "Done", tone: "green" },
  cancelled: { text: "Cancelled", tone: "gray" },
};

const iconBtn = "grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep";

function PlanCard({ plan, onEdit, onDelete, onStatus }: { plan: Plan; onEdit: () => void; onDelete: () => void; onStatus: (s: PlanStatus) => void }) {
  const openJournal = useNav((s) => s.openJournal);
  const time = fmtTime(plan.time);
  const progress = checklistProgress(plan);

  const toggle = async (itemId: string, done: boolean) => {
    try {
      await api.plannerToggleCheck(plan.id, itemId, done);
      invalidatePlanner();
    } catch (e) {
      toastError(String(e));
    }
  };

  return (
    <article className="card p-4" aria-label={plan.title}>
      <div className="mb-2 flex flex-wrap items-center gap-1.5">
        <Pill tone={STATUS[plan.status].tone}>{STATUS[plan.status].text}</Pill>
        {plan.date && (
          <span className="text-[11.5px] font-semibold text-ink-soft">
            {fmtDate(plan.date)}
            {time ? ` · ${time}` : ""}
          </span>
        )}
      </div>
      <h3 className={`font-serif text-[18px] leading-snug ${plan.status === "cancelled" ? "text-muted line-through" : ""}`}>{plan.title}</h3>
      {plan.description && <p className="mt-1 line-clamp-3 text-[12px] leading-relaxed text-muted">{plan.description}</p>}
      <div className="mt-2.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11.5px] text-muted">
        {plan.location && (
          <span className="inline-flex items-center gap-1">
            <MapPin size={12} /> {plan.location}
          </span>
        )}
        {plan.budgetCents != null && (
          <span className="inline-flex items-center gap-1">
            <Wallet size={12} /> {formatMoney(plan.budgetCents, plan.currency)}
          </span>
        )}
      </div>

      {plan.checklist.length > 0 && (
        <div className="mt-3">
          <div className="mb-1 text-[11px] font-bold text-ink-soft">Checklist · {progress}</div>
          <ul className="flex flex-col gap-1">
            {plan.checklist.map((c) => (
              <li key={c.id}>
                <label className="flex cursor-pointer items-center gap-2 text-[12.5px]">
                  <input type="checkbox" checked={c.done} onChange={(e) => toggle(c.id, e.target.checked)} className="h-4 w-4 accent-[var(--accent)]" />
                  <span className={c.done ? "text-faint line-through" : "text-ink-soft"}>{c.text}</span>
                </label>
              </li>
            ))}
          </ul>
        </div>
      )}

      {plan.linkedEntryId && (
        <button
          onClick={() => openJournal({ mode: "view", id: plan.linkedEntryId! })}
          aria-label={`Open memory: ${plan.linkedEntryTitle ?? plan.title}`}
          className="mt-3 inline-flex items-center gap-1 text-[12px] font-bold text-accent hover:text-accent-deep"
        >
          Open memory <ArrowRight size={12} />
        </button>
      )}

      <div className="mt-3 flex items-center justify-between gap-2 border-t border-line-soft pt-3">
        <div className="flex flex-wrap gap-2">
          {plan.status === "idea" && (
            <Button size="sm" variant="secondary" onClick={onEdit}>
              Pick a day
            </Button>
          )}
          {plan.status === "planned" && (
            <Button size="sm" icon={<Check size={13} />} onClick={() => onStatus("done")}>
              Mark done
            </Button>
          )}
          {plan.status === "done" && !plan.linkedEntryId && (
            <Button size="sm" variant="secondary" onClick={() => onStatus("done")}>
              Keep the memory
            </Button>
          )}
          {plan.status === "planned" && (
            <Button size="sm" variant="secondary" icon={<Ban size={13} />} onClick={() => onStatus("cancelled")}>
              Cancel
            </Button>
          )}
          {(plan.status === "done" || plan.status === "cancelled") && (
            <Button size="sm" variant="secondary" icon={<RotateCcw size={13} />} onClick={() => onStatus("idea")}>
              Back to ideas
            </Button>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button onClick={onEdit} aria-label={`Edit "${plan.title}"`} title="Edit" className={iconBtn}>
            <Pencil size={14} />
          </button>
          <button onClick={onDelete} aria-label={`Delete "${plan.title}"`} title="Delete" className={iconBtn}>
            <Trash2 size={14} />
          </button>
        </div>
      </div>
    </article>
  );
}

function Section({ title, hint, plans, children }: { title: string; hint: string; plans: Plan[]; children: (p: Plan) => React.ReactNode }) {
  if (!plans.length) return null;
  return (
    <section className="mb-7" aria-label={title}>
      <h2 className="section-title">{title}</h2>
      <p className="mt-1 mb-3 text-[12px] text-muted">{hint}</p>
      <div className="grid gap-3.5 md:grid-cols-2 xl:grid-cols-3">{plans.map(children)}</div>
    </section>
  );
}

type Tab = "plans" | "ideas";

export function PlannerPage() {
  const { data: plans, isLoading, error } = usePlans();
  const [tab, setTab] = useState<Tab>("plans");
  const [editing, setEditing] = useState<{ plan: Plan | null } | null>(null);
  const [removing, setRemoving] = useState<Plan | null>(null);
  const [remembering, setRemembering] = useState<Plan | null>(null);

  const today = localIsoDate();
  const groups = useMemo(() => groupPlans(plans ?? [], today), [plans, today]);
  const total = plans?.length ?? 0;

  const setStatus = async (p: Plan, status: PlanStatus) => {
    try {
      // "Keep the memory" on a finished date just reopens the dialog
      if (!(p.status === "done" && status === "done")) {
        await api.plannerSetStatus(p.id, status);
        invalidatePlanner();
      }
      if (status === "done") setRemembering({ ...p, status: "done" });
    } catch (e) {
      toastError(String(e));
    }
  };

  const remove = async () => {
    if (!removing) return;
    const p = removing;
    setRemoving(null);
    try {
      await api.plannerDelete(p.id);
      invalidatePlanner();
      toast(`"${p.title}" removed`, {
        label: "Undo",
        run: () => {
          api.plannerRestore(p.id).then(invalidatePlanner, (e) => toastError(String(e)));
        },
      });
    } catch (e) {
      toastError(String(e));
    }
  };

  const card = (p: Plan) => <PlanCard key={p.id} plan={p} onEdit={() => setEditing({ plan: p })} onDelete={() => setRemoving(p)} onStatus={(s) => setStatus(p, s)} />;
  const tabBtn = (t: Tab, text: string) => (
    <button
      role="tab"
      aria-selected={tab === t}
      onClick={() => setTab(t)}
      className={`rounded-full px-4 py-2 text-[12.5px] font-bold transition-colors ${tab === t ? "bg-accent text-on-accent" : "text-muted hover:text-ink"}`}
    >
      {text}
    </button>
  );

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex items-end justify-between gap-6">
        <div>
          <div className="eyebrow">Date planner</div>
          <h1 className="page-title mt-2">It doesn&apos;t need to be fancy ♡</h1>
          <p className="mt-2 text-[13px] text-muted">Plan the next date, or find a little inspiration.</p>
        </div>
        <Button icon={<Plus size={15} />} onClick={() => setEditing({ plan: null })}>
          Plan a date
        </Button>
      </div>

      <div role="tablist" aria-label="Planner views" className="mb-6 inline-flex gap-1 rounded-full border border-line bg-paper p-1">
        {tabBtn("plans", "Our plans")}
        {tabBtn("ideas", "Date inspiration")}
      </div>

      {tab === "ideas" ? (
        <IdeaGenerator />
      ) : error ? (
        <div className="card p-5 text-[13px] text-muted">Couldn&apos;t load your plans: {String(error)}</div>
      ) : isLoading ? null : total === 0 ? (
        <div className="card">
          <EmptyState
            icon={<CalendarHeart size={22} />}
            title="No dates planned yet"
            action={
              <div className="flex gap-2">
                <Button icon={<Plus size={15} />} onClick={() => setEditing({ plan: null })}>
                  Plan a date
                </Button>
                <Button variant="secondary" onClick={() => setTab("ideas")}>
                  Find an idea
                </Button>
              </div>
            }
          >
            A dinner at home counts. So does a walk. Start with one little plan, or let us suggest something.
          </EmptyState>
        </div>
      ) : (
        <>
          <Section title="Coming up" hint="Soonest first." plans={groups.upcoming}>
            {card}
          </Section>
          <Section title="Ideas on the shelf" hint="No day yet. Pick one when you're both free." plans={groups.ideas}>
            {card}
          </Section>
          <Section title="Looking back" hint="Done, cancelled, or past. Mark old ones done to keep their memory." plans={groups.earlier}>
            {card}
          </Section>
          {!groups.upcoming.length && (
            <p className="text-[12.5px] text-muted">
              Nothing coming up.{" "}
              <button className="font-bold text-accent hover:underline" onClick={() => setTab("ideas")}>
                Find something lovely to do
              </button>
            </p>
          )}
        </>
      )}

      {editing && <PlanEditor plan={editing.plan} onClose={() => setEditing(null)} />}
      {remembering && <DoneDialog plan={remembering} onClose={() => setRemembering(null)} />}
      <ConfirmDialog
        open={!!removing}
        title="Delete this date?"
        message={`"${removing?.title ?? ""}" will be removed from your planner.`}
        confirmLabel="Delete"
        danger
        onConfirm={remove}
        onCancel={() => setRemoving(null)}
      />
    </div>
  );
}
