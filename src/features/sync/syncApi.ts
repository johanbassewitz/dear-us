import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { call, isDesktop } from "../../lib/api";
import { queryClient } from "../../lib/queries";
import type { Plan } from "../plan/planApi";

export type SyncState =
  | "off"
  | "setup"
  | "waiting_for_partner"
  | "syncing"
  | "up_to_date"
  | "pending"
  | "offline"
  | "waiting_for_space"
  | "error"
  | "signed_out";

export interface SyncDevice {
  id: string;
  name: string;
  mine: boolean;
  thisPc: boolean;
  revoked: boolean;
}
export interface SyncStatus {
  state: SyncState;
  configured: boolean;
  paired: boolean;
  signedIn: boolean;
  email: string | null;
  projectUrl: string | null;
  lastSyncAt: string | null;
  lastError: string | null;
  counts: {
    pendingOps: number;
    awaitingDelivery: number;
    uploadsPending: number;
    downloadsPending: number;
    failedUpdates: number;
    openConflicts: number;
  };
  devices: SyncDevice[];
  thisDevice: string;
  relayBytes: number | null;
  directEnabled: boolean;
  directPeer: boolean;
  /** Store edition: the couple's plan, last known. */
  plan?: Plan | null;
}
export interface SyncConflict {
  id: string;
  entity: string;
  entityId: string;
  field: string;
  keptValue: string;
  otherValue: string;
  createdAt: string;
  title: string | null;
}

export const syncApi = {
  status: () => call<SyncStatus>("sync_status"),
  checkProject: (url: string, anonKey: string) => call<number>("sync_check_project", { url, anonKey }),
  connectAccount: (url: string, anonKey: string, email: string, password: string, create: boolean) =>
    call<{ needsEmailConfirmation: boolean }>("sync_connect_account", { url, anonKey, email, password, create }),
  createSpace: () => call<string>("sync_create_space"),
  createInvite: () => call<string>("sync_create_invite"),
  join: (input: { invite: string; email: string; password: string; createAccount: boolean; myName: string | null }) =>
    call<{ needsEmailConfirmation: boolean; joined: boolean }>("sync_join", { input }),
  signIn: (password: string) => call<void>("sync_sign_in", { password }),
  now: () => call<void>("sync_now"),
  recoveryPhrase: () => call<string>("sync_recovery_phrase"),
  revokeDevice: (deviceId: string) => call<void>("sync_revoke_device", { deviceId }),
  disconnect: () => call<void>("sync_disconnect"),
  conflicts: () => call<SyncConflict[]>("sync_conflicts"),
  resolve: (id: string, useOther: boolean) => call<void>("sync_resolve_conflict", { id, useOther }),
  setDirect: (on: boolean) => call<void>("sync_set_direct", { on }),
};

export const syncKeys = { status: ["sync", "status"] as const, conflicts: ["sync", "conflicts"] as const };

export const useSyncStatus = () => useQuery({ queryKey: syncKeys.status, queryFn: syncApi.status, refetchInterval: 15_000, staleTime: 5_000 });

/** Live updates from the background sync: status changes, and new data from the partner. */
export function useSyncEvents() {
  useEffect(() => {
    if (!isDesktop) return;
    const offs: (() => void)[] = [];
    let cancelled = false;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<SyncStatus>("sync-status", (e) => {
        queryClient.setQueryData(syncKeys.status, e.payload);
        if (e.payload.counts.openConflicts) queryClient.invalidateQueries({ queryKey: syncKeys.conflicts });
      }).then((u) => (cancelled ? u() : offs.push(u)));
      // The partner's changes arrived: refresh everything on screen.
      listen("data-changed", () => queryClient.invalidateQueries()).then((u) => (cancelled ? u() : offs.push(u)));
    });
    return () => {
      cancelled = true;
      offs.forEach((u) => u());
    };
  }, []);
}

/** Plain-language status, e.g. for the badge and dashboard. */
export function describe(s: SyncStatus | undefined, partner = "your partner"): { label: string; tone: "green" | "gold" | "accent" | "gray"; sentence: string } {
  if (!s || s.state === "off")
    return { label: "Saved on this PC", tone: "green", sentence: "Everything is saved on this computer. Sync with your partner isn't set up yet." };
  const waiting = s.counts.pendingOps + s.counts.awaitingDelivery;
  const files = s.counts.uploadsPending + s.counts.downloadsPending;
  switch (s.state) {
    case "setup":
      return { label: "Finish setup", tone: "gold", sentence: "Almost there: create your shared space or join your partner's." };
    case "waiting_for_partner":
      return { label: "Waiting for partner", tone: "gold", sentence: `Your memories are safe here. They'll start travelling as soon as ${partner} joins with your invite code.` };
    case "up_to_date":
      return { label: "Up to date", tone: "green", sentence: "Everything is saved on this computer and shared with your partner." };
    case "pending":
      return {
        label: files ? `${files} file${files > 1 ? "s" : ""} on the way` : `${waiting} waiting`,
        tone: "gold",
        sentence: `Your memories are saved safely on this computer.${waiting ? ` ${waiting} update${waiting > 1 ? "s are" : " is"} waiting to reach ${partner}.` : ""}${files ? ` ${files} photo${files > 1 ? "s/videos are" : "/video is"} still travelling.` : ""}`,
      };
    case "offline":
      return { label: "Offline", tone: "gray", sentence: "You're offline. Keep going: everything is saved here and will sync when you're back online." };
    case "waiting_for_space":
      if (s.plan)
        return {
          label: "Photos wait for Wi-Fi",
          tone: "gold",
          sentence: `Your memories keep syncing. This month's free cloud space for photos and videos is used up, so new ones travel over your home Wi-Fi when you're both home, or through the cloud next month.`,
        };
      return { label: "Cloud space full", tone: "gold", sentence: `The temporary cloud space is full for now. Big files wait safely here until ${partner}'s PC picks up the earlier ones.` };
    case "signed_out":
      return { label: "Sign in to sync", tone: "accent", sentence: "Your sign-in expired. Your memories are safe here; sign in again to keep syncing." };
    case "error":
      return { label: "Sync problem", tone: "accent", sentence: s.lastError ?? "Something went wrong while syncing. Your memories are safe on this PC." };
    default:
      return { label: "Syncing…", tone: "green", sentence: "Syncing with your partner…" };
  }
}
