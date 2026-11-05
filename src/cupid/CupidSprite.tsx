import type { CSSProperties } from "react";
import type { CupidMood } from "./Cupid";
import happy from "./assets/cupid-happy.webp";
import thinking from "./assets/cupid-thinking.webp";
import sleeping from "./assets/cupid-sleeping.webp";

const SPRITES = { happy, thinking, sleeping };

/** The original artwork, with independent wing movement on the happy pose. */
export function CupidSprite({ mood, flying = false, bank = 0, dragging = false }: {
  mood: CupidMood; flying?: boolean; bank?: number; dragging?: boolean;
}) {
  return (
    <span aria-hidden="true" className={`cupid-sprite ${flying ? "is-flying" : ""} ${dragging ? "is-held" : ""}`}
      style={{ "--cupid-bank": `${bank}deg` } as CSSProperties}>
      <span className={`cupid-sprite-pose ${mood === "sleeping" ? "cupid-breathe" : "cupid-float"}`}>
        {mood === "happy" ? <>
          <img src={happy} alt="" draggable={false} className="cupid-wing cupid-wing-left" />
          <img src={happy} alt="" draggable={false} className="cupid-wing cupid-wing-right" />
          <img src={happy} alt="" draggable={false} className="cupid-body" />
        </> : <img src={SPRITES[mood]} alt="" draggable={false} />}
      </span>
    </span>
  );
}
