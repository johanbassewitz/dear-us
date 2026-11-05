import { beforeAll, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { queryClient } from "../../lib/queries";
import { api, type Plan } from "../../lib/api";
import { allIdeas, filterIdeas, formatMoney, groupPlans, moneyToText, NO_FILTERS, parseMoney, pickRandom } from "./plannerUtils";

beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
  Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  document.elementFromPoint = () => null;
});

const plan = (o: Partial<Plan>): Plan => ({
  id: "x", title: "T", description: "", date: null, time: null, location: "", budgetCents: null, currency: "USD", status: "idea",
  checklist: [], ideaRef: null, linkedEntryId: null, linkedEntryTitle: null, createdAt: "2025-01-01", updatedAt: "2025-01-01", ...o,
});

describe("planner logic", () => {
  it("merges the library with custom ideas and filters by ceilings", () => {
    const lib = allIdeas();
    expect(lib.length).toBeGreaterThan(50);
    expect(lib.every((i) => !i.custom)).toBe(true);
    const mine = allIdeas([{ id: "c1", title: "Mine", description: "", category: "romantic", budget: "free", duration: "short", setting: "indoor", mood: "cozy" }]);
    expect(mine[0]).toMatchObject({ id: "c1", custom: true });
    expect(mine).toHaveLength(lib.length + 1);

    const free = filterIdeas(mine, { ...NO_FILTERS, budget: "free" });
    expect(free.length).toBeGreaterThan(0);
    expect(free.every((i) => i.budget === "free")).toBe(true);
    const low = filterIdeas(mine, { ...NO_FILTERS, budget: "low" });
    expect(low.every((i) => ["free", "low"].includes(i.budget))).toBe(true);
    expect(low.length).toBeGreaterThan(free.length);
    const quickIndoor = filterIdeas(mine, { ...NO_FILTERS, duration: "short", setting: "indoor" });
    expect(quickIndoor.every((i) => i.duration === "short" && i.setting !== "outdoor")).toBe(true);
    expect(filterIdeas(mine, { ...NO_FILTERS, mood: "cozy", category: "romantic" }).some((i) => i.id === "c1")).toBe(true);
  });

  it("picks a random matching idea, avoiding a repeat", () => {
    const two = allIdeas().slice(0, 2);
    expect(pickRandom([])).toBeNull();
    expect(pickRandom(two, two[0].id, () => 0)?.id).toBe(two[1].id);
    expect(pickRandom(two.slice(0, 1), two[0].id)?.id).toBe(two[0].id);
  });

  it("formats and parses money with the currency's own decimals", () => {
    expect(formatMoney(4500, "USD")).toContain("45");
    expect(formatMoney(5000, "JPY")).toContain("5,000");
    expect(parseMoney("12.50", "USD")).toBe(1250);
    expect(parseMoney("12,5", "EUR")).toBe(1250);
    expect(parseMoney("500", "JPY")).toBe(500);
    expect(parseMoney("", "USD")).toBeNull();
    expect(parseMoney("abc", "USD")).toBeUndefined();
    expect(moneyToText(1250, "USD")).toBe("12.50");
  });

  it("groups plans: upcoming soonest first, ideas, then earlier", () => {
    const g = groupPlans(
      [
        plan({ id: "later", status: "planned", date: "2026-09-01" }),
        plan({ id: "soon", status: "planned", date: "2026-06-01" }),
        plan({ id: "idea" }),
        plan({ id: "past", status: "planned", date: "2025-01-01" }),
        plan({ id: "done", status: "done", date: "2026-01-01" }),
      ],
      "2026-05-01",
    );
    expect(g.upcoming.map((p) => p.id)).toEqual(["soon", "later"]);
    expect(g.ideas.map((p) => p.id)).toEqual(["idea"]);
    expect(g.earlier.map((p) => p.id)).toEqual(["done", "past"]);
  });
});

