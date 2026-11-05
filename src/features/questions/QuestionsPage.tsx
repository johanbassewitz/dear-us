import { useMemo, useState } from "react";
import { MessageCircleHeart, Plus, Trash2 } from "lucide-react";
import { api, type Answer, type CustomQuestion } from "../../lib/api";
import { invalidateQuestions, useAnswers, useCustomQuestions, useDailyQuestion, useProfile } from "../../lib/queries";
import { localIsoDate } from "../../lib/dates";
import { fmtDate, fmtShort } from "../../lib/format";
import { CATEGORIES, QUESTIONS, categoryLabel, isoWeek, questionById, questionOfTheDay, weeklyCheckIn } from "../../lib/questions";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { EmptyState } from "../../components/ui/EmptyState";
import { Modal } from "../../components/ui/Modal";
import { Pill } from "../../components/ui/Pill";
import { Field } from "../../components/ui/Field";
import { toast, toastError } from "../../components/ui/Toast";
import { AnswerBox, PartnerReveal } from "./AnswerBox";
import { MoodCheckIn } from "./MoodCheckIn";

interface Q {
  id: string;
  text: string;
  category: string;
  custom?: boolean;
}

const PAGE_SIZE = 12;

function AddQuestionDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [text, setText] = useState("");
  const [category, setCategory] = useState("reflection");
  const [error, setError] = useState<string | null>(null);
  const submit = async () => {
    try {
      await api.questionCustomCreate(text, category);
      invalidateQuestions();
      toast("Added to your questions");
      setText("");
      setError(null);
      onClose();
    } catch (e) {
      setError(String(e).replace(/^Error:\s*/, ""));
    }
  };
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Add a question of your own"
      description="Something only the two of you would ask."
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit}>Add question</Button>
        </>
      }
    >
      <div className="flex flex-col gap-3.5">
        <Field label="Your question" value={text} onChange={(e) => setText(e.target.value)} error={error} maxLength={300} />
        <div className="flex flex-col gap-1.5">
          <label htmlFor="q-category" className="text-[11px] font-bold text-ink-soft">
            Category
          </label>
          <select id="q-category" className="field-input" value={category} onChange={(e) => setCategory(e.target.value)}>
            {CATEGORIES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      </div>
    </Modal>
  );
}

/** Answer any question (today's date). Earlier answers stay below so you can revisit over time. */
function AnswerDialog({ q, today, answers, partnerName, onClose }: { q: Q; today: string; answers: Answer[]; partnerName: string; onClose: () => void }) {
  const mine = answers.filter((a) => a.mine && a.questionId === q.id);
  const todays = mine.find((a) => a.answeredForDate === today);
  const earlier = mine.filter((a) => a.answeredForDate !== today);
  return (
    <Modal open onClose={onClose} title={q.text} description={categoryLabel(q.category)} width={600}>
      <div className="flex flex-col gap-4">
        <AnswerBox key={todays?.id ?? "new"} questionId={q.id} questionText={q.text} date={today} existing={todays} onSaved={onClose} />
        {earlier.length > 0 && (
          <div>
            <div className="tiny-label mb-2">You answered this before</div>
            <ul className="flex flex-col gap-2">
              {earlier.map((a) => {
                const theirs = answers.find((x) => !x.mine && x.questionId === q.id && x.answeredForDate === a.answeredForDate);
                return (
                  <li key={a.id} className="rounded-xl bg-paper-2 p-3 text-[12.5px] leading-relaxed">
                    <span className="text-[11px] font-bold text-faint">{fmtDate(a.answeredForDate)}</span>
                    <p className="mt-0.5 whitespace-pre-wrap text-ink-soft">{a.body}</p>
                    {theirs && (
                      <p className="mt-1.5 whitespace-pre-wrap border-t border-line pt-1.5 text-muted">
                        <b>{partnerName}:</b> {theirs.body}
                      </p>
                    )}
                  </li>
                );
              })}
            </ul>
          </div>
        )}
      </div>
    </Modal>
  );
}

