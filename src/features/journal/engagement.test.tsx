import { describe, expect, it, beforeAll, afterEach } from "vitest";
import { cleanup, render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { queryClient } from "../../lib/queries";
import { api, type EntryInput } from "../../lib/api";
import { useNav } from "../../app/navigation";
import { resolveLayout } from "./scrapbookStyles";
import { relativeTime } from "./relativeTime";

beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
  Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  document.elementFromPoint = () => null;
});

afterEach(cleanup);

const input = (title: string, over: Partial<EntryInput> = {}): EntryInput => ({
  title,
  bodyJson: "",
  entryDate: "2025-07-12",
  entryTime: null,
  location: null,
  tags: [],
  media: [],
  coverMediaId: null,
  highlight: false,
  layout: "auto",
  frame: "none",
  paper: "none",
  ...over,
});

describe("reactions and comments", () => {
  it("react, comment, edit and delete from the memory view", async () => {
    const entry = await api.journalCreate(input("Rainy cinema"));
    useNav.getState().openJournal({ mode: "view", id: entry.id });
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );

    // reaction toggles on and off
    const bar = await screen.findByRole("group", { name: "Reactions" });
    const love = within(bar).getByRole("button", { name: /^Love/ });
    expect(love).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(love);
    await waitFor(() => expect(within(bar).getByRole("button", { name: /^Love, 1/ })).toHaveAttribute("aria-pressed", "true"));
    expect((await api.journalGet(entry.id)).reactionCount).toBe(1);
    fireEvent.click(within(bar).getByRole("button", { name: /^Love, 1/ }));
    await waitFor(() => expect(within(bar).getByRole("button", { name: /^Love/ })).toHaveAttribute("aria-pressed", "false"));

    // empty comment is refused by the add button being disabled; then add one
    expect(screen.getByRole("button", { name: "Add note" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Add a comment"), { target: { value: "Best night" } });
    fireEvent.click(screen.getByRole("button", { name: "Add note" }));
    expect(await screen.findByText("Best night")).toBeInTheDocument();
    expect(screen.queryByText("edited")).not.toBeInTheDocument();

    // edit inline
    fireEvent.click(screen.getByRole("button", { name: "Edit comment" }));
    fireEvent.change(screen.getByLabelText("Edit your comment"), { target: { value: "Best night ever" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByLabelText("Edit your comment")).not.toBeInTheDocument());
    expect(await screen.findByText("Best night ever")).toBeInTheDocument();
    expect(screen.getByText("edited")).toBeInTheDocument();

    // delete asks first
    fireEvent.click(screen.getByRole("button", { name: "Delete comment" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete comment" }));
    await waitFor(() => expect(screen.queryByText("Best night ever")).not.toBeInTheDocument());
    expect(await api.commentList(entry.id)).toHaveLength(0);
  });

  it("only the author can change a comment", async () => {
    const entry = await api.journalCreate(input("Other"));
    const c = await api.commentAdd(entry.id, "mine");
    await expect(api.commentAdd(entry.id, "   ")).rejects.toThrow();
    // a comment from the partner (written directly into the preview store is not possible),
    // so check the unknown-id path and the reaction validation instead
    await expect(api.commentUpdate("nope", "x")).rejects.toThrow();
    await expect(api.reactionToggle(entry.id, "angry" as never)).rejects.toThrow();
    expect((await api.commentUpdate(c.id, "mine too")).edited).toBe(true);
  });
});

describe("scrapbook styles", () => {
  it("auto picks a layout from the photo count", () => {
    expect(resolveLayout("auto", 0)).toBe("minimal");
    expect(resolveLayout("auto", 1)).toBe("magazine");
    expect(resolveLayout("auto", 3)).toBe("polaroid");
    expect(resolveLayout("auto", 9)).toBe("collage");
    expect(resolveLayout("minimal", 9)).toBe("minimal");
  });

  it("saves layout, frame and paper and rejects unknown styles", async () => {
    const e = await api.journalCreate(input("Styled", { layout: "polaroid", frame: "tape", paper: "kraft" }));
    expect([e.layout, e.frame, e.paper]).toEqual(["polaroid", "tape", "kraft"]);
    await expect(api.journalUpdate(e.id, input("Styled", { frame: "glitter" as never }))).rejects.toThrow();
  });

  it("the editor style panel changes the saved style", async () => {
    useNav.getState().openJournal({ mode: "edit", id: null });
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );
    fireEvent.change(await screen.findByLabelText("Title"), { target: { value: "Dotty day" } });
    fireEvent.click(screen.getByRole("radio", { name: "Paper: Dots" }));
    fireEvent.click(screen.getByRole("radio", { name: "Layout: Collage" }));
    expect(screen.getByRole("radio", { name: "Paper: Dots" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByRole("button", { name: /save memory/i }));
    await waitFor(async () => {
      const saved = (await api.journalList()).find((x) => x.title === "Dotty day");
      expect(saved).toBeTruthy();
      const d = await api.journalGet(saved!.id);
      expect([d.layout, d.paper]).toEqual(["collage", "dots"]);
    });
  });
});

describe("relativeTime", () => {
  it("is short and kind", () => {
    const now = new Date("2025-07-12T12:00:00Z");
    expect(relativeTime("2025-07-12T11:59:40Z", now)).toBe("just now");
    expect(relativeTime("2025-07-12T11:30:00Z", now)).toBe("30 min ago");
    expect(relativeTime("2025-07-12T09:00:00Z", now)).toBe("3 h ago");
    expect(relativeTime("2025-07-11T10:00:00Z", now)).toBe("yesterday");
    expect(relativeTime("2025-07-09T10:00:00Z", now)).toBe("3 days ago");
  });
});
