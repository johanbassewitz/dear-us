import { beforeAll, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "../../app/App";
import { queryClient } from "../../lib/queries";
import { api } from "../../lib/api";
import { localIsoDate } from "../../lib/dates";
import { addDays } from "./occurrence";

beforeAll(async () => {
  await api.profileSave({ myName: "Jo", partnerName: "Sam", spaceName: "Us", startDate: null });
});

describe("special dates flow", () => {
  it("add, show the countdown, edit validation, delete with undo", async () => {
    render(
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>,
    );
    await screen.findByText(/welcome to your little world/i);
    fireEvent.click(screen.getByRole("button", { name: "Special Dates" }));
    fireEvent.click((await screen.findAllByRole("button", { name: /add a date|add your first date/i }))[0]);

    fireEvent.click(await screen.findByRole("button", { name: /add to our calendar/i }));
    expect(await screen.findByText(/give this date a name/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("What are we celebrating?"), { target: { value: "Picnic day" } });
    fireEvent.change(screen.getByLabelText("Date"), { target: { value: addDays(localIsoDate(), 12) } });
    fireEvent.change(screen.getByLabelText("Repeats"), { target: { value: "none" } });
    fireEvent.change(screen.getByLabelText("Remind me"), { target: { value: "3" } });
    fireEvent.click(screen.getByRole("button", { name: /add to our calendar/i }));

    const hero = await screen.findByRole("region", { name: "Next special date" });
    expect(hero).toHaveTextContent("12");
    expect(hero).toHaveTextContent("Picnic day");

    fireEvent.click(screen.getByRole("button", { name: 'Delete "Picnic day"' }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Next special date" })).not.toBeInTheDocument());
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    expect(await screen.findByRole("region", { name: "Next special date" })).toHaveTextContent("Picnic day");
  });
});
