import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}

/** Accessible modal: Escape closes, focus moves in and is restored on close. */
export function Modal({ open, onClose, title, description, children, footer, width = 540 }: Props) {
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement as HTMLElement | null;
    const t = setTimeout(() => {
      const first = panel.current?.querySelector<HTMLElement>("[autofocus],input,textarea,select,button");
      (first ?? panel.current)?.focus();
    }, 0);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "Tab" && panel.current) {
        const items = panel.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]),input,textarea,select,[tabindex]:not([tabindex="-1"])',
        );
        if (!items.length) return;
        const first = items[0];
        const last = items[items.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      clearTimeout(t);
      window.removeEventListener("keydown", onKey);
      previous?.focus();
    };
  }, [open, onClose]);

  if (!open) return null;
  return createPortal(
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center bg-[#31262c]/50 p-5 backdrop-blur-[3px]"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
        tabIndex={-1}
        className="animate-fade-up max-h-[90vh] w-full overflow-auto rounded-[22px] border border-line bg-paper shadow-[var(--shadow-lg)]"
        style={{ maxWidth: width }}
      >
        <div className="flex items-start justify-between gap-4 px-6 pt-6 pb-3">
          <div>
            <h2 id="modal-title" className="font-serif text-[25px] leading-tight text-ink">
              {title}
            </h2>
            {description && <p className="mt-1.5 text-[12.5px] text-muted">{description}</p>}
          </div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="grid h-9 w-9 shrink-0 place-items-center rounded-xl border border-line text-muted hover:text-ink"
          >
            <X size={16} />
          </button>
        </div>
        <div className="px-6 pb-6">{children}</div>
        {footer && <div className="flex justify-end gap-2 border-t border-line-soft px-6 py-4">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}
