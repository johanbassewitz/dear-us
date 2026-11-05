import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { keys, queryClient } from "../../lib/queries";
import { cupidApi, cupidKeys, type CupidStatus } from "../../cupid/cupidApi";
import { CupidChat } from "../../cupid/CupidChat";
import { syncKeys, type SyncStatus } from "../sync/syncApi";
import { SyncSettings } from "../sync/SyncSettings";

vi.mock("../../lib/api", async (orig) => ({ ...(await orig<typeof import("../../lib/api")>()), isDesktop: true }));

const cupid: CupidStatus = {
  enabled: true,
  cloud: true,
  arrows: { remaining: 0, allowance: 10, used: 10, bonus: 0, tier: "free", canBuy: true },
  hasKey: true,
  privacyAck: true,
  model: "gemini-flash-latest",
  dailyQuestion: true,
  dateIdeas: true,
  shareBucketTitles: false,
};

beforeEach(() => {
  queryClient.clear();
  queryClient.setQueryData(keys.appInfo, { version: "1.1.0", dataDir: "x", schemaVersion: 2, edition: "store" });
  Object.defineProperty(Element.prototype, "scrollIntoView", { configurable: true, value: vi.fn() });
});
afterEach(() => {
  cleanup();
  queryClient.clear();
  vi.restoreAllMocks();
});

const wrap = (ui: React.ReactNode) => render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);

it("offers a top-up instead of an error when Cupid is out of arrows", async () => {
  queryClient.setQueryData(cupidKeys.status, cupid);
  queryClient.setQueryData(cupidKeys.history, []);
  vi.spyOn(cupidApi, "status").mockResolvedValue(cupid);
  vi.spyOn(cupidApi, "history").mockResolvedValue([]);
  vi.spyOn(cupidApi, "chat").mockRejectedValue("[cupid_quota] Cupid is out of arrows for this month.");
  wrap(<CupidChat onClose={() => {}} />);
  expect(screen.getByText("0 arrows left")).toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "A date idea?" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(await screen.findByText("Cupid's quiver is empty for this month")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /100 Cupid arrows · \$1\.99/ })).toBeInTheDocument();
});

it("asks for a Dear Us account, not a Supabase project, in the Store edition", () => {
  const off: Partial<SyncStatus> = {
    state: "off",
    configured: false,
    paired: false,
    signedIn: false,
    counts: { pendingOps: 0, awaitingDelivery: 0, uploadsPending: 0, downloadsPending: 0, failedUpdates: 0, openConflicts: 0 },
    devices: [],
  };
  queryClient.setQueryData(syncKeys.status, off);
  wrap(<SyncSettings />);
  expect(screen.getByRole("button", { name: "Create my account" })).toBeInTheDocument();
  expect(screen.queryByText(/Supabase/)).not.toBeInTheDocument();
  expect(screen.queryByText(/anon/i)).not.toBeInTheDocument();
});
