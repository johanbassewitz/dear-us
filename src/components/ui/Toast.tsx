import { create } from "zustand";
import { Heart, TriangleAlert } from "lucide-react";

interface ToastAction {
  label: string;
  run: () => void;
}
interface ToastItem {
  id: number;
  text: string;
  tone: "info" | "error";
  action?: ToastAction;
}

interface ToastState {
  items: ToastItem[];
  push: (text: string, tone?: ToastItem["tone"], action?: ToastAction) => void;
  dismiss: (id: number) => void;
}

let next = 1;
export const useToasts = create<ToastState>((set, get) => ({
  items: [],
  push: (text, tone = "info", action) => {
    const id = next++;
    set((s) => ({ items: [...s.items, { id, text, tone, action }] }));
    setTimeout(() => get().dismiss(id), action ? 7000 : tone === "error" ? 6000 : 3200);
  },
  dismiss: (id) => set((s) => ({ items: s.items.filter((t) => t.id !== id) })),
}));

export const toast = (text: string, action?: ToastAction) => useToasts.getState().push(text, "info", action);
export const toastError = (text: string) => useToasts.getState().push(text, "error");

export function Toasts() {
  const items = useToasts((s) => s.items);
  const dismiss = useToasts((s) => s.dismiss);
  return (
    <div className="pointer-events-none fixed bottom-6 left-1/2 z-[120] flex -translate-x-1/2 flex-col items-center gap-2" aria-live="polite">
      {items.map((t) => (
        <div
          key={t.id}
          className="animate-fade-up pointer-events-auto flex items-center gap-2.5 rounded-xl bg-[#3d3435] px-4 py-3 text-[12.5px] text-white shadow-lg ring-1 ring-white/20"
        >
          {t.tone === "error" ? <TriangleAlert size={15} className="text-[#f2b8a8]" /> : <Heart size={15} className="text-[#dfb4c0]" />}
          {t.text}
          {t.action && (
            <button
              onClick={() => {
                t.action!.run();
                dismiss(t.id);
              }}
              className="ml-1 rounded-md px-2 py-0.5 font-bold text-[#f3c3cf] hover:bg-white/10"
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
