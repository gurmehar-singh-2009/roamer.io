import { Container, Graphics } from "pixi.js";
import { Z } from "./layers";
import { OBJ } from "../protocol";
import { WEAPONS } from "../data/weapons";

/** Distance between rock sample points (server/src/systems/cave_system.rs). */
export const SAMPLE = 50;
/** Squares per drawing chunk (each chunk is redrawn on its own). */
const CHUNK = 32;

const ROCK = 0x2b221b;
const FLOOR = 0x6e5a45;
const FLOOR_DARK = 0x5f4d3b;
const WALL = 0x1d1611;
/** Partly dug rock: between the rock and the floor, lighter as it's dug. */
const DUG_FROM = [0x3a, 0x2f, 0x26];
const DUG_TO = [0x5c, 0x4b, 0x3b];
/** Dig hole radius and reach (cave_system.rs: DIG_RADIUS, SHOVEL_DIG_RADIUS). */
const DIG_RADIUS = 60;
const SHOVEL_DIG_RADIUS = 85;
const PLAYER_RADIUS = 35;

const mix = (t) =>
  (Math.round(DUG_FROM[0] + (DUG_TO[0] - DUG_FROM[0]) * t) << 16) |
  (Math.round(DUG_FROM[1] + (DUG_TO[1] - DUG_FROM[1]) * t) << 8) |
  Math.round(DUG_FROM[2] + (DUG_TO[2] - DUG_FROM[2]) * t);

// Unit square corners and edge midpoints: TL, TR, BR, BL, top, right,
// bottom, left. Must match VERTS in cave_system.rs.
const VERTS = [
  [0, 0],
  [1, 0],
  [1, 1],
  [0, 1],
  [0.5, 0],
  [1, 0.5],
  [0.5, 1],
  [0, 0.5],
];
const [TL, TR, BR, BL, T, R, B, L] = [0, 1, 2, 3, 4, 5, 6, 7];

/** Floor polygon per marching-squares case (8 = TL open, 4 = TR, 2 = BR,
 * 1 = BL). Must match CASES in cave_system.rs. */
const CASES = [
  [],
  [L, B, BL],
  [B, R, BR],
  [L, R, BR, BL],
  [T, TR, R],
  [T, TR, R, B, BL, L],
  [T, TR, BR, B],
  [L, T, TR, BR, BL],
  [TL, T, L],
  [TL, T, B, BL],
  [TL, T, R, BR, B, L],
  [TL, T, R, BR, BL],
  [TL, TR, R, L],
  [TL, TR, R, B, BL],
  [TL, TR, BR, B, L],
  [TL, TR, BR, BL],
];

/** Walls (edges between two midpoints) per case. */
const WALLS = CASES.map((verts) =>
  verts.map((v, k) => [v, verts[(k + 1) % verts.length]]).filter(([a, b]) => a >= T && b >= T),
);

const key = (i, j) => i * 4096 + j;

/**
 * The underground layer: solid rock with the dug-out space drawn as floor
 * (marching squares over the open sample points, like the server). It covers
 * the surface while the player is in the caves.
 */
export default class CaveLayer {
  constructor(renderer) {
    this.renderer = renderer;
    this.container = new Container();
    this.container.zIndex = Z.CAVE;
    this.container.visible = false;

    this.rock = new Graphics();
    this.floor = new Container();
    this.lights = new Graphics();
    this.container.addChild(this.rock, this.floor, this.lights);

    this.open = new Set();
    // Partly dug points: key -> amount (1-254).
    this.dug = new Map();
    this.chunks = new Map();
    this.dirty = new Set();
  }

  isOpen(i, j) {
    return this.open.has(key(i, j));
  }

  /** Points from the server's cave packet. */
  addCells({ reset, cells, dug }) {
    if (reset) {
      this.open.clear();
      this.dug.clear();
      for (const g of this.chunks.values()) g.destroy();
      this.chunks.clear();
      this.dirty.clear();
    }
    for (const { x, y } of cells) {
      this.open.add(key(x, y));
      this.dug.delete(key(x, y));
      this.markDirty(x, y);
    }
    for (const { x, y, amount } of dug ?? []) {
      if (this.open.has(key(x, y))) continue;
      this.dug.set(key(x, y), amount);
      this.markDirty(x, y);
    }
  }

  /** The squares around a point (and so their chunks) need redrawing. */
  markDirty(x, y) {
    for (const [di, dj] of [[0, 0], [-1, 0], [0, -1], [-1, -1]]) {
      this.dirty.add(`${Math.floor((x + di) / CHUNK)},${Math.floor((y + dj) / CHUNK)}`);
    }
  }

  /** Marching-squares case of the square whose top-left point is (i, j). */
  caseAt(i, j) {
    return (
      (this.isOpen(i, j) << 3) | (this.isOpen(i + 1, j) << 2) | (this.isOpen(i + 1, j + 1) << 1) | this.isOpen(i, j + 1)
    );
  }

