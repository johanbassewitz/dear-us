import type { InputHTMLAttributes, ReactNode } from "react";

interface Props extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  hint?: ReactNode;
  error?: string | null;
}

export function Field({ label, hint, error, id, ...rest }: Props) {
  const fid = id ?? `f-${label.replace(/\W+/g, "-").toLowerCase()}`;
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={fid} className="text-[11px] font-bold text-ink-soft">
        {label}
      </label>
      <input id={fid} className="field-input" aria-invalid={!!error} {...rest} />
      {error ? (
        <span className="text-[11px] text-[#b0413e]">{error}</span>
      ) : (
        hint && <span className="text-[11px] text-muted">{hint}</span>
      )}
    </div>
  );
}
