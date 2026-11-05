import { StrictMode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "../lib/queries";
import { cupidApi, cupidKeys } from "./cupidApi";
import { CupidChat } from "./CupidChat";

beforeEach(() => {
  queryClient.clear();
  queryClient.setQueryData(cupidKeys.history, []);
  // Modern Chromium/WebView2 returns a Promise; older engines return void.
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(() => Promise.resolve()),
  });
});

afterEach(() => {
  cleanup();
  queryClient.clear();
  vi.restoreAllMocks();
  Reflect.deleteProperty(Element.prototype, "scrollIntoView");
});

it("keeps chat usable when scrolling returns a Promise, including sending and closing", async () => {
  const reply = { id: "reply", role: "model" as const, content: "How about a picnic?", createdAt: "2026-11-05" };
  vi.spyOn(cupidApi, "chat").mockResolvedValue(reply);
  vi.spyOn(cupidApi, "history").mockResolvedValue([reply]);
  const { unmount } = render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <CupidChat onClose={() => {}} />
      </QueryClientProvider>
    </StrictMode>,
  );

  expect(screen.getByRole("dialog", { name: "Chat with Cupid" })).toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "Suggest a date" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(await screen.findByText("How about a picnic?")).toBeInTheDocument();
  expect(() => unmount()).not.toThrow();
});