  /** Redraws one chunk of floor. */
  drawChunk(ck) {
    let g = this.chunks.get(ck);
    if (!g) {
      g = new Graphics();
      this.chunks.set(ck, g);
      this.floor.addChild(g);
    }
    g.clear();
    const [cx, cy] = ck.split(",").map(Number);
    const walls = [];
    let any = false;
    let hasFloor = false;
    // Partly dug rock first, under the floor.
    for (let i = cx * CHUNK; i < (cx + 1) * CHUNK; i++) {
      for (let j = cy * CHUNK; j < (cy + 1) * CHUNK; j++) {
        const amount = this.dug.get(key(i, j));
        if (!amount) continue;
        g.circle(i * SAMPLE, j * SAMPLE, SAMPLE * 0.62).fill(mix(amount / 255));
        any = true;
      }
    }
    for (let i = cx * CHUNK; i < (cx + 1) * CHUNK; i++) {
      for (let j = cy * CHUNK; j < (cy + 1) * CHUNK; j++) {
        const c = this.caseAt(i, j);
        if (c === 0) continue;
        any = true;
        hasFloor = true;
        const pts = [];
        for (const v of CASES[c]) pts.push((i + VERTS[v][0]) * SAMPLE, (j + VERTS[v][1]) * SAMPLE);
        g.poly(pts);
        for (const [a, b] of WALLS[c]) walls.push(i, j, a, b);
      }
    }
    if (!any) return;
    if (!hasFloor) return;
    g.fill(FLOOR);
    // Walls: a wide dark band for depth, then a thin crisp edge.
    const strokeWalls = (width, color, alpha) => {
      for (let k = 0; k < walls.length; k += 4) {
        const [i, j, a, b] = walls.slice(k, k + 4);
        g.moveTo((i + VERTS[a][0]) * SAMPLE, (j + VERTS[a][1]) * SAMPLE);
        g.lineTo((i + VERTS[b][0]) * SAMPLE, (j + VERTS[b][1]) * SAMPLE);
      }
      g.stroke({ width, color, alpha, cap: "round", join: "round" });
    };
    strokeWalls(26, FLOOR_DARK, 0.9);
    strokeWalls(9, WALL, 1);
  }

  /** Called every frame. */
  update(underground) {
    this.container.visible = underground;
    if (!underground) return;

    for (const ck of this.dirty) this.drawChunk(ck);
    this.dirty.clear();

    // Rock fills the view; the floor chunks sit on top of it.
    const r = this.renderer;
    const halfW = r.app.screen.width / 2 / r.zoom + 200;
    const halfH = r.app.screen.height / 2 / r.zoom + 200;
    const cam = r.camera;
    this.rock.clear().rect(cam.x - halfW, cam.y - halfH, halfW * 2, halfH * 2).fill(ROCK);

    // Only draw the chunks near the camera.
    const size = CHUNK * SAMPLE;
    const minX = Math.floor((cam.x - halfW) / size);
    const maxX = Math.floor((cam.x + halfW) / size);
    const minY = Math.floor((cam.y - halfH) / size);
    const maxY = Math.floor((cam.y + halfH) / size);
    for (const [ck, g] of this.chunks) {
      const [cx, cy] = ck.split(",").map(Number);
      g.visible = cx >= minX && cx <= maxX && cy >= minY && cy <= maxY;
    }

    // Light coming down through entrances.
    this.lights.clear();
    for (const o of r.game.objects.values()) {
      if (!isEntrance(o)) continue;
      if (Math.abs(o.x - cam.x) > halfW || Math.abs(o.y - cam.y) > halfH) continue;
      this.lights
        .circle(o.x, o.y, 70)
        .fill({ color: 0xfff1c4, alpha: 0.18 })
        .circle(o.x, o.y, 42)
        .fill({ color: 0xfff1c4, alpha: 0.25 });
    }

    // Where the next swing digs (holding a melee weapon).
    const me = r.game.me;
    const w = me && WEAPONS[me.weapon];
    if (w && !w.isGun && !w.ranged && !me.holdingItem) {
      const radius = w.id === 29 ? SHOVEL_DIG_RADIUS : DIG_RADIUS;
      const reach = PLAYER_RADIUS + radius * 0.6;
      const aim = r.game.lastAimDir;
      this.lights
        .circle(me._rx + Math.cos(aim) * reach, me._ry + Math.sin(aim) * reach, radius)
        .fill({ color: 0xffffff, alpha: 0.08 });
    }
  }
}

/** Natural cave entrances and Tunnels. */
export function isEntrance(o) {
  return o.type_obj === OBJ.CAVE || o.item?.id === 25;
}
