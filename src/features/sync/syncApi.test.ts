import { describe as group, expect, it } from "vitest";
import { describe, type SyncStatus } from "./syncApi";

const base: SyncStatus = {
  state: "up_to_date",
  configured: true,
  paired: true,
  signedIn: true,
  email: null,
  projectUrl: null,
  lastSyncAt: null,
  lastError: null,
  counts: { pendingOps: 0, awaitingDelivery: 0, uploadsPending: 0, downloadsPending: 0, failedUpdates: 0, openConflicts: 0 },
  devices: [],
  thisDevice: "d",
  relayBytes: 0,
  directEnabled: true,
  directPeer: false,
};

group("sync status in plain words", () => {
  it("is honest when sync isn't set up", () => {
    expect(describe(undefined).label).toBe("Saved on this PC");
    expect(describe({ ...base, state: "off" }).sentence).toMatch(/isn't set up/);
  });
  it("explains what's waiting", () => {
    const d = describe({ ...base, state: "pending", counts: { ...base.counts, pendingOps: 2, awaitingDelivery: 1 } }, "Sam");
    expect(d.sentence).toBe("Your memories are saved safely on this computer. 3 updates are waiting to reach Sam.");
  });
  it("reassures when offline or the cloud is full", () => {
    expect(describe({ ...base, state: "offline" }).sentence).toMatch(/saved here/);
    expect(describe({ ...base, state: "waiting_for_space" }, "Sam").sentence).toMatch(/wait safely here/);
  });
});
