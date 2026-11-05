import { useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, Gift, Pencil, Plus, Trash2 } from "lucide-react";
import { api, type SpecialDate } from "../../lib/api";
import { invalidateSpecial, useProfile, useSpecialDates } from "../../lib/queries";
import { useNav } from "../../app/navigation";
import { fmtDate, fmtTime } from "../../lib/format";
import { localIsoDate } from "../../lib/dates";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { Pill } from "../../components/ui/Pill";
import { toast, toastError } from "../../components/ui/Toast";
import { SpecialEditor } from "./SpecialEditor";
import { countdownLabel, daysInMonth, monthMarks, pastMilestones, repeatLabel, timeSince, upcomingOccasions, type Occasion } from "./occurrence";

const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

const REPEAT_TEXT = { none: "", yearly: "Every year", monthly: "Every month" } as const;

function whenText(o: Occasion): string {
  const t = fmtTime(o.time);
  return `${fmtDate(o.date)}${t ? ` · ${t}` : ""}`;
}

function heroSentence(o: Occasion): string {
  const rep = repeatLabel(o.recurrence, o.count);
  if (!rep) return "";
  return o.recurrence === "yearly" ? `The ${rep.replace(" year", "")} one ♡` : rep;
}

function MonthCalendar({ specials, start, today }: { specials: SpecialDate[]; start: string | null; today: string }) {
  const [cursor, setCursor] = useState({ y: +today.slice(0, 4), m: +today.slice(5, 7) });
  const marks = useMemo(() => monthMarks(specials, start, cursor.y, cursor.m), [specials, start, cursor]);
  const lead = new Date(cursor.y, cursor.m - 1, 1).getDay();
  const total = daysInMonth(cursor.y, cursor.m);
  const cells = [...Array(lead).fill(null), ...Array.from({ length: total }, (_, i) => i + 1)];
  const step = (n: number) =>
    setCursor((c) => {
      const idx = c.y * 12 + (c.m - 1) + n;
      return { y: Math.floor(idx / 12), m: (idx % 12) + 1 };
    });
  const isToday = (d: number) => today === `${cursor.y}-${String(cursor.m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
  return (
    <section className="card p-5" aria-label="Month calendar">
      <div className="mb-3 flex items-center justify-between">
        <h2 className="section-title">
          {MONTHS[cursor.m - 1]} {cursor.y}
        </h2>
        <div className="flex gap-1">
          <button onClick={() => step(-1)} aria-label="Previous month" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
            <ChevronLeft size={16} />
          </button>
          <button onClick={() => step(1)} aria-label="Next month" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
            <ChevronRight size={16} />
          </button>
        </div>
      </div>
      <div className="grid grid-cols-7 gap-1 text-center">
        {WEEKDAYS.map((w) => (
          <div key={w} className="tiny-label py-1">
            {w}
          </div>
        ))}
        {cells.map((d, i) =>
          d === null ? (
            <div key={`b${i}`} />
          ) : (
            <div
              key={d}
              title={marks.get(d)?.join(", ")}
              aria-label={marks.get(d) ? `${MONTHS[cursor.m - 1]} ${d}: ${marks.get(d)!.join(", ")}` : undefined}
              className={`grid aspect-square place-items-center rounded-lg text-[12px] ${
                marks.has(d) ? "bg-accent font-bold text-on-accent" : isToday(d) ? "border border-accent text-accent-deep" : "text-ink-soft"
              }`}
            >
              {d}
            </div>
          ),
        )}
      </div>
    </section>
  );
}

export function SpecialPage() {
  const { data, isLoading, error } = useSpecialDates();
  const { data: profile } = useProfile();
  const openJournal = useNav((s) => s.openJournal);
  const [editing, setEditing] = useState<{ item: SpecialDate | null } | null>(null);
  const [removing, setRemoving] = useState<SpecialDate | null>(null);

  const today = localIsoDate();
  const start = profile?.space?.startDate ?? null;
  const specials = useMemo(() => data ?? [], [data]);
  const upcoming = useMemo(() => upcomingOccasions(specials, start, today), [specials, start, today]);
  const past = useMemo(() => pastMilestones(specials, today), [specials, today]);
  const byId = useMemo(() => new Map(specials.map((s) => [s.id, s])), [specials]);
  const next = upcoming[0];

  const remove = async (item: SpecialDate) => {
    setRemoving(null);
    try {
      await api.specialDelete(item.id);
      invalidateSpecial();
      toast("Removed from your calendar", {
        label: "Undo",
        run: () =>
          api
            .specialRestore(item.id)
            .then(invalidateSpecial)
            .catch((e) => toastError(String(e))),
      });
    } catch (e) {
      toastError(String(e));
    }
  };

  const actions = (o: { id: string | null; title: string }) => {
    const item = o.id ? byId.get(o.id) : undefined;
    if (!item) return null;
    return (
      <div className="flex items-center gap-1">
        <button onClick={() => setEditing({ item })} aria-label={`Edit "${item.title}"`} title="Edit" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
          <Pencil size={14} />
        </button>
        <button onClick={() => setRemoving(item)} aria-label={`Delete "${item.title}"`} title="Delete" className="grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-accent-soft hover:text-accent-deep">
          <Trash2 size={14} />
        </button>
      </div>
    );
  };

  const nothing = !isLoading && !error && upcoming.length === 0 && past.length === 0;

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="eyebrow">Mark your calendars</div>
          <h1 className="page-title mt-2">Special dates</h1>
          <p className="mt-2 text-[13px] text-muted">Anniversaries, birthdays and trips, and how long until the next one.</p>
        </div>
        <Button icon={<Plus size={15} />} onClick={() => setEditing({ item: null })}>
          Add a date
        </Button>
      </div>

      {error ? (
        <div className="card">
          <EmptyState title="Couldn't load your special dates">{String(error)}</EmptyState>
        </div>
      ) : nothing ? (
        <div className="card">
          <EmptyState
            icon={<Gift size={22} />}
            title="Nothing on the calendar yet"
            action={
              <Button icon={<Plus size={15} />} onClick={() => setEditing({ item: null })}>
                Add your first date
              </Button>
            }
          >
            Add your anniversary, a birthday or a trip, and the countdown will appear here and on Home.
            {!start && " Set your start date in Settings and your anniversary shows up on its own."}
          </EmptyState>
        </div>
      ) : (
        <div className="grid items-start gap-5 lg:grid-cols-[1fr_340px]">
          <div className="flex min-w-0 flex-col gap-5">
            {next && (
              <section className="card relative overflow-hidden bg-hero! p-7" aria-label="Next special date">
                <span className="tiny-label flex items-center gap-1.5">
                  <Gift size={12} /> Next up
                </span>
                <div className="mt-3 flex flex-wrap items-baseline gap-x-3 gap-y-1">
                  <span className="font-serif text-[56px] leading-none">{next.days === 0 ? "Today" : next.days === 1 ? "Tomorrow" : next.days}</span>
                  {next.days > 1 && <span className="font-serif text-[22px] text-ink-soft">days until</span>}
                  {next.days <= 1 && <span className="font-serif text-[22px] text-ink-soft">is</span>}
                </div>
                <h2 className="mt-1 font-serif text-[26px] leading-tight">{next.title}</h2>
                <p className="mt-2 text-[12.5px] text-muted">
                  {whenText(next)}
                  {heroSentence(next) && ` · ${heroSentence(next)}`}
                </p>
                {next.computed && <p className="mt-1 text-[11.5px] text-muted">Worked out from the start date in your Settings.</p>}
              </section>
            )}

            {upcoming.length > 0 && (
              <section aria-label="Coming up">
                <h2 className="section-title mb-3">Coming up</h2>
                <ul className="card divide-y divide-line-soft">
                  {upcoming.map((o) => {
                    const rep = repeatLabel(o.recurrence, o.count);
                    const item = o.id ? byId.get(o.id) : undefined;
                    return (
                      <li key={o.key} className="flex items-center gap-4 p-4">
                        <div className="w-[76px] shrink-0 rounded-xl bg-accent-soft py-2 text-center text-accent-deep">
                          <div className="font-serif text-[18px] leading-tight">{countdownLabel(o.days)}</div>
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-serif text-[16px]">{o.title}</div>
                          <div className="text-[11.5px] text-muted">{whenText(o)}</div>
                          <div className="mt-1.5 flex flex-wrap gap-1.5">
                            {o.recurrence !== "none" && <Pill tone="gray">{REPEAT_TEXT[o.recurrence]}</Pill>}
                            {rep && <Pill tone="accent">{rep}</Pill>}
                            {o.remindDaysBefore !== null && <Pill tone="gold">{o.remindDaysBefore === 0 ? "Reminder on the day" : `Reminder ${o.remindDaysBefore}d before`}</Pill>}
                            {o.computed && <Pill tone="gray">From your start date</Pill>}
                          </div>
                          {item?.description && <p className="mt-1.5 line-clamp-2 text-[12px] text-muted">{item.description}</p>}
                          {item?.linkedEntryId && (
                            <button onClick={() => openJournal({ mode: "view", id: item.linkedEntryId! })} className="mt-1.5 text-[12px] font-bold text-accent hover:text-accent-deep">
                              Open memory: {item.linkedEntryTitle}
                            </button>
                          )}
                        </div>
                        {actions(o)}
                      </li>
                    );
                  })}
                </ul>
              </section>
            )}

            {past.length > 0 && (
              <section aria-label="Time since">
                <h2 className="section-title mb-3">Time since</h2>
                <ul className="card divide-y divide-line-soft">
                  {past.map((s) => (
                    <li key={s.id} className="flex items-center gap-4 p-4">
                      <div className="min-w-0 flex-1">
                        <div className="truncate font-serif text-[16px]">{s.title}</div>
                        <div className="text-[11.5px] text-muted">{fmtDate(s.date)}</div>
                      </div>
                      <div className="text-right font-serif text-[15px] text-accent-deep">{timeSince(s.date, today)} ago</div>
                      {actions(s)}
                    </li>
                  ))}
                </ul>
              </section>
            )}
          </div>

          <MonthCalendar specials={specials} start={start} today={today} />
        </div>
      )}

      {editing && <SpecialEditor item={editing.item} onClose={() => setEditing(null)} />}
      <ConfirmDialog
        open={!!removing}
        title="Remove this date?"
        message={removing ? `"${removing.title}" will leave your calendar. You can undo it right after.` : ""}
        confirmLabel="Remove"
        danger
        onConfirm={() => removing && remove(removing)}
        onCancel={() => setRemoving(null)}
      />
    </div>
  );
}
