import { useState } from "react";
import { act, cleanup, fireEvent, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clampPos, flightTarget, useCupidFlight } from "./useCupidFlight";

let motion: EventTarget & { matches: boolean; addListener: () => void; removeListener: () => void };
beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  motion = Object.assign(new EventTarget(), { matches: false, addListener() {}, removeListener() {} });
  vi.stubGlobal("matchMedia", () => motion);
  vi.stubGlobal("PointerEvent", MouseEvent);
  Object.defineProperty(window, "innerWidth", { configurable: true, value: 1320 });
  Object.defineProperty(window, "innerHeight", { configurable: true, value: 860 });
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
const advance = (ms: number) => act(() => { vi.advanceTimersByTime(ms); });
function setup() {
  return renderHook(({ paused }) => {
    const [position, setPosition] = useState({ right: 28, bottom: 24 });
    return { position, setPosition, ...useCupidFlight({ position, setPosition, paused }) };
  }, { initialProps: { paused: false } });
}

describe("edge routes", () => {
  it("travels along the closest edge without crossing the center", () => {
    expect(flightTarget({ right: 28, bottom: 24 }, { width: 1320, height: 860 }, 1)).toEqual({ right: 208, bottom: 24 });
    expect(flightTarget({ right: 8, bottom: 300 }, { width: 1320, height: 860 }, 1)).toEqual({ right: 8, bottom: 480 });
    expect(flightTarget({ right: 500, bottom: 300 }, { width: 1320, height: 860 }, 1)).toBeNull();
  });
  it("turns back at corners and stays within small viewports", () => {
    expect(flightTarget({ right: 1162, bottom: 8 }, { width: 1320, height: 860 }, 1)).toEqual({ right: 982, bottom: 8 });
    expect(clampPos({ right: 900, bottom: 900 }, 320, 240)).toEqual({ right: 162, bottom: 120 });
    expect(flightTarget({ right: 8, bottom: 8 }, { width: 170, height: 140 }, 1)).toBeNull();
  });
});

describe("quiet flights", () => {
  it("rests, flies along the edge, and rests again", () => {
    const { result } = setup();
    advance(11000);
    expect(result.current.position).toEqual({ right: 28, bottom: 24 });
    advance(3000);
    expect(result.current.position.right).toBeGreaterThan(28);
    expect(result.current.position.bottom).toBe(24);
    advance(3000);
    expect(result.current.position).toEqual({ right: 208, bottom: 24 });
    expect(result.current.flying).toBe(false);
  });
  it("freezes a flight immediately when interaction pauses it and resumes after rest", () => {
    const { result, rerender } = setup();
    advance(14000);
    rerender({ paused: true });
    const stopped = result.current.position;
    advance(20000);
    expect(result.current.position).toEqual(stopped);
    expect(result.current.flying).toBe(false);
    rerender({ paused: false });
    advance(11000);
    expect(result.current.position).toEqual(stopped);
    advance(3000);
    expect(result.current.position).not.toEqual(stopped);
  });
  it("stops near the pointer and waits while the user is typing or scrolling", () => {
    const { result } = setup();
    advance(14000);
    const pos = result.current.position;
    fireEvent.pointerMove(window, { clientX: 1320 - pos.right - 75, clientY: 860 - pos.bottom - 56 });
    advance(16000);
    expect(result.current.position).toEqual(pos);
    fireEvent.pointerMove(window, { clientX: 400, clientY: 100 });
    fireEvent.wheel(window);
    advance(10000);
    expect(result.current.position).toEqual(pos);
    const input = document.createElement("textarea");
    document.body.append(input);
    act(() => input.focus());
    advance(20000);
    expect(result.current.position).toEqual(pos);
    act(() => { input.blur(); input.remove(); });
    advance(15000);
    expect(result.current.position).not.toEqual(pos);
  });
  it("honors live reduced-motion changes and page visibility", () => {
    const { result } = setup();
    act(() => { motion.matches = true; motion.dispatchEvent(new Event("change")); });
    advance(20000);
    expect(result.current.position).toEqual({ right: 28, bottom: 24 });
    act(() => { motion.matches = false; motion.dispatchEvent(new Event("change")); });
    advance(14000);
    expect(result.current.flying).toBe(true);
    Object.defineProperty(document, "hidden", { configurable: true, value: true });
    fireEvent(document, new Event("visibilitychange"));
    const pos = result.current.position;
    advance(20000);
    expect(result.current.position).toEqual(pos);
    Reflect.deleteProperty(document, "hidden");
    fireEvent(document, new Event("visibilitychange"));
    advance(15000);
    expect(result.current.position).not.toEqual(pos);
  });
  it("clamps on resize and cancels its work on unmount", () => {
    const { result, unmount } = setup();
    advance(14000);
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 320 });
    Object.defineProperty(window, "innerHeight", { configurable: true, value: 240 });
    fireEvent.resize(window);
    expect(result.current.position.right).toBeLessThanOrEqual(162);
    expect(result.current.position.bottom).toBeLessThanOrEqual(120);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
  it("stays paused while another window has focus", () => {
    const { result } = setup();
    advance(14000);
    fireEvent.blur(window);
    const pos = result.current.position;
    advance(20000);
    expect(result.current.position).toEqual(pos);
    fireEvent.focus(window);
    advance(15000);
    expect(result.current.position).not.toEqual(pos);
  });
  it("stops for scrollbar and touch scrolling inside a page", () => {
    const { result } = setup();
    advance(14000);
    const pos = result.current.position;
    const scroller = document.createElement("div");
    document.body.append(scroller);
    fireEvent.scroll(scroller);
    advance(10000);
    expect(result.current.position).toEqual(pos);
    scroller.remove();
  });
  it("does not fly when first mounted in an unfocused window, even after a pause change", () => {
    vi.mocked(document.hasFocus).mockReturnValue(false);
    const { result, rerender } = setup();
    advance(18000);
    expect(result.current.position).toEqual({ right: 28, bottom: 24 });
    rerender({ paused: true });
    rerender({ paused: false });
    advance(18000);
    expect(result.current.position).toEqual({ right: 28, bottom: 24 });
    fireEvent.focus(window);
    advance(14000);
    expect(result.current.position.right).toBeGreaterThan(28);
  });
});
