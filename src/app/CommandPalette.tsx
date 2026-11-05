import { useEffect, useMemo, useState } from "react";
import { BookHeart, Search } from "lucide-react";
import { createPortal } from "react-dom";
import { ALL_PAGES } from "./nav";
import { useNav } from "./navigation";
import { api, type EntrySummary } from "../lib/api";
import { fmtDate } from "../lib/format";

type Result = { kind: "page"; id: (typeof ALL_PAGES)[number]["id"]; label: string; icon: (typeof ALL_PAGES)[number]["icon"] } | { kind: "entry"; entry: EntrySummary };

/** Ctrl+K: jump to a page or find a memory. */
export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const go = useNav((s) => s.go);
  const openJournal = useNav((s) => s.openJournal);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [entries, setEntries] = useState<EntrySummary[]>([]);

  useEffect(() => {
    if (open) {
      setQ("");
      setSel(0);
      setEntries([]);
    }
  }, [open]);

  useEffect(() => {
    setSel(0);
    const t = q.trim();
    if (t.length < 2) return setEntries([]);
    let alive = true;
    const timer = setTimeout(() => {
      api
        .journalList({ text: t, limit: 8 })
        .then((r) => alive && setEntries(r))
        .catch(() => alive && setEntries([]));
    }, 150);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [q]);

  const results: Result[] = useMemo(() => {
    const t = q.trim().toLowerCase();
    const pages = (t ? ALL_PAGES.filter((p) => p.label.toLowerCase().includes(t)) : ALL_PAGES).map(
      (p) => ({ kind: "page", id: p.id, label: p.label, icon: p.icon }) as Result,
    );
    return [...entries.map((entry) => ({ kind: "entry", entry }) as Result), ...pages];
  }, [q, entries]);

  if (!open) return null;

  const pick = (i: number) => {
    const r = results[i];
    if (!r) return;
    if (r.kind === "page") go(r.id);
    else openJournal({ mode: "view", id: r.entry.id });
    onClose();
  };

  return createPortal(
    <div
      className="fixed inset-0 z-[90] flex items-start justify-center bg-[#31262c]/40 p-5 pt-[14vh] backdrop-blur-[2px]"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div role="dialog" aria-label="Search" className="animate-fade-up w-full max-w-[580px] overflow-hidden rounded-[20px] border border-line bg-paper shadow-[var(--shadow-lg)]">
        <div className="flex items-center gap-3 border-b border-line-soft px-5 py-4">
          <Search size={17} className="text-muted" />
          <input
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") onClose();
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setSel((s) => Math.min(s + 1, results.length - 1));
              }
              if (e.key === "ArrowUp") {
                e.preventDefault();
                setSel((s) => Math.max(s - 1, 0));
              }
              if (e.key === "Enter") pick(sel);
            }}
            placeholder="Search memories, or jump to a page…"
            aria-label="Search"
            className="flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-faint"
          />
        </div>
        <ul className="max-h-[380px] overflow-auto p-2" role="listbox">
          {results.length === 0 && <li className="px-4 py-6 text-center text-[12.5px] text-muted">Nothing found.</li>}
          {results.map((r, i) => {
            const active = i === sel;
            const cls = `flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-[13px] ${active ? "bg-accent-soft text-accent-deep" : "text-ink-soft"}`;
            if (r.kind === "page") {
              const Icon = r.icon;
              return (
                <li key={`p-${r.id}`} role="option" aria-selected={active}>
                  <button onMouseEnter={() => setSel(i)} onClick={() => pick(i)} className={cls}>
                    <Icon size={16} />
                    {r.label}
                  </button>
                </li>
              );
            }
            return (
              <li key={`e-${r.entry.id}`} role="option" aria-selected={active}>
                <button onMouseEnter={() => setSel(i)} onClick={() => pick(i)} className={cls}>
                  <BookHeart size={16} className="shrink-0" />
                  <span className="min-w-0 flex-1 truncate">{r.entry.title}</span>
                  <span className="shrink-0 text-[11px] text-faint">{fmtDate(r.entry.entryDate)}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </div>,
    document.body,
  );
}
