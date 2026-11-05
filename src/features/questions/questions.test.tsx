import { beforeAll, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { queryClient } from "../../lib/queries";
import { api } from "../../lib/api";
import { localIsoDate } from "../../lib/dates";
import { questionOfTheDay } from "../../lib/questions";

beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
});

describe("questions flow", () => {
  it("answers today's question, keeps a private mood, adds a custom question, and Home shows the same question", async () => {
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );
    await screen.findByText(/welcome to your little world/i);
    const today = localIsoDate();
    const expected = questionOfTheDay(today, "preview-space");

    // Home uses the same daily question and links to the page
    await screen.findByText(expected.text);
    fireEvent.click(screen.getByRole("button", { name: "Questions" }));
    expect(await screen.findByRole("heading", { name: expected.text, level: 2 })).toBeInTheDocument();
    expect((await api.questionDaily(today, "something-else")).questionId).toBe(expected.id); // first writer wins

    // partner's answer is locked until I answer
    expect(screen.getByText(/write yours first/i)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(`Your answer to: ${expected.text}`), { target: { value: "Our slow Sundays" } });
    fireEvent.click(screen.getByRole("button", { name: /keep my answer/i }));
    expect(await screen.findByText(/will appear here once you're connected/i)).toBeInTheDocument();
    const kept = await screen.findByRole("region", { name: new RegExp(String(new Date().getFullYear())) });
    expect(within(kept).getByText("Our slow Sundays")).toBeInTheDocument();

    // empty answers are refused
    await expect(api.questionAnswerSave(expected.id, "  ", today)).rejects.toThrow(/few words/i);

    // private mood
    expect(screen.getByText("Only you can see this")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: /calm/i }));
    fireEvent.click(screen.getByRole("button", { name: /save check-in/i }));
    await waitFor(async () => expect((await api.moodList())[0].mood).toBe("calm"));

    // custom question
    fireEvent.click(screen.getAllByRole("button", { name: /add a question/i })[0]);
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: /add question/i }));
    expect(await within(dialog).findByText(/write your question first/i)).toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText("Your question"), { target: { value: "What did we never say out loud?" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /add question/i }));
    await screen.findByText("What did we never say out loud?");
    expect((await api.questionCustomList()).length).toBe(1);

    // library "answer this" opens a dialog and keeps the answer for revisiting
    fireEvent.click(screen.getByRole("button", { name: "Answer this: What did we never say out loud?" }));
    const ans = await screen.findByRole("dialog");
    fireEvent.change(within(ans).getByLabelText(/your answer to/i), { target: { value: "That I'm still amazed." } });
    fireEvent.click(within(ans).getByRole("button", { name: /keep my answer/i }));
    await waitFor(async () => expect((await api.questionAnswers()).length).toBe(2));
  });
});
