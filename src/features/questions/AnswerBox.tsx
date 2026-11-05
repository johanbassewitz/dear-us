import { useState } from "react";
import { Lock, Heart } from "lucide-react";
import { api, type Answer } from "../../lib/api";
import { invalidateQuestions } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { toast, toastError } from "../../components/ui/Toast";

/** A gentle answer box. Saving again on the same day edits the answer; a later day keeps a new one. */
export function AnswerBox({
  questionId,
  questionText,
  date,
  existing,
  onSaved,
  rows = 4,
}: {
  questionId: string;
  questionText: string;
  date: string;
  existing?: Answer;
  onSaved?: (a: Answer) => void;
  rows?: number;
}) {
  const [body, setBody] = useState(existing?.body ?? "");
  const [busy, setBusy] = useState(false);
  const unchanged = body.trim() === (existing?.body ?? "");

  const save = async () => {
    setBusy(true);
    try {
      const saved = await api.questionAnswerSave(questionId, body, date);
      invalidateQuestions();
      toast(existing ? "Answer updated" : "Answer kept ♡");
      onSaved?.(saved);
    } catch (e) {
      toastError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-2.5">
      <textarea
        className="field-input resize-y leading-relaxed"
        rows={rows}
        value={body}
        onChange={(e) => setBody(e.target.value)}
        aria-label={`Your answer to: ${questionText}`}
        placeholder="Take your time, there's no wrong answer…"
      />
      <div className="flex items-center justify-between gap-3">
        <span className="text-[11px] text-muted">Kept on this PC, shared with your partner once you're connected.</span>
        <Button size="sm" icon={<Heart size={13} />} disabled={busy || !body.trim() || unchanged} onClick={save}>
          {existing ? "Update answer" : "Keep my answer"}
        </Button>
      </div>
    </div>
  );
}

/** The partner's answer: hidden until I've answered, and honest about sync. */
export function PartnerReveal({
  iAnswered,
  partnerAnswer,
  partnerName,
}: {
  iAnswered: boolean;
  partnerAnswer?: Answer;
  partnerName: string;
}) {
  if (iAnswered && partnerAnswer) {
    return (
      <div className="rounded-xl bg-accent-soft p-3.5">
        <div className="tiny-label">{partnerName}'s answer</div>
        <p className="mt-1.5 whitespace-pre-wrap text-[13px] leading-relaxed text-ink">{partnerAnswer.body}</p>
      </div>
    );
  }
  return (
    <div className="flex items-start gap-2.5 rounded-xl border border-dashed border-line p-3.5 text-[12px] text-muted">
      <Lock size={14} className="mt-0.5 shrink-0" aria-hidden="true" />
      <span>
        {iAnswered
          ? `${partnerName}'s answer will appear here once you're connected`
          : `Write yours first, then ${partnerName === "Your partner" ? "your partner" : partnerName}'s answer is revealed here ♡`}
      </span>
    </div>
  );
}