export function QuestionsPage() {
  const { data: profile } = useProfile();
  const { data: answersData } = useAnswers();
  const { data: customs } = useCustomQuestions();
  const answers = useMemo(() => answersData ?? [], [answersData]);
  const today = localIsoDate();
  const spaceId = profile?.space?.id;
  const partnerName = profile?.partner?.displayName ?? "Your partner";

  const picked = questionOfTheDay(today, spaceId);
  const { data: daily } = useDailyQuestion(today, picked.id);

  const lookup = useMemo(() => {
    const m = new Map<string, Q>();
    for (const q of QUESTIONS) m.set(q.id, q);
    for (const c of customs ?? []) m.set(c.id, { ...c, custom: true });
    return m;
  }, [customs]);
  const resolve = (id: string): Q => lookup.get(id) ?? questionById(id) ?? { id, text: "A question we no longer have", category: "reflection" };

  const todayQ = resolve(daily?.questionId ?? picked.id);
  const myToday = answers.find((a) => a.mine && a.questionId === todayQ.id && a.answeredForDate === today);
  const partnerToday = answers.find((a) => !a.mine && a.questionId === todayQ.id && a.answeredForDate === today);

  const week = isoWeek(today);
  const checkIn = useMemo(() => weeklyCheckIn(today, spaceId), [today, spaceId]);

  const [adding, setAdding] = useState(false);
  const [answering, setAnswering] = useState<Q | null>(null);
  const [category, setCategory] = useState<string>("all");
  const [search, setSearch] = useState("");
  const [limit, setLimit] = useState(PAGE_SIZE);
  const [removingAnswer, setRemovingAnswer] = useState<Answer | null>(null);
  const [removingQuestion, setRemovingQuestion] = useState<CustomQuestion | null>(null);

  const library: Q[] = useMemo(() => {
    const all: Q[] = [...(customs ?? []).map((c) => ({ ...c, custom: true })), ...QUESTIONS];
    const s = search.trim().toLowerCase();
    return all.filter((q) => (category === "all" || (category === "own" ? q.custom : q.category === category)) && (!s || q.text.toLowerCase().includes(s)));
  }, [customs, category, search]);

  const history = useMemo(() => {
    const byDate = new Map<string, Map<string, Answer[]>>();
    for (const a of answers) {
      const d = byDate.get(a.answeredForDate) ?? new Map<string, Answer[]>();
      d.set(a.questionId, [...(d.get(a.questionId) ?? []), a]);
      byDate.set(a.answeredForDate, d);
    }
    return [...byDate.entries()].sort((a, b) => b[0].localeCompare(a[0]));
  }, [answers]);

  const removeAnswer = async (a: Answer) => {
    setRemovingAnswer(null);
    try {
      await api.questionAnswerDelete(a.id);
      invalidateQuestions();
      toast("Answer removed", {
        label: "Undo",
        run: () =>
          api
            .questionAnswerRestore(a.id)
            .then(invalidateQuestions)
            .catch((e) => toastError(String(e))),
      });
    } catch (e) {
      toastError(String(e));
    }
  };
  const removeQuestion = async (c: CustomQuestion) => {
    setRemovingQuestion(null);
    try {
      await api.questionCustomDelete(c.id);
      invalidateQuestions();
      toast("Question removed", {
        label: "Undo",
        run: () =>
          api
            .questionCustomRestore(c.id)
            .then(invalidateQuestions)
            .catch((e) => toastError(String(e))),
      });
    } catch (e) {
      toastError(String(e));
    }
  };

  const chip = (id: string, label: string) => (
    <button
      key={id}
      type="button"
      aria-pressed={category === id}
      onClick={() => {
        setCategory(id);
        setLimit(PAGE_SIZE);
      }}
      className={`rounded-full border px-3 py-1.5 text-[11.5px] font-semibold transition-colors ${
        category === id ? "border-accent bg-accent-soft text-accent-deep" : "border-line text-muted hover:border-accent"
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="animate-fade-up">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="eyebrow">Put a little love into words</div>
          <h1 className="page-title mt-2">Questions</h1>
          <p className="mt-2 text-[13px] text-muted">Gentle questions to answer at your own pace. No scores, no pressure.</p>
        </div>
        <Button icon={<Plus size={15} />} onClick={() => setAdding(true)}>
          Add a question
        </Button>
      </div>

      <div className="grid gap-5 lg:grid-cols-[1fr_340px]">
        <section className="card p-6" aria-labelledby="today-q">
          <span className="tiny-label">A little question for today · {fmtDate(today)}</span>
          <h2 id="today-q" className="my-3 font-serif text-[24px] leading-snug">
            {todayQ.text}
          </h2>
          <div className="flex flex-col gap-4">
            <AnswerBox key={myToday?.id ?? "new"} questionId={todayQ.id} questionText={todayQ.text} date={today} existing={myToday} />
            <PartnerReveal iAnswered={!!myToday} partnerAnswer={partnerToday} partnerName={partnerName} />
          </div>
        </section>
        <MoodCheckIn today={today} />
      </div>

      <section className="mt-8" aria-labelledby="week-title">
        <h2 id="week-title" className="section-title">This week's check-in</h2>
        <p className="mt-1 text-[12px] text-muted">
          Three gentle prompts for {fmtShort(week.start)} to {fmtShort(week.end)}. Answer one, all, or none.
        </p>
        <div className="mt-3.5 grid gap-4 md:grid-cols-3">
          {checkIn.map((q) => {
            const mine = answers.find((a) => a.mine && a.questionId === q.id && a.answeredForDate >= week.start && a.answeredForDate <= week.end);
            return (
              <article key={q.id} className="card flex flex-col gap-3 p-4" aria-label={q.text}>
                <Pill tone="gold">{categoryLabel(q.category)}</Pill>
                <h3 className="font-serif text-[16px] leading-snug">{q.text}</h3>
                {mine && <p className="line-clamp-4 whitespace-pre-wrap text-[12px] leading-relaxed text-muted">{mine.body}</p>}
                <Button variant="secondary" size="sm" className="mt-auto self-start" onClick={() => setAnswering(q)}>
                  {mine ? "Revisit" : "Answer this"}
                </Button>
              </article>
            );
          })}
        </div>
      </section>

      <section className="mt-9" aria-labelledby="library-title">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <h2 id="library-title" className="section-title">Question library</h2>
          <div className="w-full max-w-[260px]">
            <input
              className="field-input"
              type="search"
              aria-label="Search questions"
              placeholder="Search questions…"
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setLimit(PAGE_SIZE);
              }}
            />
          </div>
        </div>
        <div className="mt-3 flex flex-wrap gap-2" role="group" aria-label="Question categories">
          {chip("all", "All")}
          {(customs?.length ?? 0) > 0 && chip("own", "Our own")}
          {CATEGORIES.map((c) => chip(c.id, c.label))}
        </div>
        <ul className="mt-4 grid gap-3 md:grid-cols-2">
          {library.slice(0, limit).map((q) => {
            const before = answers.filter((a) => a.mine && a.questionId === q.id).length;
            return (
              <li key={q.id} className="card flex items-start justify-between gap-3 p-4">
                <div>
                  <div className="mb-1.5 flex flex-wrap gap-1.5">
                    <Pill tone={q.custom ? "green" : "gray"}>{q.custom ? "Our own" : categoryLabel(q.category)}</Pill>
                  </div>
                  <p className="font-serif text-[15px] leading-snug">{q.text}</p>
                </div>
                <div className="flex shrink-0 flex-col items-end gap-1.5">
                  <Button variant="secondary" size="sm" onClick={() => setAnswering(q)} aria-label={`Answer this: ${q.text}`}>
                    {before ? "Revisit" : "Answer this"}
                  </Button>
                  {q.custom && (
                    <button
                      type="button"
                      aria-label={`Remove question: ${q.text}`}
                      title="Remove"
                      onClick={() => setRemovingQuestion(customs?.find((c) => c.id === q.id) ?? null)}
                      className="rounded p-1 text-faint hover:text-ink"
                    >
                      <Trash2 size={13} />
                    </button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
        {library.length === 0 && <p className="mt-4 text-[12.5px] text-muted">Nothing matches that. Try another word, or add your own question.</p>}
        {library.length > limit && (
          <div className="mt-4 text-center">
            <Button variant="secondary" size="sm" onClick={() => setLimit(limit + PAGE_SIZE)}>
              Show more
            </Button>
          </div>
        )}
      </section>

      <section className="mt-9" aria-labelledby="kept-title">
        <h2 id="kept-title" className="section-title">Answers we've kept</h2>
        {history.length === 0 ? (
          <EmptyState icon={<MessageCircleHeart size={22} />} title="Nothing here yet" compact>
            Your answers will be kept here by date, so you can look back on them together.
          </EmptyState>
        ) : (
          <div className="mt-3 flex flex-col gap-5">
            {history.map(([date, byQuestion]) => (
              <section key={date} aria-label={fmtDate(date)}>
                <div className="tiny-label mb-2">{fmtDate(date)}</div>
                <div className="flex flex-col gap-3">
                  {[...byQuestion.entries()].map(([qid, list]) => {
                    const q = resolve(qid);
                    const mine = list.find((a) => a.mine);
                    const theirs = list.find((a) => !a.mine);
                    return (
                      <article key={qid} className="card p-4" aria-label={q.text}>
                        <h3 className="font-serif text-[15px] leading-snug">{q.text}</h3>
                        {mine && (
                          <div className="mt-2.5 flex items-start justify-between gap-3">
                            <p className="whitespace-pre-wrap text-[12.5px] leading-relaxed text-ink-soft">
                              <b className="text-[11px] text-faint">You </b>
                              {mine.body}
                            </p>
                            <button
                              type="button"
                              aria-label={`Remove your answer to: ${q.text}`}
                              title="Remove my answer"
                              onClick={() => setRemovingAnswer(mine)}
                              className="shrink-0 rounded p-1 text-faint hover:text-ink"
                            >
                              <Trash2 size={13} />
                            </button>
                          </div>
                        )}
                        {theirs && (
                          <p className="mt-2 whitespace-pre-wrap rounded-xl bg-accent-soft p-3 text-[12.5px] leading-relaxed">
                            <b className="text-[11px] text-accent-deep">{partnerName} </b>
                            {theirs.body}
                          </p>
                        )}
                      </article>
                    );
                  })}
                </div>
              </section>
            ))}
          </div>
        )}
      </section>

      <AddQuestionDialog open={adding} onClose={() => setAdding(false)} />
      {answering && <AnswerDialog q={answering} today={today} answers={answers} partnerName={partnerName} onClose={() => setAnswering(null)} />}
      <ConfirmDialog
        open={!!removingAnswer}
        title="Remove this answer?"
        message="It will leave your kept answers. You can undo right after."
        confirmLabel="Remove"
        danger
        onConfirm={() => removingAnswer && removeAnswer(removingAnswer)}
        onCancel={() => setRemovingAnswer(null)}
      />
      <ConfirmDialog
        open={!!removingQuestion}
        title="Remove this question?"
        message={`"${removingQuestion?.text ?? ""}" will leave your library. Answers you wrote stay. You can undo right after.`}
        confirmLabel="Remove"
        danger
        onConfirm={() => removingQuestion && removeQuestion(removingQuestion)}
        onCancel={() => setRemovingQuestion(null)}
      />
    </div>
  );
}
