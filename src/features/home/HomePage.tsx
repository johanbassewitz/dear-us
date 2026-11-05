import type { ReactNode } from "react";
import {
  ArrowRight,
  BookHeart,
  CalendarHeart,
  Gift,
  Heart,
  Image as ImageIcon,
  ListChecks,
  Mail,
  MessageCircleHeart,
  Plus,
  Sparkles,
  Star,
  X,
} from "lucide-react";
import { useBucket, useDailyQuestion, useDashboard, useEntries, useOnThisDay, usePlans, useProfile, useSpecialDates } from "../../lib/queries";
import { nextPlan } from "../planner/plannerUtils";
import { upcomingOccasions } from "../special/occurrence";
import { usePrefs } from "../../lib/settings";
import { fmtDate, fmtTime, yearsAgo } from "../../lib/format";
import { MediaThumb } from "../../components/media/MediaThumb";
import { EntryCard } from "../journal/EntryCard";
import { useNav } from "../../app/navigation";
import type { PageId } from "../../app/nav";
import { daysTogether, greetingFor, localIsoDate, longDate, togetherDuration } from "../../lib/dates";
import { questionById, questionOfTheDay } from "../../lib/questions";
import { Button } from "../../components/ui/Button";
import { EmptyState } from "../../components/ui/EmptyState";
import { sortColumn } from "../bucket/bucketUtils";

function LinkButton({ to, children }: { to: PageId; children: ReactNode }) {
  const go = useNav((s) => s.go);
  return (
    <button onClick={() => go(to)} className="inline-flex items-center gap-1.5 text-[12px] font-bold text-accent hover:text-accent-deep">
      {children} <ArrowRight size={13} />
    </button>
  );
}

function Stat({ label, icon, value, sub }: { label: string; icon: ReactNode; value: ReactNode; sub: ReactNode }) {
  return (
    <div className="card min-h-[108px] px-5 py-4">
      <div className="flex items-center justify-between">
        <span className="tiny-label">{label}</span>
        <span className="text-accent">{icon}</span>
      </div>
      <div className="mt-2 font-serif text-[30px] leading-none tracking-tight">{value}</div>
      <div className="mt-1.5 text-[11px] text-muted">{sub}</div>
    </div>
  );
}

const QUICK_ACTIONS: { to: PageId; label: string; icon: ReactNode }[] = [
  { to: "journal", label: "Add memory", icon: <BookHeart size={15} /> },
  { to: "bucket", label: "Add bucket-list item", icon: <ListChecks size={15} /> },
  { to: "letters", label: "Write a letter", icon: <Mail size={15} /> },
  { to: "planner", label: "Plan a date", icon: <CalendarHeart size={15} /> },
  { to: "questions", label: "Answer a question", icon: <MessageCircleHeart size={15} /> },
];

/** Compact "Our next date": the soonest planned date, or a gentle nudge toward the planner. */
function NextDateCard({ today }: { today: string }) {
  const { data: plans } = usePlans();
  const next = nextPlan(plans ?? [], today);
  const time = fmtTime(next?.time ?? null);
  return (
    <div className="card p-5">
      <span className="tiny-label flex items-center gap-1.5">
        <CalendarHeart size={12} /> Our next date
      </span>
      {next ? (
        <>
          <h3 className="mt-2 mb-1 truncate font-serif text-[19px]">{next.title}</h3>
          <p className="mb-2.5 text-[12px] text-muted">
            {next.date === today ? "Today" : fmtDate(next.date!)}
            {time ? ` · ${time}` : ""}
            {next.location ? ` · ${next.location}` : ""}
          </p>
          <LinkButton to="planner">Open the planner</LinkButton>
        </>
      ) : (
        <>
          <p className="mt-2 mb-2.5 text-[12px] leading-relaxed text-muted">No date planned yet. Even a walk and a coffee counts.</p>
          <LinkButton to="planner">Plan something lovely</LinkButton>
        </>
      )}
    </div>
  );
}

