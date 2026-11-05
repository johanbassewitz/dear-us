import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { Loader, Send, Trash2, X } from "lucide-react";
import { cupidApi, cupidKeys, type ChatMessage } from "./cupidApi";
import { queryClient } from "../lib/queries";
import { toastError } from "../components/ui/Toast";
import { CupidSprite } from "./CupidSprite";

/** A small chat with Cupid. History stays on this PC only. */
export function CupidChat({ onClose }: { onClose: () => void }) {
  const { data: history } = useQuery({ queryKey: cupidKeys.history, queryFn: cupidApi.history });
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Newer WebView2 engines return a Promise, which isn't a React cleanup function.
    end.current?.scrollIntoView({ block: "end" });
  }, [history, busy]);
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);

  const send = async () => {
    const m = text.trim();
    if (!m || busy) return;
    setText("");
    setBusy(true);
    const optimistic: ChatMessage = { id: "pending", role: "user", content: m, createdAt: "" };
    queryClient.setQueryData<ChatMessage[]>(cupidKeys.history, (h) => [...(h ?? []), optimistic]);
    try {
      await cupidApi.chat(m);
    } catch (e) {
      toastError(String(e));
    } finally {
      await queryClient.invalidateQueries({ queryKey: cupidKeys.history });
      setBusy(false);
    }
  };

  return createPortal(
    <div role="dialog" aria-label="Chat with Cupid" className="animate-fade-up fixed right-6 bottom-6 z-[95] flex h-[min(560px,80vh)] w-[min(380px,92vw)] flex-col overflow-hidden rounded-[22px] border border-line bg-paper shadow-[var(--shadow-lg)]">
      <div className="flex items-center gap-3 border-b border-line-soft bg-hero px-4 py-3">
        <div className="h-10 w-14"><CupidSprite mood={busy ? "thinking" : "happy"} /></div>
        <div className="flex-1">
          <strong className="font-serif text-[17px]">Cupid</strong>
          <div className="text-[10.5px] text-muted">Can't see your journal or letters, only this chat.</div>
        </div>
        <button
          onClick={async () => {
            await cupidApi.clear();
            queryClient.invalidateQueries({ queryKey: cupidKeys.history });
          }}
          aria-label="Clear chat"
          title="Clear chat"
          className="rounded p-1 text-faint hover:text-ink"
        >
          <Trash2 size={14} />
        </button>
        <button onClick={onClose} aria-label="Close" className="rounded p-1 text-faint hover:text-ink">
          <X size={16} />
        </button>
      </div>
      <div className="flex-1 space-y-2.5 overflow-y-auto p-4" aria-live="polite">
        {!history?.length && <p className="pt-6 text-center font-serif text-[15px] text-muted italic">Hi! Ask me for a date idea, help with a sweet message, or anything on your heart ♡</p>}
        {history?.map((m) => (
          <div key={m.id + m.createdAt} className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}>
            <div className={`max-w-[85%] rounded-2xl px-3.5 py-2 text-[12.5px] leading-relaxed whitespace-pre-wrap ${m.role === "user" ? "bg-accent text-on-accent" : "bg-paper-2 text-ink-soft"}`}>
              {m.content}
            </div>
          </div>
        ))}
        {busy && (
          <div className="flex items-center gap-2 text-[12px] text-muted">
            <Loader size={13} className="animate-spin" /> Cupid is thinking…
          </div>
        )}
        <div ref={end} />
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          send();
        }}
        className="flex gap-2 border-t border-line-soft p-3"
      >
        <input value={text} onChange={(e) => setText(e.target.value)} maxLength={2000} autoFocus placeholder="Write to Cupid…" aria-label="Message" className="field-input flex-1" />
        <button type="submit" disabled={busy || !text.trim()} aria-label="Send" className="grid h-10 w-10 place-items-center rounded-xl bg-accent text-on-accent disabled:opacity-40">
          <Send size={15} />
        </button>
      </form>
    </div>,
    document.body,
  );
}
