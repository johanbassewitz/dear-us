import { beforeAll, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { invalidateLetters, queryClient } from "../../lib/queries";
import { api, type LetterInput, type LetterSummary } from "../../lib/api";
import { countdown, fromLocalInput, group, readyToOpen, stillSealed, toLocalInput, toName } from "./lettersUtils";

beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
  Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  document.elementFromPoint = () => null;
});

const doc = (t: string) => JSON.stringify({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: t }] }] });
const inDays = (d: number) => new Date(Date.now() + d * 86_400_000).toISOString();

const sample = (o: Partial<LetterSummary>): LetterSummary => ({
  id: "x", senderId: "me", senderName: "Jo", recipientId: null, recipientName: null, mine: true, title: "T", kind: "letter",
  status: "sent", sentAt: "2026-01-01T00:00:00Z", unlockAt: null, showTitle: true, lockKind: "none", sealed: false, needsUnlock: false,
  unlockedAt: null, excerpt: "", photoCount: 0, cover: null, createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z", ...o,
});

describe("letters logic", () => {
  it("describes how long until a capsule opens", () => {
    const now = Date.parse("2026-06-01T12:00:00Z");
    expect(countdown("2026-06-01T12:00:00Z", now)).toBe("any moment now");
    expect(countdown("2026-06-01T12:05:00Z", now)).toBe("in 5 minutes");
    expect(countdown("2026-06-01T15:00:00Z", now)).toBe("in 3 hours");
    expect(countdown("2026-06-13T12:00:00Z", now)).toBe("in 12 days");
    expect(countdown("2027-06-01T12:00:00Z", now)).toBe("in about 12 months");
    expect(countdown("2030-06-01T12:00:00Z", now)).toBe("in about 4 years");
  });

  it("round-trips the date picker value through an instant", () => {
    const iso = fromLocalInput("2030-05-06T07:08")!;
    expect(new Date(iso).getFullYear()).toBe(2030);
    expect(toLocalInput(iso)).toBe("2030-05-06T07:08");
    expect(fromLocalInput("")).toBeNull();
    expect(toLocalInput(null)).toBe("");
  });

  it("sorts letters onto the right shelf and knows when a capsule is ready", () => {
    const now = Date.parse("2026-06-01T12:00:00Z");
    const mineLetter = sample({ id: "a" });
    const theirLetter = sample({ id: "b", mine: false, senderName: "Sam" });
    const sealedMine = sample({ id: "c", kind: "capsule", unlockAt: "2026-12-01T00:00:00Z" });
    const readyTheirs = sample({ id: "d", kind: "capsule", mine: false, unlockAt: "2026-05-01T00:00:00Z" });
    const draft = sample({ id: "e", status: "draft", sentAt: null });
    const all = [mineLetter, theirLetter, sealedMine, readyTheirs, draft];
    expect(group(all, "letters").sent.map((l) => l.id)).toEqual(["a"]);
    expect(group(all, "letters").received.map((l) => l.id)).toEqual(["b"]);
    expect(group(all, "capsules").sent.map((l) => l.id)).toEqual(["c"]);
    expect(group(all, "capsules").received.map((l) => l.id)).toEqual(["d"]);
    expect(group(all, "drafts").drafts.map((l) => l.id)).toEqual(["e"]);
    expect(stillSealed(sealedMine, now)).toBe(true);
    expect(readyToOpen(readyTheirs, now)).toBe(true);
    expect(readyToOpen(sealedMine, now)).toBe(false);
    expect(toName(sample({}))).toBe("your person");
  });
});

