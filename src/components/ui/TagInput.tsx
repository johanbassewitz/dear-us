import { useState } from "react";
import { X } from "lucide-react";

interface Props {
  tags: string[];
  onChange: (tags: string[]) => void;
  suggestions?: string[];
  id?: string;
}

/** Type a tag and press Enter or comma. Backspace removes the last one. */
export function TagInput({ tags, onChange, suggestions = [], id }: Props) {
  const [text, setText] = useState("");
  const add = (raw: string) => {
    const t = raw.trim().replace(/^#/, "").toLowerCase();
    if (t && !tags.includes(t) && t.length <= 40 && tags.length < 20) onChange([...tags, t]);
    setText("");
  };
  const open = suggestions.filter((s) => !tags.includes(s) && (!text || s.startsWith(text.toLowerCase()))).slice(0, 6);

  return (
    <div>
      <div className="field-input flex min-h-[42px] flex-wrap items-center gap-1.5 py-1.5!">
        {tags.map((t) => (
          <span key={t} className="inline-flex items-center gap-1 rounded-full bg-accent-soft px-2.5 py-1 text-[11.5px] font-semibold text-accent-deep">
            #{t}
            <button type="button" onClick={() => onChange(tags.filter((x) => x !== t))} aria-label={`Remove tag ${t}`} className="opacity-60 hover:opacity-100">
              <X size={11} />
            </button>
          </span>
        ))}
        <input
          id={id}
          value={text}
          onChange={(e) => {
            const v = e.target.value;
            if (v.endsWith(",")) add(v.slice(0, -1));
            else setText(v);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              add(text);
            } else if (e.key === "Backspace" && !text && tags.length) onChange(tags.slice(0, -1));
          }}
          onBlur={() => text && add(text)}
          placeholder={tags.length ? "" : "beach, anniversary, first…"}
          className="min-w-[120px] flex-1 bg-transparent text-[13px] outline-none placeholder:text-faint"
        />
      </div>
      {open.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          {open.map((s) => (
            <button key={s} type="button" onClick={() => add(s)} className="rounded-full border border-line px-2 py-0.5 text-[11px] text-muted hover:border-accent hover:text-accent">
              + {s}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
