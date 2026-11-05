import { describe, expect, it, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { App } from "./App";
import { queryClient } from "../lib/queries";

// jsdom has no Tauri, so the app talks to the dev preview backend.
function renderApp() {
  return render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );
}

describe("first run → home", () => {
  beforeEach(() => queryClient.clear());

  it("creates a space and shows an honest, empty home", async () => {
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: /create our space/i }));
    fireEvent.change(screen.getByLabelText(/your name/i), { target: { value: "Johan" } });
    fireEvent.click(screen.getByRole("button", { name: /open our space/i }));

    await waitFor(() => expect(screen.getByText(/welcome to your little world/i)).toBeInTheDocument());
    // No invented stats: without a start date we ask for one instead of showing a number.
    expect(screen.getByRole("button", { name: /add your start date/i })).toBeInTheDocument();
    expect(screen.getByText(/your story starts here/i)).toBeInTheDocument();
    // Sidebar has exactly the 9 agreed destinations.
    const nav = screen.getAllByRole("button").filter((b) =>
      /^(Home|Journal|Timeline|Bucket List|Date Planner|Love Letters|Questions|Special Dates|Settings & Sync)$/.test(b.textContent ?? ""),
    );
    expect(nav).toHaveLength(9);
  });
});
