import { TextStyle } from "pixi.js";

/** One look for the whole HUD: see-through black panels at a few gray
 * levels, white for highlights, small rounded corners (like the page's
 * boxes in mainstyle.css). */
export const UI = {
  font: "GameFont",
  panel: 0x000000,
  stroke: "#3a3a3a",
  // Corner radius, inner padding and distance from the screen edges, for
  // every HUD panel (mainstyle.css uses the same numbers).
  radius: 6,
  pad: 10,
  margin: 16,
  // Panel darkness.
  alpha: {
    panel: 0.25,
    slot: 0.2,
    active: 0.4,
    tooltip: 0.6,
  },
};

/** Small spaced-out caption used as every panel's title. */
export function titleStyle() {
  return uiTextStyle({ size: 13, letterSpacing: 2, weight: "bold" });
}

export function uiTextStyle({
  size = 14,
  fill = "#ffffff",
  weight = "normal",
  letterSpacing = 0.5,
  wrap = 0,
} = {}) {
  const style = {
    fontSize: size,
    fontFamily: UI.font,
    fontWeight: weight,
    fill,
    stroke: { color: UI.stroke, width: Math.max(2, size * 0.2), join: "round" },
    letterSpacing,
  };
  if (wrap > 0) {
    style.wordWrap = true;
    style.wordWrapWidth = wrap;
  }
  return new TextStyle(style);
}

/** Redraws a Graphics as a standard HUD panel. */
export function drawPanel(g, w, h, alpha = UI.alpha.panel) {
  return g.clear().roundRect(0, 0, w, h, UI.radius).fill({ color: UI.panel, alpha });
}

export function easeOutBack(t, s = 1.70158) {
  const c3 = s + 1;
  return 1 + c3 * Math.pow(t - 1, 3) + s * Math.pow(t - 1, 2);
}
