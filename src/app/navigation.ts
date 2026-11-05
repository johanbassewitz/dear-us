import { create } from "zustand";
import { isPageId, type PageId } from "./nav";
import { usePrefs } from "../lib/settings";

/**
 * Opens the editor pre-filled. `linkTo` asks the editor to link the NEW memory back to the thing
 * it was written for once it is saved (see features/journal/entryLinks.ts).
 */
export interface EditPrefill {
  entryDate?: string;
  title?: string;
  linkTo?: { kind: "bucket" | "date"; id: string };
}

export type JournalView =
  | { mode: "list" }
  | { mode: "view"; id: string }
  | { mode: "edit"; id: string | null; prefill?: EditPrefill }
  | { mode: "trash" };

interface Pending {
  page: PageId;
  journal?: JournalView;
}

interface NavState {
  page: PageId;
  journal: JournalView;
  /** Set while there are unsaved changes; navigation then asks first. */
  blocker: string | null;
  pending: Pending | null;
  go: (page: PageId) => void;
  openJournal: (view: JournalView) => void;
  setBlocker: (msg: string | null) => void;
  confirmLeave: () => void;
  cancelLeave: () => void;
}

function apply(set: (s: Partial<NavState>) => void, p: Pending) {
  set({ page: p.page, journal: p.journal ?? { mode: "list" }, blocker: null, pending: null });
  usePrefs.getState().set("ui.lastPage", p.page);
}

export const useNav = create<NavState>((set, get) => ({
  page: "home",
  journal: { mode: "list" },
  blocker: null,
  pending: null,
  go: (page) => {
    const p: Pending = { page, journal: page === "journal" && get().page === "journal" ? { mode: "list" } : undefined };
    if (get().blocker) set({ pending: p });
    else apply(set, p);
  },
  openJournal: (view) => {
    const p: Pending = { page: "journal", journal: view };
    if (get().blocker) set({ pending: p });
    else apply(set, p);
  },
  setBlocker: (msg) => set({ blocker: msg }),
  confirmLeave: () => {
    const p = get().pending;
    if (p) apply(set, p);
  },
  cancelLeave: () => set({ pending: null }),
}));

/** Restore the last visited page once preferences have loaded. */
export function restoreLastPage() {
  const last = usePrefs.getState().prefs["ui.lastPage"];
  if (isPageId(last)) useNav.setState({ page: last });
}