describe("preview backend letters", () => {
  const base: LetterInput = { title: "Hi", bodyJson: doc("hello"), kind: "letter", unlockAt: null, showTitle: true, mediaIds: [] };

  it("keeps drafts private to the sender and refuses to edit sent letters", async () => {
    const id = await api.letterSaveDraft(null, base);
    expect((await api.letterGet(id)).status).toBe("draft");
    await api.letterSend(id);
    expect((await api.letterGet(id)).status).toBe("sent");
    await expect(api.letterSaveDraft(id, base)).rejects.toThrow(/sent letters can't change/);
    await api.letterDelete(id);
    await api.letterRestore(id);
  });

  it("only seals capsules for a future moment", async () => {
    const past = await api.letterSaveDraft(null, { ...base, kind: "capsule", unlockAt: inDays(-1) });
    await expect(api.letterSend(past)).rejects.toThrow(/future/);
    const noDate = await api.letterSaveDraft(null, { ...base, kind: "capsule" });
    await expect(api.letterSend(noDate)).rejects.toThrow(/future/);
    const ok = await api.letterSaveDraft(null, { ...base, kind: "capsule", unlockAt: inDays(3), showTitle: false });
    await api.letterSend(ok);
    const sent = await api.letterGet(ok);
    expect(sent.sealed).toBe(true);
    expect(sent.lockKind).toBe("tlock");
    expect(sent.bodyJson).toContain("hello"); // the sender can read their own copy
  });
});

describe("letters page", () => {
  it("compose → validation → draft → send → sealed capsule → delete with undo", async () => {
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );
    await screen.findByText(/welcome to your little world/i);
    fireEvent.click(screen.getByRole("button", { name: "Love Letters" }));

    // honest empty states
    expect(await screen.findByText(/some words are worth waiting for/i)).toBeInTheDocument();
    expect(await screen.findByText(/nothing in your inbox yet/i)).toBeInTheDocument();
    expect(screen.getByText(/what sam writes to you will arrive here once you're connected/i)).toBeInTheDocument();

    // compose: a title is needed before sending
    fireEvent.click(screen.getAllByRole("button", { name: /write a letter/i })[0]);
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Send" }));
    expect(await within(dialog).findByText(/give it a title before sending/i)).toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "For the road" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Send" }));
    expect(await within(dialog).findByText(/write a few words first/i)).toBeInTheDocument();

    // a capsule needs a future date
    fireEvent.click(within(dialog).getByRole("radio", { name: "A time capsule" }));
    expect(within(dialog).getByRole("button", { name: "Seal capsule" })).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Opens on")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("radio", { name: "A letter" }));

    // save as a draft: it shows up under Drafts, only on this PC
    fireEvent.click(within(dialog).getByRole("button", { name: "Save draft" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    fireEvent.click(screen.getByRole("tab", { name: /drafts/i }));
    expect(await screen.findByRole("article", { name: "For the road" })).toBeInTheDocument();
    expect(screen.getByText(/drafts stay on this pc/i)).toBeInTheDocument();
    const [draft] = await api.letterList();
    expect(draft.status).toBe("draft");

    // send real letters through the api and see them on the shelves
    const sentId = await api.letterSaveDraft(null, { title: "Rainy day", bodyJson: doc("Stay warm."), kind: "letter", unlockAt: null, showTitle: true, mediaIds: [] });
    await api.letterSend(sentId);
    const capId = await api.letterSaveDraft(null, { title: "Open in spring", bodyJson: doc("Look up."), kind: "capsule", unlockAt: inDays(30), showTitle: true, mediaIds: [] });
    await api.letterSend(capId);
    invalidateLetters();

    fireEvent.click(screen.getByRole("tab", { name: "Letters" }));
    const sentCard = await screen.findByRole("article", { name: "Rainy day" });
    expect(within(sentCard).getByText(/will reach sam once you're connected/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "Time capsules" }));
    const capCard = await screen.findByRole("article", { name: "Open in spring" });
    expect(within(capCard).getByText(/opens in 3[01] days/i)).toBeInTheDocument();
    expect(screen.getByText(/next to open: /i)).toBeInTheDocument();

    // the sender sees their own capsule with a "sealed until" label
    fireEvent.click(within(capCard).getByRole("button", { name: "Read letter" }));
    const view = await screen.findByRole("dialog");
    expect(await within(view).findByText(/sealed until/i)).toBeInTheDocument();
    expect(within(view).getByText("Look up.")).toBeInTheDocument();
    fireEvent.click(within(view).getAllByRole("button", { name: "Close" })[0]);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    // delete asks first, then can be undone
    fireEvent.click(screen.getByRole("button", { name: /delete "open in spring"/i }));
    const confirm = await screen.findByRole("dialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(async () => expect((await api.letterList()).map((l) => l.title)).not.toContain("Open in spring"));
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(async () => expect((await api.letterList()).map((l) => l.title)).toContain("Open in spring"));
  });
});
