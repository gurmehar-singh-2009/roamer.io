/**
 * Cracks drawn over damaged buildings and crates. Each object always cracks
 * the same way (seeded by its id); more cracks show as its health drops, and
 * they disappear again as it heals.
 */

/** Most cracks shown (at very low health). */
export const MAX_CRACKS = 6;

/** How many cracks show at a health ratio, as a fraction (none above 90%):
 * 2.5 means two full cracks and a third half faded in. */
export function crackStage(ratio) {
  if (ratio >= 0.9) return 0;
  return Math.min(MAX_CRACKS, ((0.9 - ratio) / 0.9) * MAX_CRACKS);
}

/** Small seeded random number generator (mulberry32). */
function seeded(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The crack lines of an object: a list of polylines [[x, y], ...]. */
function crackLines(id, radius, count) {
  const rand = seeded(id * 2654435761);
  const cracks = [];
  for (let c = 0; c < count; c++) {
    const lines = [];
    // Each crack starts near the middle and zig-zags outwards, with a
    // smaller branch partway along.
    let angle = (c / MAX_CRACKS) * Math.PI * 2 + rand() * 0.8;
    let r = radius * (0.08 + rand() * 0.15);
    const line = [[Math.cos(angle) * r, Math.sin(angle) * r]];
    const steps = 3 + Math.floor(rand() * 2);
    let branch = null;
    for (let s = 0; s < steps; s++) {
      r += (radius * 0.75) / steps;
      angle += (rand() - 0.5) * 0.7;
      line.push([Math.cos(angle) * r, Math.sin(angle) * r]);
      if (s === 1) branch = { at: line[line.length - 1], angle };
    }
    lines.push(line);
    if (branch) {
      const turn = (rand() < 0.5 ? -1 : 1) * (0.5 + rand() * 0.4);
      const len = radius * (0.2 + rand() * 0.15);
      const [bx, by] = branch.at;
      lines.push([branch.at, [bx + Math.cos(branch.angle + turn) * len, by + Math.sin(branch.angle + turn) * len]]);
    }
    cracks.push(lines);
  }
  return cracks;
}

/**
 * Redraws `g` (centered on the object, in its rotated frame) with `stage`
 * cracks reaching out to `radius`; the last one fades in with the fraction.
 */
export function drawCracks(g, id, radius, stage) {
  g.clear();
  if (stage <= 0) return;
  const cracks = crackLines(id, radius, Math.ceil(stage));
  cracks.forEach((lines, c) => {
    const alpha = Math.min(1, stage - c);
    const path = () => {
      for (const line of lines) {
        g.moveTo(line[0][0], line[0][1]);
        for (let k = 1; k < line.length; k++) g.lineTo(line[k][0], line[k][1]);
      }
    };
    // A light edge under a dark line reads as a crack on any color.
    path();
    g.stroke({ width: 6, color: 0xffffff, alpha: 0.2 * alpha, cap: "round", join: "round" });
    path();
    g.stroke({ width: 3, color: 0x2a2420, alpha: 0.85 * alpha, cap: "round", join: "round" });
  });
}