/** "Dream a little bigger": up to 3 open bucket-list items, or a gentle invitation. */
function BucketPreview() {
  const { data: items } = useBucket();
  const open = (items ?? []).filter((i) => i.status !== "done");
  const next = [...sortColumn(open.filter((i) => i.status === "planned"), "planned"), ...sortColumn(open.filter((i) => i.status === "want"), "want")].slice(0, 3);
  return (
    <div className="card relative min-h-[148px] overflow-hidden p-5">
      <div className="grid h-9 w-9 place-items-center rounded-[11px] bg-blush text-accent">
        <ListChecks size={18} />
      </div>
      <h3 className="mt-3 mb-1 font-serif text-[18px]">Dream a little bigger</h3>
      {next.length ? (
        <ul className="mb-3 flex flex-col gap-1 text-[12px] text-ink-soft">
          {next.map((i) => (
            <li key={i.id} className="flex items-center gap-2">
              <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" aria-hidden="true" />
              <span className="truncate">{i.title}</span>
              {i.status === "planned" && <span className="shrink-0 text-[10.5px] text-muted">planned</span>}
            </li>
          ))}
        </ul>
      ) : (
        <p className="mb-3 max-w-[260px] text-[11.5px] leading-relaxed text-muted">
          {items?.length ? "Everything on your list is done. Time to dream up something new?" : "All the things you want to try, see, eat and experience together."}
        </p>
      )}
      <LinkButton to="bucket">Our bucket list</LinkButton>
    </div>
  );
}

