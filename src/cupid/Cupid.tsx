import { useCallback, useEffect, useRef, useState } from "react";
import { BellRing, CalendarHeart, EyeOff, ImagePlus, MessageCircleHeart } from "lucide-react";
import { usePrefs } from "../lib/settings";
import { useNav } from "../app/navigation";
import { useProfile } from "../lib/queries";
import { useCupidStatus } from "./cupidApi";
import { CupidChat } from "./CupidChat";
import { toast } from "../components/ui/Toast";
import { CupidSprite } from "./CupidSprite";
import { clampPos, CUPID_WIDTH as W, CUPID_HEIGHT as H, useCupidFlight } from "./useCupidFlight";

export type CupidMood = "happy" | "thinking" | "sleeping";
const SLEEP_AFTER_MS = 2 * 60_000;
const DRAG_THRESHOLD = 4;
export { clampPos } from "./useCupidFlight";

export function Cupid() {
  const visible = usePrefs((s) => s.prefs["cupid.visible"]);
  const savedPos = usePrefs((s) => s.prefs["cupid.pos"]);
  const setPref = usePrefs((s) => s.set);
  const go = useNav((s) => s.go);
  const openJournal = useNav((s) => s.openJournal);
  const { data: profile } = useProfile();

  const [pos, setPos] = useState(() => clampPos(savedPos, window.innerWidth, window.innerHeight));
  const [open, setOpen] = useState(false);
  const [asleep, setAsleep] = useState(false);
  const [hoverAsk, setHoverAsk] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [held, setHeld] = useState(false);
  const [focused, setFocused] = useState(false);
  const [chatOpen, setChatOpen] = useState(false);
  const { data: cupid } = useCupidStatus();
  const drag = useRef<{ x: number; y: number; right: number; bottom: number; moved: boolean } | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const { flying, bank } = useCupidFlight({
    position: pos,
    setPosition: setPos,
    paused: !visible || open || chatOpen || asleep || held || focused,
  });

  // Doze off after a while without activity; wake on any interaction.
  useEffect(() => {
    let timer = setTimeout(() => setAsleep(true), SLEEP_AFTER_MS);
    const wake = () => {
      setAsleep(false);
      clearTimeout(timer);
      timer = setTimeout(() => setAsleep(true), SLEEP_AFTER_MS);
    };
    const events = ["pointermove", "keydown", "pointerdown", "wheel"] as const;
    events.forEach((e) => window.addEventListener(e, wake, { passive: true }));
    return () => {
      clearTimeout(timer);
      events.forEach((e) => window.removeEventListener(e, wake));
    };
  }, []);

  // Close the menu on outside click or Escape.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    setHeld(true);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, right: pos.right, bottom: pos.bottom, moved: false };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (!d.moved && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
    if (!d.moved) {
      d.moved = true;
      setDragging(true);
      setOpen(false);
    }
    setPos(clampPos({ right: d.right - dx, bottom: d.bottom - dy }, window.innerWidth, window.innerHeight));
  };
  const onPointerUp = () => {
    setHeld(false);
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (d.moved) {
      setDragging(false);
      setPref("cupid.pos", pos);
    } else {
      setOpen((o) => !o);
    }
  };

  const act = useCallback(
    (page: Parameters<typeof go>[0]) => {
      setOpen(false);
      go(page);
    },
    [go],
  );

  if (!visible) return chatOpen ? <CupidChat onClose={() => setChatOpen(false)} /> : null;

  const mood: CupidMood = open ? (hoverAsk ? "thinking" : "happy") : asleep ? "sleeping" : "happy";
  // Open the menu toward the side of the screen with more room.
  const onLeftHalf = pos.right > window.innerWidth / 2;
  const menuAbove = pos.bottom < window.innerHeight / 2;
  const name = profile?.me?.displayName;

  const item = "flex w-full items-center gap-3 rounded-[13px] border border-line bg-paper px-4 py-2.5 text-left text-[13px] font-semibold text-ink-soft transition-colors hover:border-accent hover:bg-accent-soft hover:text-accent-deep";

  return (
    <div ref={root} className="fixed z-[70]" style={{ right: pos.right, bottom: pos.bottom, width: W, height: H }}
      onFocusCapture={() => setFocused(true)}
      onBlurCapture={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) setFocused(false); }}>
      {open && (
        <div
          className={`animate-fade-up absolute w-[250px] ${menuAbove ? "bottom-full mb-2" : "top-full mt-2"} ${onLeftHalf ? "left-0" : "right-0"}`}
          role="menu"
          aria-label="Cupid"
        >
          <div className="mb-2 rounded-[18px] border border-line bg-paper px-4 py-3 shadow-[var(--shadow-lg)]">
            <div className="font-serif text-[17px] font-semibold text-accent-deep">Hi{name ? `, ${name}` : ""}! I'm Cupid ♡</div>
            <p className="mt-0.5 text-[12px] leading-snug text-muted">How can I help you today?</p>
          </div>
          <div className="flex flex-col gap-1.5 rounded-[18px] border border-line bg-paper-2/95 p-2 shadow-[var(--shadow-lg)] backdrop-blur">
            <button role="menuitem" className={item} onClick={() => {
                setOpen(false);
                openJournal({ mode: "edit", id: null });
              }}>
              <ImagePlus size={16} className="text-accent" /> Add memory
            </button>
            <button role="menuitem" className={item} onClick={() => act("special")}>
              <BellRing size={16} className="text-accent" /> Remind me
            </button>
            <button role="menuitem" className={item} onClick={() => act("planner")}>
              <CalendarHeart size={16} className="text-accent" /> Plan a date
            </button>
            <button
              role="menuitem"
              className={item}
              onMouseEnter={() => setHoverAsk(true)}
              onMouseLeave={() => setHoverAsk(false)}
              onClick={() => {
                setOpen(false);
                if (cupid?.enabled) setChatOpen(true);
                else {
                  go("settings");
                  toast(cupid?.cloud ? "Turn Cupid on in Settings → Cupid's brain, and he can chat ♡" : "Add your own free Gemini key in Settings → Cupid's brain, and Cupid can chat ♡");
                }
              }}
            >
              <MessageCircleHeart size={16} className="text-accent" /> Ask Cupid
              {!cupid?.enabled && <span className="ml-auto text-[10px] font-medium text-faint">set up</span>}
            </button>
            <div className="mt-1 flex items-center justify-between px-2 pb-0.5 text-[10.5px] text-faint">
              <span>Drag me anywhere ♡</span>
              <button
                onClick={() => {
                  setOpen(false);
                  setPref("cupid.visible", false);
                }}
                className="inline-flex items-center gap-1 hover:text-ink"
              >
                <EyeOff size={11} /> Hide
              </button>
            </div>
          </div>
        </div>
      )}

      <button
        aria-label="Cupid, your companion. Click for options, drag to move."
        aria-haspopup="menu"
        aria-expanded={open}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          drag.current = null;
          setHeld(false);
          setDragging(false);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen((o) => !o);
          }
        }}
        className={`h-full w-full touch-none select-none rounded-full transition-transform duration-200 hover:scale-[1.04] ${dragging ? "cursor-grabbing" : "cursor-grab"}`}
      >
        <CupidSprite mood={mood} flying={flying} bank={bank} dragging={dragging} />
      </button>
      {chatOpen && <CupidChat onClose={() => setChatOpen(false)} />}
    </div>
  );
}
