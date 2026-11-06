import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { Feather, Loader, Send, Trash2, X } from "lucide-react";
import { cupidApi, cupidKeys, useCupidStatus, type ChatMessage } from "./cupidApi";
import { queryClient } from "../lib/queries";
import { quotaError } from "../features/plan/planApi";
import { BuyButton, useUpgrade } from "../features/plan/Purchase";
import { ReportAnswer } from "./ReportAnswer";
import { toastError } from "../components/ui/Toast";
import { CupidSprite } from "./CupidSprite";

/** A small chat with Cupid. History stays on this PC only. */
export function CupidChat({ onClose }: { onClose: () => void }) {
  const { data: history } = useQuery({ queryKey: cupidKeys.history, queryFn: cupidApi.history });
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [outOfArrows, setOutOfArrows] = useState(false);
  const { data: st } = useCupidStatus();
  const showUpgrade = useUpgrade((s) => s.show);
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
      setOutOfArrows(false);
    } catch (e) {
      const q = quotaError(e);
      if (q.code === "cupid_quota") setOutOfArrows(true);
      else toastError(q.message);
    } finally {
      await queryClient.invalidateQueries({ queryKey: cupidKeys.history });
      queryClient.invalidateQueries({ queryKey: cupidKeys.status });
      setBusy(false);
    }
  };

  return createPortal(
    <div role="dialog" aria-label="Chat with Cupid" className="animate-fade-up fixed right-6 bottom-6 z-[95] flex h-[min(560px,80vh)] w-[min(380px,92vw)] flex-col overflow-hidden rounded-[22px] border border-line bg-paper shadow-[var(--shadow-lg)]">
      <div className="flex items-center gap-3 border-b border-line-soft bg-hero px-4 py-3">
        <div className="h-10 w-14"><CupidSprite mood={busy ? "thinking" : "happy"} /></div>
        <div className="flex-1">
          <strong className="font-serif text-[17px]">Cupid</strong>
          <div className="text-[10.5px] text-muted">Answers are written by AI and can be wrong. He can't see your journal or letters.</div>
          {st?.cloud && st.arrows && (
            <div className="mt-0.5 flex items-center gap-1 text-[10.5px] font-semibold text-accent-deep" title="One arrow per message. They refill on the 1st of each month.">
              <Feather size={11} /> {st.arrows.remaining} arrow{st.arrows.remaining === 1 ? "" : "s"} left
            </div>
          )}
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
            <div className="flex max-w-[85%] flex-col items-start">
              <div className={`rounded-2xl px-3.5 py-2 text-[12.5px] leading-relaxed whitespace-pre-wrap ${m.role === "user" ? "bg-accent text-on-accent" : "bg-paper-2 text-ink-soft"}`}>
                {m.content}
              </div>
              {m.role === "model" && st?.cloud && m.id !== "pending" && <ReportAnswer messageId={m.id} />}
            </div>
          </div>
        ))}
        {outOfArrows && (
          <div className="rounded-2xl bg-gold p-3.5 text-[12px] leading-relaxed text-ink-soft">
            <strong className="block">Cupid's quiver is empty for this month</strong>
            His arrows refill on the 1st. Or top up now: extra arrows never expire.
            {st?.arrows?.canBuy && (
              <div className="mt-2.5 flex flex-wrap gap-2">
                <BuyButton product="arrows_100" onDone={() => setOutOfArrows(false)} />
                {st.arrows.tier !== "premium" && (
                  <button onClick={showUpgrade} className="text-[11.5px] font-semibold text-accent hover:underline">
                    Premium: 100 a month →
                  </button>
                )}
              </div>
            )}
          </div>
        )}
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
