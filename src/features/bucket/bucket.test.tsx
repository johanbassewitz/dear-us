import { beforeAll, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { queryClient } from "../../lib/queries";
import { api, type BucketItem } from "../../lib/api";
import { categoriesOf, filterItems, progressLabel, sortColumn } from "./bucketUtils";

beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
  Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  document.elementFromPoint = () => null;
});

const item = (o: Partial<BucketItem>): BucketItem => ({
  id: "x", title: "T", description: "", category: "", priority: "medium", status: "want", targetDate: null, completedAt: null,
  linkedEntryId: null, linkedEntryTitle: null, coverMediaId: null, cover: null, media: [], createdAt: "2025-01-01", updatedAt: "2025-01-01", ...o,
});

describe("bucket logic", () => {
  it("sorts by priority then date, filters and counts", () => {
    const a = item({ id: "a", title: "Low", priority: "low", category: "Food" });
    const b = item({ id: "b", title: "High later", priority: "high", targetDate: "2026-05-01", category: "travel" });
    const c = item({ id: "c", title: "High sooner", priority: "high", targetDate: "2026-01-01", category: "Travel" });
    expect(sortColumn([a, b, c], "want").map((i) => i.id)).toEqual(["c", "b", "a"]);
    expect(filterItems([a, b, c], { text: "later", status: null, category: null }).map((i) => i.id)).toEqual(["b"]);
    expect(filterItems([a, b, c], { text: "", status: null, category: "TRAVEL" }).map((i) => i.id)).toEqual(["b", "c"]);
    expect(categoriesOf([a, b, c])).toEqual(["Food", "travel"]);
    expect(progressLabel(7, 20)).toBe("7 of 20 done together");
  });
});

describe("bucket list flow", () => {
  it("add → plan → done → write the memory links it back → delete with undo", async () => {
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );
    await screen.findByText(/welcome to your little world/i);
    fireEvent.click(screen.getByRole("button", { name: "Bucket List" }));
    fireEvent.click((await screen.findAllByRole("button", { name: /new dream/i }))[0]);

    // a title is required
    fireEvent.click(await screen.findByRole("button", { name: /add to our list/i }));
    expect(await screen.findByText(/give this dream a title/i)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("What do you dream of?"), { target: { value: "Sleep under the stars" } });
    fireEvent.change(screen.getByLabelText("Category"), { target: { value: "Adventure" } });
    fireEvent.change(screen.getByLabelText("Priority"), { target: { value: "high" } });
    fireEvent.click(screen.getByRole("button", { name: /add to our list/i }));

    const card = await screen.findByRole("article", { name: "Sleep under the stars" });
    expect(within(card).getByText("Adventure")).toBeInTheDocument();
    expect(screen.getByText("0 of 1 done together")).toBeInTheDocument();
    expect((await api.dashboardSummary()).bucketTotal).toBe(1);

    fireEvent.click(within(card).getByRole("button", { name: "Start planning" }));
    const planned = await screen.findByRole("region", { name: "Making plans" });
    fireEvent.click(await within(planned).findByRole("button", { name: "Mark done" }));

    // celebrate dialog → write the memory (opens the journal prefilled)
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/you did it/i)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: /write the memory/i }));
    const title = (await screen.findByLabelText("Title")) as HTMLInputElement;
    expect(title.value).toBe("Sleep under the stars");
    fireEvent.click(screen.getByRole("button", { name: /save memory/i }));
    await screen.findByRole("heading", { name: "Sleep under the stars" });

    const [saved] = await api.bucketList();
    expect(saved.status).toBe("done");
    expect(saved.completedAt).toBeTruthy();
    expect(saved.linkedEntryId).toBeTruthy();
    expect((await api.dashboardSummary()).bucketDone).toBe(1);

    // back on the list the card offers the memory
    fireEvent.click(screen.getByRole("button", { name: "Bucket List" }));
    const done = await screen.findByRole("region", { name: "Done together" });
    expect(await within(done).findByRole("button", { name: /open memory/i })).toBeInTheDocument();
    expect(screen.getByText("1 of 1 done together")).toBeInTheDocument();

    // reopen clears completion
    fireEvent.click(within(done).getByRole("button", { name: "Reopen" }));
    await waitFor(async () => expect((await api.bucketList())[0].completedAt).toBeNull());

    // delete asks first, then can be undone
    fireEvent.click(await screen.findByRole("button", { name: /delete "sleep under the stars"/i }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    await waitFor(async () => expect(await api.bucketList()).toHaveLength(0));
    expect((await api.dashboardSummary()).bucketTotal).toBe(0);
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(async () => expect(await api.bucketList()).toHaveLength(1));
  });
});
