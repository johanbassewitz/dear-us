import type { ScrapFrame, ScrapLayout, ScrapPaper } from "../../lib/api";

export type ResolvedLayout = Exclude<ScrapLayout, "auto">;

/** 'auto' picks a layout that suits how many photos the memory has. */
export function resolveLayout(layout: ScrapLayout, mediaCount: number): ResolvedLayout {
  if (layout !== "auto") return layout;
  if (mediaCount === 0) return "minimal";
  if (mediaCount === 1) return "magazine";
  if (mediaCount <= 3) return "polaroid";
  return "collage";
}

export const LAYOUT_OPTIONS: { value: ScrapLayout; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "polaroid", label: "Polaroid" },
  { value: "collage", label: "Collage" },
  { value: "magazine", label: "Magazine" },
  { value: "minimal", label: "Minimal" },
];
export const FRAME_OPTIONS: { value: ScrapFrame; label: string }[] = [
  { value: "none", label: "None" },
  { value: "polaroid", label: "Polaroid" },
  { value: "tape", label: "Taped" },
  { value: "rounded", label: "Rounded" },
  { value: "vintage", label: "Vintage" },
];
export const PAPER_OPTIONS: { value: ScrapPaper; label: string }[] = [
  { value: "none", label: "Plain" },
  { value: "dots", label: "Dots" },
  { value: "lines", label: "Lines" },
  { value: "kraft", label: "Kraft" },
];

export const frameClass = (f: ScrapFrame) => (f === "none" ? "" : `frame-${f}`);
export const paperClass = (p: ScrapPaper) => (p === "none" ? "" : `paper-${p}`);