export function HomePage() {
  const go = useNav((s) => s.go);
  const { data: profile } = useProfile();
  const { data: summary } = useDashboard();
  const now = new Date();
  const start = profile?.space?.startDate;
  const days = daysTogether(start, now);
  const duration = togetherDuration(start, now);
  const today = localIsoDate(now);
  const picked = questionOfTheDay(today, profile?.space?.id);
  // the day's question as recorded on this device, so Home and the Questions page always agree
  const { data: daily } = useDailyQuestion(today, picked.id);
  const dailyId = daily?.questionId ?? picked.id;
  const question = questionById(dailyId)?.text ?? picked.text;
  const openJournal = useNav((s) => s.openJournal);
  const { data: specials } = useSpecialDates();
  const nextMoment = upcomingOccasions(specials ?? [], start, today)[0];
  const { data: recent } = useEntries({ limit: 3 });
  const { data: onThisDay } = useOnThisDay(today);
  const dismissed = usePrefs((s) => s.prefs["home.dismissed"]);
  const setPref = usePrefs((s) => s.set);
  const year = today.slice(0, 4);
  const memoriesToday = (onThisDay ?? []).filter((e) => !dismissed.includes(`${e.id}:${year}`));
  const otd = memoriesToday[0];
  const dismiss = (id: string) =>
    // keep the list short: only this year's dismissals matter
    setPref("home.dismissed", [...dismissed.filter((d) => d.endsWith(`:${year}`)), `${id}:${year}`]);

  return (
    <div className="animate-fade-up">
      <div className="mb-7 flex items-end justify-between gap-6">
        <div>
          <div className="eyebrow">{longDate(now)}</div>
          <h1 className="page-title mt-2">Welcome to your little world ♡</h1>
          <p className="mt-2 text-[13px] text-muted">Our little world, one memory at a time.</p>
        </div>
      </div>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.8fr)_minmax(290px,0.95fr)]">
        <div className="min-w-0">
          {/* hero */}
          <div className="relative grid min-h-[280px] overflow-hidden rounded-[21px] border border-[var(--hero-line)] bg-hero md:grid-cols-[1.05fr_0.95fr]">
            <div className="z-10 flex flex-col items-start justify-center px-8 py-8">
              <div className="mb-2 font-hand text-[22px] text-accent">
                ✦ {greetingFor(now)}
                {profile?.me ? `, ${profile.me.displayName}` : ""}
              </div>
              <h2 className="mb-4 max-w-[340px] font-serif text-[clamp(30px,3.1vw,42px)] leading-[1.04] tracking-[-0.04em]">
                Every love story is beautiful. <i className="font-normal text-accent">Ours is my favorite.</i>
              </h2>
              <p className="mb-5 max-w-[320px] text-[12px] leading-relaxed text-muted">
                Collect the memories, dreams, letters and tiny everyday things that make up your forever.
              </p>
              <Button icon={<Plus size={15} />} onClick={() => openJournal({ mode: "edit", id: null })}>
                Add a memory
              </Button>
            </div>
            <div className="relative hidden items-center justify-center md:flex" aria-hidden="true">
              <div className="absolute inset-0 bg-[radial-gradient(circle_at_70%_40%,color-mix(in_srgb,var(--accent)_22%,transparent),transparent_60%)]" />
              <div className="relative h-[170px] w-[150px] rotate-[7deg] border border-line bg-paper p-2.5 pb-8 shadow-[0_13px_22px_rgba(80,50,50,0.15)]">
                <div className="grid h-full w-full place-items-center bg-blush text-accent">
                  <Heart size={34} strokeWidth={1.4} />
                </div>
                <div className="absolute right-0 bottom-1.5 left-0 text-center font-hand text-[17px] text-muted">your first photo?</div>
              </div>
              <div className="absolute top-5 right-6 rotate-[10deg] font-hand text-[19px] text-accent">just us ♡</div>
            </div>
          </div>

          {/* stats */}
          <div className="mt-4 grid grid-cols-3 gap-3.5">
            <Stat
              label="Our days together"
              icon={<Heart size={17} />}
              value={days !== null ? days.toLocaleString() : "—"}
              sub={
                days !== null ? (
                  duration ?? "and counting…"
                ) : (
                  <button onClick={() => go("settings")} className="font-semibold text-accent hover:underline">
                    Add your start date
                  </button>
                )
              }
            />
            <Stat label="Memories saved" icon={<ImageIcon size={17} />} value={summary?.memories ?? 0} sub="little moments" />
            <Stat
              label="Dreams together"
              icon={<Star size={17} />}
              value={
                <>
                  {summary?.bucketDone ?? 0}
                  <span className="text-[17px] text-faint"> / {summary?.bucketTotal ?? 0}</span>
                </>
              }
              sub="bucket list completed"
            />
          </div>
        </div>

        {/* right column */}
        <div className="flex flex-col gap-4">
          <div className="card relative overflow-hidden bg-gold! p-5">
            <span className="tiny-label flex items-center gap-1.5">
              <Gift size={12} /> Next special moment
            </span>
            {nextMoment ? (
              <>
                <h3 className="mt-2 mb-1.5 font-serif text-[21px]">
                  {nextMoment.days === 0 ? "Today: " : nextMoment.days === 1 ? "Tomorrow: " : `${nextMoment.days} days until `}
                  {nextMoment.title}
                </h3>
                <p className="mb-3 text-[12px] leading-relaxed text-muted">{fmtDate(nextMoment.date)}</p>
                <LinkButton to="special">See all special dates</LinkButton>
              </>
            ) : (
              <>
                <h3 className="mt-2 mb-1.5 font-serif text-[21px]">Nothing on the calendar yet</h3>
                <p className="mb-3 text-[12px] leading-relaxed text-muted">
                  Add your anniversary, birthdays and trips, and the countdown will appear here.
                </p>
                <LinkButton to="special">Add a special date</LinkButton>
              </>
            )}
          </div>

          <NextDateCard today={today} />

          {otd ? (
            <div className="card relative grid grid-cols-[110px_1fr] items-center gap-4 p-3">
              <button onClick={() => openJournal({ mode: "view", id: otd.id })} className="h-[110px] overflow-hidden rounded-xl" aria-label={`Open ${otd.title}`}>
                <MediaThumb media={otd.cover} />
              </button>
              <div className="min-w-0 pr-5">
                <span className="tiny-label flex items-center gap-1.5">
                  <Sparkles size={12} /> On this day
                </span>
                <h3 className="mt-1 truncate font-serif text-[17px]">{otd.title}</h3>
                <p className="text-[11.5px] text-muted">
                  {yearsAgo(otd.entryDate, today) === 1 ? "One year ago" : `${yearsAgo(otd.entryDate, today)} years ago`} · {fmtDate(otd.entryDate)}
                </p>
                <button onClick={() => openJournal({ mode: "view", id: otd.id })} className="mt-1.5 text-[11.5px] font-bold text-accent hover:text-accent-deep">
                  Remember this? →
                </button>
                {memoriesToday.length > 1 && <span className="ml-2 text-[11px] text-faint">+{memoriesToday.length - 1} more</span>}
              </div>
              <button onClick={() => dismiss(otd.id)} aria-label="Dismiss" title="Not today" className="absolute top-2.5 right-2.5 rounded p-1 text-faint hover:text-ink">
                <X size={13} />
              </button>
            </div>
          ) : (
            <div className="card p-5">
              <span className="tiny-label flex items-center gap-1.5">
                <Sparkles size={12} /> On this day
              </span>
              <p className="mt-2 text-[12px] leading-relaxed text-muted">
                Memories from this date in past years will come back to visit you here.
              </p>
            </div>
          )}

          <div className="card p-5">
            <span className="tiny-label">A little question for today</span>
            <h3 className="my-2.5 font-serif text-[19px] leading-snug">{question}</h3>
            <LinkButton to="questions">Answer together</LinkButton>
          </div>
        </div>
      </div>

      {/* quick actions */}
      <div className="mt-7 flex flex-wrap gap-2">
        {QUICK_ACTIONS.map((a) => (
          <Button
            key={a.label}
            variant="secondary"
            size="sm"
            icon={a.icon}
            onClick={() => (a.to === "journal" ? openJournal({ mode: "edit", id: null }) : go(a.to))}
          >
            {a.label}
          </Button>
        ))}
      </div>

      {/* recent memories */}
      <div className="mt-8 mb-4 flex items-end justify-between">
        <div>
          <h2 className="section-title">Recently in our story</h2>
          <p className="mt-1 text-[12px] text-muted">Little moments worth remembering forever.</p>
        </div>
        <LinkButton to="journal">All memories</LinkButton>
      </div>
      {recent?.length ? (
        <div className="grid gap-4 md:grid-cols-3">
          {recent.map((e) => (
            <EntryCard key={e.id} e={e} />
          ))}
        </div>
      ) : (
        <div className="card">
          <EmptyState
            icon={<BookHeart size={22} />}
            title="Your story starts here"
            action={
              <Button icon={<Plus size={15} />} onClick={() => openJournal({ mode: "edit", id: null })}>
                Write your first memory
              </Button>
            }
          >
            Your first date, a lazy Sunday, a silly inside joke: save it once and it's yours to keep.
          </EmptyState>
        </div>
      )}

      {/* made for two */}
      <div className="mt-8 mb-4">
        <h2 className="section-title">Made for the two of you</h2>
        <p className="mt-1 text-[12px] text-muted">A little something for every kind of day.</p>
      </div>
      <div className="grid grid-cols-2 gap-3.5">
        <BucketPreview />
        {[
          { icon: <Mail size={18} />, title: "Words worth keeping", text: "Write a love letter now, or seal a time capsule for a special day.", to: "letters" as const, link: "Our letters", tone: "bg-lilac" },
        ].map((c) => (
          <div key={c.title} className="card relative min-h-[148px] overflow-hidden p-5">
            <div className={`grid h-9 w-9 place-items-center rounded-[11px] text-accent ${c.tone}`}>{c.icon}</div>
            <h3 className="mt-3 mb-1 font-serif text-[18px]">{c.title}</h3>
            <p className="mb-3 max-w-[260px] text-[11.5px] leading-relaxed text-muted">{c.text}</p>
            <LinkButton to={c.to}>{c.link}</LinkButton>
            <span className="pointer-events-none absolute right-3 -bottom-6 font-serif text-[74px] text-line" aria-hidden="true">
              ♡
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
