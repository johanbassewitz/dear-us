import { useState } from "react";
import { Pencil, Trash2 } from "lucide-react";
import { api, type EntryComment, type ReactionType } from "../../lib/api";
import { invalidateJournal, useComments, useProfile, useReactions } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import { toast, toastError } from "../../components/ui/Toast";
import { relativeTime } from "./relativeTime";

export const REACTIONS: { type: ReactionType; emoji: string; label: string }[] = [
  { type: "love", emoji: "❤️", label: "Love" },
  { type: "emotional", emoji: "🥹", label: "Emotional" },
  { type: "funny", emoji: "😂", label: "Funny" },
  { type: "adorable", emoji: "🥰", label: "Adorable" },
  { type: "special", emoji: "✨", label: "Special" },
];

export function ReactionBar({ entryId }: { entryId: string }) {
  const { data } = useReactions(entryId);
  const toggle = async (type: ReactionType) => {
    try {
      await api.reactionToggle(entryId, type);
      invalidateJournal();
    } catch (err) {
      toastError(String(err));
    }
  };
  return (
    <div role="group" aria-label="Reactions" className="flex flex-wrap items-center gap-2">
      {REACTIONS.map((r) => {
        const c = data?.find((x) => x.type === r.type);
        const mine = !!c?.mine;
        const count = c?.count ?? 0;
        return (
          <button
            key={r.type}
            type="button"
            aria-pressed={mine}
            aria-label={`${r.label}${count ? `, ${count}` : ""}${mine ? ", you reacted" : ""}`}
            onClick={() => toggle(r.type)}
            className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-[13px] transition-all hover:-translate-y-px ${mine ? "border-accent bg-accent-soft text-accent-deep" : "border-line bg-paper text-ink-soft hover:border-accent"}`}
          >
            <span aria-hidden>{r.emoji}</span>
            {count > 0 && <span className="text-[11.5px] font-bold">{count}</span>}
          </button>
        );
      })}
    </div>
  );
}

function CommentItem({ c, mine }: { c: EntryComment; mine: boolean }) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(c.body);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      await api.commentUpdate(c.id, text);
      invalidateJournal();
      setEditing(false);
    } catch (err) {
      toastError(String(err));
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    setConfirm(false);
    try {
      await api.commentDelete(c.id);
      invalidateJournal();
      toast("Comment deleted", {
        label: "Undo",
        run: () =>
          api
            .commentRestore(c.id)
            .then(() => invalidateJournal())
            .catch((err) => toastError(String(err))),
      });
    } catch (err) {
      toastError(String(err));
    }
  };

  return (
    <li className="rounded-2xl bg-paper-2 px-4 py-3">
      <div className="flex flex-wrap items-baseline gap-x-2 text-[12px]">
        <strong className="font-bold text-ink">{c.authorName ?? "Someone"}</strong>
        <span className="text-faint">{relativeTime(c.createdAt)}</span>
        {c.edited && <span className="text-faint italic">edited</span>}
        {mine && !editing && (
          <span className="ml-auto flex gap-1">
            <button type="button" onClick={() => { setText(c.body); setEditing(true); }} aria-label="Edit comment" className="grid h-6 w-6 place-items-center rounded-full text-muted hover:text-accent">
              <Pencil size={13} />
            </button>
            <button type="button" onClick={() => setConfirm(true)} aria-label="Delete comment" className="grid h-6 w-6 place-items-center rounded-full text-muted hover:text-accent">
              <Trash2 size={13} />
            </button>
          </span>
        )}
      </div>
      {editing ? (
        <div className="mt-2 flex flex-col gap-2">
          <textarea value={text} onChange={(e) => setText(e.target.value)} aria-label="Edit your comment" maxLength={2000} rows={3} className="field-input resize-y" />
          <div className="flex gap-2">
            <Button size="sm" onClick={save} disabled={busy || !text.trim()}>
              Save
            </Button>
            <Button size="sm" variant="secondary" onClick={() => setEditing(false)}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <p className="mt-1 text-[13.5px] leading-relaxed whitespace-pre-wrap text-ink-soft">{c.body}</p>
      )}
      <ConfirmDialog
        open={confirm}
        title="Delete this comment?"
        message="You can undo it right after."
        confirmLabel="Delete comment"
        danger
        onConfirm={remove}
        onCancel={() => setConfirm(false)}
      />
    </li>
  );
}

export function Comments({ entryId }: { entryId: string }) {
  const { data: list } = useComments(entryId);
  const { data: profile } = useProfile();
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const meId = profile?.me?.id;

  const add = async () => {
    if (!text.trim()) return;
    setBusy(true);
    try {
      await api.commentAdd(entryId, text);
      invalidateJournal();
      setText("");
    } catch (err) {
      toastError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section aria-label="Comments" className="mt-8 border-t border-line pt-6">
      <h2 className="section-title">Little notes{list?.length ? ` · ${list.length}` : ""}</h2>
      {list && list.length === 0 && <p className="mt-2 text-[12.5px] text-muted">No notes yet. Say something sweet about this memory.</p>}
      {!!list?.length && (
        <ul className="mt-4 flex flex-col gap-2.5">
          {list.map((c) => (
            <CommentItem key={c.id} c={c} mine={c.authorId === meId} />
          ))}
        </ul>
      )}
      <div className="mt-4 flex flex-col gap-2">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          aria-label="Add a comment"
          placeholder="Add a note…"
          maxLength={2000}
          rows={2}
          className="field-input resize-y"
        />
        <div>
          <Button size="sm" onClick={add} disabled={busy || !text.trim()}>
            Add note
          </Button>
        </div>
      </div>
    </section>
  );
}
