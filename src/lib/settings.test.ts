import { describe, expect, it } from "vitest";
import { DEFAULT_PREFS, parsePrefs } from "./settings";
import { clampPos } from "../cupid/Cupid";

describe("parsePrefs", () => {
  it("falls back to defaults for missing, corrupt or wrong-typed values", () => {
    const p = parsePrefs({
      "ui.theme": "not json",
      "cupid.visible": '"yes"',
      "ui.lastPage": '"journal"',
    });
    expect(p["ui.theme"]).toBe(DEFAULT_PREFS["ui.theme"]);
    expect(p["cupid.visible"]).toBe(true);
    expect(p["ui.lastPage"]).toBe("journal");
  });
  it("rejects unknown themes", () => {
    expect(parsePrefs({ "ui.theme": '"neon"' })["ui.theme"]).toBe("paper");
    expect(parsePrefs({ "ui.theme": '"cozy-dark"' })["ui.theme"]).toBe("cozy-dark");
  });
});

describe("clampPos", () => {
  it("keeps Cupid inside the window", () => {
    expect(clampPos({ right: -50, bottom: 5000 }, 1200, 800)).toEqual({ right: 8, bottom: 800 - 112 - 8 });
    expect(clampPos({ right: 100, bottom: 100 }, 1200, 800)).toEqual({ right: 100, bottom: 100 });
  });
});
