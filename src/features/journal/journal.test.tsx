import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { invalidateJournal, queryClient } from "../../lib/queries";
import { api } from "../../lib/api";
import { useNav } from "../../app/navigation";
import { groupTimeline } from "../timeline/TimelinePage";

// Runs against the in-memory preview backend (no Tauri in jsdom).
beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
  // jsdom lacks these; TipTap touches them.
  Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  document.elementFromPoint = () => null;
});

function renderApp() {
  return render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );
}

describe("journal", () => {
  it("create → view → delete → undo → bin → restore", async () => {
    renderApp();
    await screen.findByText(/welcome to your little world/i);

    // New memory from the top bar opens the editor
    fireEvent.click(screen.getAllByRole("button", { name: /new memory/i })[0]);
    const title = await screen.findByLabelText("Title");
    // Saving without a title is refused with a gentle message
    fireEvent.click(screen.getByRole("button", { name: /save memory/i }));
    expect(await screen.findByText(/give this memory a title/i)).toBeInTheDocument();

    fireEvent.change(title, { target: { value: "Picnic by the sea" } });
    fireEvent.change(screen.getByLabelText("Where"), { target: { value: "Batroun" } });
    const tags = screen.getByLabelText("Tags");
    fireEvent.change(tags, { target: { value: "beach" } });
    fireEvent.keyDown(tags, { key: "Enter" });

    // Navigating away with unsaved changes asks first
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/leave without saving/i)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    fireEvent.click(screen.getByRole("button", { name: /save memory/i }));
    expect(await screen.findByRole("heading", { name: "Picnic by the sea" })).toBeInTheDocument();
    expect(screen.getByText("Batroun")).toBeInTheDocument();
    expect(screen.getByText("#beach")).toBeInTheDocument();

    // Delete (to the bin) and undo from the toast
    fireEvent.click(screen.getByRole("button", { name: /delete memory/i }));
    fireEvent.click(await screen.findByRole("button", { name: /move to recovery bin/i }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(async () => expect((await api.journalList()).map((e) => e.title)).toContain("Picnic by the sea"));

    // Delete again, then restore from the recovery bin
    const id = (await api.journalList())[0].id;
    await api.journalDelete(id);
    invalidateJournal();
    useNav.getState().openJournal({ mode: "trash" });
    fireEvent.click(await screen.findByRole("button", { name: /^restore$/i }));
    await waitFor(async () => expect((await api.journalTrash()).length).toBe(0));
  });

  it("search finds memories by word and tag", async () => {
    await api.journalCreate({
      title: "Cinema night",
      bodyJson: "",
      entryDate: "2024-02-01",
      entryTime: null,
      location: null,
      tags: ["movies"],
      media: [],
      coverMediaId: null,
      highlight: true,
      layout: "auto",
      frame: "none",
      paper: "none",
    });
    expect((await api.journalList({ text: "cinema" })).length).toBe(1);
    expect((await api.journalList({ tag: "movies" })).length).toBe(1);
    expect((await api.journalList({ highlightsOnly: true }))[0].title).toBe("Cinema night");
  });
});

describe("timeline grouping", () => {
  it("groups by year then month, newest first as given", () => {
    const mk = (date: string) => ({ kind: "entry" as const, id: date, title: date, date, time: null, text: "", major: false, linkedEntryId: null, cover: null });
    const g = groupTimeline([mk("2025-07-12"), mk("2025-07-01"), mk("2025-03-02"), mk("2023-05-14")]);
    expect(g.map((y) => y.year)).toEqual(["2025", "2023"]);
    expect(g[0].months.map((m) => m.items.length)).toEqual([2, 1]);
  });
});
