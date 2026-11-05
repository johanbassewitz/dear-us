import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Cupid } from "./Cupid";
import { cupidKeys } from "./cupidApi";
import { queryClient, keys } from "../lib/queries";
import { DEFAULT_PREFS, usePrefs } from "../lib/settings";

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  vi.stubGlobal("PointerEvent", MouseEvent);
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  Element.prototype.setPointerCapture = vi.fn();
  Element.prototype.scrollIntoView = vi.fn();
  queryClient.clear();
  queryClient.setQueryData(keys.profile, { me: null, partner: null, space: null });
  queryClient.setQueryData(cupidKeys.status, { enabled: true });
  queryClient.setQueryData(cupidKeys.history, []);
  usePrefs.setState({ prefs: { ...DEFAULT_PREFS } });
});
afterEach(() => {
  cleanup(); queryClient.clear(); vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks();
  Reflect.deleteProperty(Element.prototype, "setPointerCapture");
  Reflect.deleteProperty(Element.prototype, "scrollIntoView");
});
const advance = (ms: number) => act(() => { vi.advanceTimersByTime(ms); });
function setup() {
  render(<QueryClientProvider client={queryClient}><Cupid /></QueryClientProvider>);
  const button = screen.getByRole("button", { name: /Cupid, your companion/ });
  return { button, root: button.parentElement! };
}

it("flies without saving automatic positions and freezes for the menu and chat", () => {
  const { button, root } = setup();
  advance(14000);
  expect(Number.parseFloat(root.style.right)).toBeGreaterThan(28);
  expect(usePrefs.getState().prefs["cupid.pos"]).toEqual({ right: 28, bottom: 24 });
  fireEvent.keyDown(button, { key: "Enter" });
  const stopped = root.style.right;
  advance(20000);
  expect(root.style.right).toBe(stopped);
  fireEvent.click(screen.getByRole("menuitem", { name: "Ask Cupid" }));
  expect(screen.getByRole("dialog", { name: "Chat with Cupid" })).toBeInTheDocument();
  advance(20000);
  expect(root.style.right).toBe(stopped);
});

it("freezes on keyboard focus and saves a deliberate drag from its current flight position", () => {
  const { button, root } = setup();
  advance(14000);
  act(() => button.focus());
  const stopped = root.style.right;
  advance(20000);
  expect(root.style.right).toBe(stopped);
  fireEvent.pointerDown(button, { button: 0, clientX: 800, clientY: 700 });
  fireEvent.pointerMove(button, { clientX: 750, clientY: 670 });
  fireEvent.pointerUp(button);
  expect(usePrefs.getState().prefs["cupid.pos"]).toEqual({ right: Math.round(Number.parseFloat(stopped) + 50), bottom: 54 });
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

it("can still hide Cupid from the menu", () => {
  const { button } = setup();
  fireEvent.keyDown(button, { key: "Enter" });
  fireEvent.click(screen.getByRole("button", { name: "Hide" }));
  advance(20000);
  expect(screen.queryByRole("button", { name: /Cupid, your companion/ })).not.toBeInTheDocument();
});
