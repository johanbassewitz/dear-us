// Local preferences, cached in memory and persisted through the Rust core.
import { create } from "zustand";
import { api } from "./api";

export const THEMES = [
  { id: "paper", name: "Paper", swatch: "#dfc8a7" },
  { id: "rose", name: "Rose", swatch: "#eecfd7" },
  { id: "lavender", name: "Lavender", swatch: "#d9cae9" },
  { id: "sage", name: "Sage", swatch: "#c9d9c5" },
  { id: "cozy-dark", name: "Cozy Dark", swatch: "#3a2c31" },
] as const;
export type ThemeId = (typeof THEMES)[number]["id"];

export interface Prefs {
  "ui.theme": ThemeId;
  "ui.lastPage": string;
  "cupid.visible": boolean;
  "cupid.pos": { right: number; bottom: number };
  "journal.view": "feed" | "gallery";
  /** On This Day memories dismissed, as "entryId:year" */
  "home.dismissed": string[];
  /** Desktop reminders for special dates (local-only, off by default) */
  "reminders.enabled": boolean;
  /** Already-notified occasions, as "id:occurrenceDate" */
  "reminders.notified": string[];
}

export const DEFAULT_PREFS: Prefs = {
  "ui.theme": "paper",
  "ui.lastPage": "home",
  "cupid.visible": true,
  "cupid.pos": { right: 28, bottom: 24 },
  "journal.view": "feed",
  "home.dismissed": [],
  "reminders.enabled": false,
  "reminders.notified": [],
};

interface PrefsState {
  loaded: boolean;
  prefs: Prefs;
  load: () => Promise<void>;
  set: <K extends keyof Prefs>(key: K, value: Prefs[K]) => void;
}

/** Parse stored JSON, falling back to the default for missing or corrupt values. */
export function parsePrefs(raw: Record<string, string>): Prefs {
  const out = { ...DEFAULT_PREFS } as Record<string, unknown>;
  for (const key of Object.keys(DEFAULT_PREFS)) {
    if (raw[key] === undefined) continue;
    try {
      const v = JSON.parse(raw[key]);
      if (typeof v === typeof (DEFAULT_PREFS as unknown as Record<string, unknown>)[key]) out[key] = v;
    } catch {
      /* keep default */
    }
  }
  if (out["journal.view"] !== "feed" && out["journal.view"] !== "gallery") out["journal.view"] = "feed";
  if (!Array.isArray(out["home.dismissed"])) out["home.dismissed"] = [];
  if (!Array.isArray(out["reminders.notified"])) out["reminders.notified"] = [];
  if (!THEMES.some((t) => t.id === out["ui.theme"])) out["ui.theme"] = DEFAULT_PREFS["ui.theme"];
  return out as unknown as Prefs;
}

export const usePrefs = create<PrefsState>((set, get) => ({
  loaded: false,
  prefs: DEFAULT_PREFS,
  load: async () => {
    const raw = await api.settingsGetAll();
    set({ prefs: parsePrefs(raw), loaded: true });
  },
  set: (key, value) => {
    set({ prefs: { ...get().prefs, [key]: value } });
    api.settingsSet(key, JSON.stringify(value)).catch((e) => console.error("could not save setting", key, e));
  },
}));

export function applyTheme(theme: ThemeId) {
  document.documentElement.dataset.theme = theme;
}