describe("date planner flow", () => {
  it("surprise us -> add to planner, plan a date -> done -> write the memory links it back -> delete with undo", async () => {
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );
    await screen.findByText(/welcome to your little world/i);
    fireEvent.click(screen.getByRole("button", { name: "Date Planner" }));
    expect(await screen.findByText(/no dates planned yet/i)).toBeInTheDocument();

    // inspiration: filter, shuffle, add as an idea
    fireEvent.click(screen.getByRole("tab", { name: "Date inspiration" }));
    fireEvent.change(await screen.findByLabelText("Budget (up to)"), { target: { value: "free" } });
    fireEvent.click(screen.getByRole("button", { name: "Surprise us" }));
    const another = await screen.findByRole("button", { name: "Another one" });
    expect(another).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add to planner" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save as an idea" }));
    await waitFor(async () => expect(await api.plannerList()).toHaveLength(1));
    const [idea] = await api.plannerList();
    expect(idea.status).toBe("idea");
    expect(idea.ideaRef).toBeTruthy();

    // add your own idea
    fireEvent.click(screen.getByRole("button", { name: /add your own idea/i }));
    fireEvent.click(await screen.findByRole("button", { name: "Add idea" }));
    expect(await screen.findByText(/give your idea a name/i)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Idea name"), { target: { value: "Midnight pancakes" } });
    fireEvent.click(screen.getByRole("button", { name: "Add idea" }));
    await waitFor(async () => expect(await api.ideaList()).toHaveLength(1));
    // it was saved as a "low" budget idea, so it appears once the free-only filter is cleared
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(await screen.findByRole("article", { name: "Midnight pancakes" })).toBeInTheDocument();

    // plan a date by hand
    fireEvent.click(screen.getByRole("tab", { name: "Our plans" }));
    fireEvent.click(screen.getByRole("button", { name: "Plan a date" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add to our planner" }));
    expect(await screen.findByText(/give this date a name/i)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/what.s the plan/i), { target: { value: "Pasta night" } });
    fireEvent.change(screen.getByLabelText("Status"), { target: { value: "planned" } });
    fireEvent.click(screen.getByRole("button", { name: "Add to our planner" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/pick a day/i);
    fireEvent.change(screen.getByLabelText("Day"), { target: { value: "2099-02-14" } });
    fireEvent.change(screen.getByLabelText("Budget"), { target: { value: "40" } });
    fireEvent.change(screen.getByLabelText("Add a checklist item"), { target: { value: "Buy fresh basil" } });
    fireEvent.click(screen.getByRole("button", { name: "Add to our planner" }));

    const card = await screen.findByRole("article", { name: "Pasta night" });
    expect(within(card).getByText(/\$40/)).toBeInTheDocument();
    expect(within(card).getByText(/0 of 1 ready/)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("checkbox", { name: "Buy fresh basil" }));
    await waitFor(() => expect(within(screen.getByRole("article", { name: "Pasta night" })).getByText(/1 of 1 ready/)).toBeInTheDocument());

    // done -> write the memory
    fireEvent.click(within(screen.getByRole("article", { name: "Pasta night" })).getByRole("button", { name: "Mark done" }));
    const done = await screen.findByRole("dialog");
    expect(within(done).getByText(/what a lovely date/i)).toBeInTheDocument();
    fireEvent.click(within(done).getByRole("button", { name: /write the memory/i }));
    const title = (await screen.findByLabelText("Title")) as HTMLInputElement;
    expect(title.value).toBe("Pasta night");
    fireEvent.click(screen.getByRole("button", { name: /save memory/i }));
    await screen.findByRole("heading", { name: "Pasta night" });
    const saved = (await api.plannerList()).find((p) => p.title === "Pasta night")!;
    expect(saved.status).toBe("done");
    expect(saved.linkedEntryId).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Date Planner" }));
    const again = await screen.findByRole("article", { name: "Pasta night" });
    expect(await within(again).findByRole("button", { name: /open memory/i })).toBeInTheDocument();

    // delete asks first, then can be undone
    fireEvent.click(within(again).getByRole("button", { name: /delete "pasta night"/i }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(async () => expect((await api.plannerList()).some((p) => p.title === "Pasta night")).toBe(false));
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(async () => expect((await api.plannerList()).some((p) => p.title === "Pasta night")).toBe(true));
  }, 30000);
});
