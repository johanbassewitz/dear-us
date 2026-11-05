import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";

export type CupidPosition = { right: number; bottom: number };
export const CUPID_WIDTH = 150;
export const CUPID_HEIGHT = 112;
const MARGIN = 8;
const REST_MS = 12_000;
const FLIGHT_MS = 4_000;

export function clampPos(pos: CupidPosition, width: number, height: number): CupidPosition {
  return {
    right: Math.round(Math.min(Math.max(pos.right, MARGIN), Math.max(MARGIN, width - CUPID_WIDTH - MARGIN))),
    bottom: Math.round(Math.min(Math.max(pos.bottom, MARGIN), Math.max(MARGIN, height - CUPID_HEIGHT - MARGIN))),
  };
}

/** A short leg along one edge. A manually placed central Cupid stays put. */
export function flightTarget(pos: CupidPosition, viewport: { width: number; height: number }, direction: number): CupidPosition | null {
  const maxRight = Math.max(MARGIN, viewport.width - CUPID_WIDTH - MARGIN);
  const maxBottom = Math.max(MARGIN, viewport.height - CUPID_HEIGHT - MARGIN);
  const horizontalDistance = Math.min(pos.bottom - MARGIN, maxBottom - pos.bottom);
  const verticalDistance = Math.min(pos.right - MARGIN, maxRight - pos.right);
  if (Math.min(horizontalDistance, verticalDistance) > 40) return null;
  const axis = horizontalDistance <= verticalDistance ? "right" : "bottom";
  const max = axis === "right" ? maxRight : maxBottom;
  let target = Math.min(max, Math.max(MARGIN, pos[axis] + direction * 180));
  if (Math.abs(target - pos[axis]) < 24) target = Math.min(max, Math.max(MARGIN, pos[axis] - direction * 180));
  return Math.abs(target - pos[axis]) < 24 ? null : { ...pos, [axis]: target };
}

export function useCupidFlight({ position, setPosition, paused }: {
  position: CupidPosition;
  setPosition: Dispatch<SetStateAction<CupidPosition>>;
  paused: boolean;
}) {
  const latest = useRef({ position, setPosition });
  latest.current = { position, setPosition };
  const pointer = useRef<{ x: number; y: number } | null>(null);
  const windowFocused = useRef(document.hasFocus());
  const direction = useRef(1);
  const [flying, setFlying] = useState(false);
  const [bank, setBank] = useState(0);

  useEffect(() => {
    const motion = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    let timer: ReturnType<typeof setTimeout>;
    let nextFlight = Date.now() + REST_MS;
    let leg: { from: CupidPosition; to: CupidPosition; started: number } | null = null;
    const publish = (pos: CupidPosition) => {
      latest.current.position = pos;
      latest.current.setPosition(pos);
    };
    const stop = () => {
      leg = null;
      nextFlight = Date.now() + REST_MS;
      setFlying(false);
      setBank(0);
    };
    const near = () => {
      if (!pointer.current) return false;
      const left = window.innerWidth - latest.current.position.right - CUPID_WIDTH;
      const top = window.innerHeight - latest.current.position.bottom - CUPID_HEIGHT;
      return pointer.current.x >= left - 90 && pointer.current.x <= left + CUPID_WIDTH + 90
        && pointer.current.y >= top - 90 && pointer.current.y <= top + CUPID_HEIGHT + 90;
    };
    const blocked = () => paused || !windowFocused.current || motion?.matches || document.hidden || near()
      || !!document.activeElement?.closest('input, textarea, select, [contenteditable="true"]')
      || !!document.querySelector('[role="dialog"], [aria-modal="true"]');
    const tick = () => {
      const now = Date.now();
      if (blocked()) {
        if (leg) stop();
        nextFlight = now + REST_MS;
      } else if (!leg && now >= nextFlight) {
        const from = latest.current.position;
        const to = flightTarget(from, { width: window.innerWidth, height: window.innerHeight }, direction.current);
        if (to) {
          leg = { from, to, started: now };
          setFlying(true);
          setBank(to.right > from.right ? -5 : to.right < from.right ? 5 : 0);
        } else nextFlight = now + REST_MS;
      }
      if (leg) {
        const t = Math.min(1, (now - leg.started) / FLIGHT_MS);
        const eased = t * t * (3 - 2 * t);
        publish({ right: leg.from.right + (leg.to.right - leg.from.right) * eased, bottom: leg.from.bottom + (leg.to.bottom - leg.from.bottom) * eased });
        if (t === 1) {
          direction.current *= -1;
          stop();
        }
      }
      timer = setTimeout(tick, leg ? 32 : 250);
    };
    const onPointer = (e: PointerEvent) => {
      pointer.current = { x: e.clientX, y: e.clientY };
      if (near()) stop();
    };
    const onLeave = () => { pointer.current = null; stop(); };
    const onResize = () => {
      stop();
      publish(clampPos(latest.current.position, window.innerWidth, window.innerHeight));
    };
    const onVisibility = () => { pointer.current = null; stop(); };
    const onBlur = () => { windowFocused.current = false; stop(); };
    const onFocus = () => { windowFocused.current = true; stop(); };
    // Resting uses a low-frequency timer; only an actual flight updates position.
    stop();
    timer = setTimeout(tick, 250);
    window.addEventListener("pointermove", onPointer, { passive: true });
    document.addEventListener("pointerleave", onLeave);
    window.addEventListener("keydown", stop);
    window.addEventListener("wheel", stop, { passive: true });
    window.addEventListener("scroll", stop, { capture: true, passive: true });
    window.addEventListener("pointerdown", stop);
    window.addEventListener("focusin", stop);
    window.addEventListener("focusout", stop);
    window.addEventListener("resize", onResize);
    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    motion?.addEventListener("change", stop);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("pointermove", onPointer);
      document.removeEventListener("pointerleave", onLeave);
      window.removeEventListener("keydown", stop);
      window.removeEventListener("wheel", stop);
      window.removeEventListener("scroll", stop, true);
      window.removeEventListener("pointerdown", stop);
      window.removeEventListener("focusin", stop);
      window.removeEventListener("focusout", stop);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
      motion?.removeEventListener("change", stop);
    };
  }, [paused]);

  return { flying, bank };
}
