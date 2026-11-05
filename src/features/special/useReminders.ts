import { useEffect } from "react";
import { isDesktop } from "../../lib/api";
import { useProfile, useSpecialDates } from "../../lib/queries";
import { usePrefs } from "../../lib/settings";
import { localIsoDate } from "../../lib/dates";
import { dueReminders, upcomingOccasions, type Occasion } from "./occurrence";

const HOUR = 60 * 60 * 1000;
const KEEP = 200;

export function reminderBody(o: Occasion): string {
  if (o.days === 0) return "It's today. Have a lovely time together.";
  return `Coming up in ${o.days} day${o.days === 1 ? "" : "s"}.`;
}

/** Ask the OS for notification permission (desktop only). Returns true when notifications may be shown. */
export async function ensureNotificationPermission(): Promise<boolean> {
  if (!isDesktop) return false;
  const { isPermissionGranted, requestPermission } = await import("@tauri-apps/plugin-notification");
  if (await isPermissionGranted()) return true;
  return (await requestPermission()) === "granted";
}

/**
 * While the app is open and "Desktop reminders" is on: check at startup and every hour,
 * and notify once per occurrence. Reminders never appear while Dear Us is closed.
 */
export function useReminders() {
  const enabled = usePrefs((s) => s.prefs["reminders.enabled"]);
  const { data: specials } = useSpecialDates();
  const { data: profile } = useProfile();
  const start = profile?.space?.startDate ?? null;

  useEffect(() => {
    if (!isDesktop || !enabled || !specials) return;
    let cancelled = false;
    const check = async () => {
      try {
        const notified = usePrefs.getState().prefs["reminders.notified"];
        const due = dueReminders(upcomingOccasions(specials, start, localIsoDate())).filter((o) => !notified.includes(o.key));
        if (!due.length || cancelled) return;
        const { isPermissionGranted, sendNotification } = await import("@tauri-apps/plugin-notification");
        if (!(await isPermissionGranted())) return;
        for (const o of due) sendNotification({ title: o.title, body: reminderBody(o) });
        usePrefs.getState().set("reminders.notified", [...notified, ...due.map((o) => o.key)].slice(-KEEP));
      } catch (e) {
        console.error("reminder check failed", e);
      }
    };
    void check();
    const t = setInterval(check, HOUR);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [enabled, specials, start]);
}
