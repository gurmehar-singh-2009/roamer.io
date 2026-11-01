import { Application, Assets, ColorMatrixFilter, Container, Graphics, Sprite, Texture } from "pixi.js";
import Camera from "../objects/camera";
import Footprints from "../objects/footprints";
import { Z, UI_Z } from "./layers";
import { resolveAsset } from "../texturePack";
import { OBJ } from "../protocol";
import { WEAPONS, WEAPON_TEXTURES, TURRET_BULLET } from "../data/weapons";
import { ITEMS, ITEM_TEXTURES } from "../data/items";
import { ANIMAL_TEXTURES } from "../data/animals";
import { SHOP_TEXTURES } from "../data/shop";

import TreeSprite from "../assets/tree_1.png";
import StoneSprite from "../assets/stone.png";
import BushSprite from "../assets/world/bush.png";
import GoldSprite from "../assets/world/gold.png";
import CrateSprite from "../assets/world/crate.png";
import CaveSprite from "../assets/world/cave.png";
import CaveLayer from "./caves";
import Effects from "./effects";
import { crackStage, drawCracks } from "./cracks";
import LeafParticle from "../assets/particle_0.png";
import Ground0Sprite from "../assets/ground_0.png";
import Ground1Sprite from "../assets/ground_1.png";
import PlayerSprite from "../assets/head.png";
import Arm1Sprite from "../assets/arm_1.png";
import Arm2Sprite from "../assets/arm_2.png";
import BackPackSprite from "../assets/pack_0.png";

/** Textures of the player's body: head, arms, backpack. */
export const PLAYER_TEXTURES = [PlayerSprite, Arm1Sprite, Arm2Sprite, BackPackSprite];

/** Map size and biome bands (must match server/src/config/config.toml). */
export const MAP = {
  size: 16384,
  snowEnd: 4096,
  grassEnd: 8192,
  desertEnd: 12288,
  oceanX: 10240,
  oceanEnd: 14848,
};

/** Biome colors. */
export const BIOME_COLORS = {
  snow: 0xffffff,
  grass: 0x768f5a,
  desert: 0xa49a54,
  lava: 0x864341,
  ocean: 0x588fb2,
  sand: 0xd9c48c,
};

/** How much of the world you see (scaled to fit the window, like moomoo). */
const VIEW_W = 1750;
const VIEW_H = 984;

/** rgba(0, 0, 70, a) laid over buildings (0.35) and animals (0.15). */
const BUILDING_OVERLAY = 0.35;
const ANIMAL_OVERLAY = 0.15;
const OVERLAY_BLUE = 70 / 255;

/** A filter that lays rgba(0, 0, 70, alpha) over whatever it's applied to. */
function overlayFilter(alpha) {
  const k = 1 - alpha;
  const f = new ColorMatrixFilter();
  f.matrix = [k, 0, 0, 0, 0, 0, k, 0, 0, 0, 0, 0, k, 0, OVERLAY_BLUE * alpha, 0, 0, 0, 1, 0];
  return f;
}

/** Items that lie flat on the ground (traps, pads, platform, tunnel). */
const FLAT_ITEMS = new Set([15, 16, 18, 19, 20, 22, 25]);

/** Zoom setting (from the settings panel), 0.7 - 1.3. */
export function zoomSetting() {
  try {
    const z = Number(localStorage.getItem("roamer-zoom"));
    return z >= 0.7 && z <= 1.3 ? z : 1;
  } catch {
    return 1;
  }
}

/** Crate colors by tier: plain, gold, diamond, ruby, emerald. */
export const CRATE_COLORS = [0xc08a52, 0xf5d742, 0x6fe3ff, 0xff5a6a, 0x4ee08a];

/** Natural objects: texture and image size relative to the radius. */
const NATURAL = {
  [OBJ.TREE]: { texture: TreeSprite, size: 2.8 },
  [OBJ.STONE]: { texture: StoneSprite, size: 2.8 },
  [OBJ.BUSH]: { texture: BushSprite, size: 2.4 },
  [OBJ.GOLD]: { texture: GoldSprite, size: 2.6 },
  [OBJ.CAVE]: { texture: CaveSprite, size: 2.3 },
};

// Server tick length, used to turn per-tick speeds into per-second ones.
const SERVER_TICK_MS = 67;
// How quickly a bullet started at the shooter's on-screen position slides
// onto the server's path.
const PROJECTILE_CONVERGE = 12;

// Building HP bar size and fade timing.
const HP_W = 50;
const HP_H = 5;
const HP_SHOW_S = 2;
const HP_FADE_S = 0.5;
const HP_IDLE_ALPHA = 0.3;

export class Render {
  constructor(game) {
    this.game = game;
    this.app = new Application();
    // Starts over the grassland (seen behind the menu).
    this.camera = new Camera(MAP.oceanX / 2, (MAP.snowEnd + MAP.grassEnd) / 2);
    this.zoom = 1;
    // Extra zoom picked in the settings panel.
    this.zoomFactor = zoomSetting();

    this.world = new Container();
    this.world.sortableChildren = true;
    this.ui = new Container();

    this.ground = new Container();
    this.objects = new Container();
    this.ground.zIndex = Z.GROUND;
    this.objects.zIndex = Z.OBJECTS;
    this.objects.sortableChildren = true;

    this.worldBounds = new Graphics();
    this.worldBounds
      .rect(0, 0, MAP.oceanX, MAP.snowEnd)
      .fill(BIOME_COLORS.snow)
      .rect(0, MAP.snowEnd, MAP.oceanX, MAP.grassEnd - MAP.snowEnd)
      .fill(BIOME_COLORS.grass)
      .rect(0, MAP.grassEnd, MAP.oceanX, MAP.desertEnd - MAP.grassEnd)
      .fill(BIOME_COLORS.desert)
      .rect(0, MAP.desertEnd, MAP.oceanX, MAP.size - MAP.desertEnd)
      .fill(BIOME_COLORS.lava)
      .rect(MAP.oceanX, 0, MAP.oceanEnd - MAP.oceanX, MAP.size)
      .fill(BIOME_COLORS.ocean)
      .rect(MAP.oceanEnd, 0, MAP.size - MAP.oceanEnd, MAP.size)
      .fill(BIOME_COLORS.sand);
    // Arena floors live east of the map (see server/src/systems/arena_system.rs).
    this.worldBounds.rect(MAP.size + 200, 0, 30000, MAP.size).fill(0x4a5340);
    for (let slot = 0; slot < 8; slot++) {
      const x = 17500 + slot * 3000;
      this.worldBounds
        .rect(x, 1500, 2400, 2400)
        .fill(0x5d6b4e)
        .stroke({ width: 40, color: 0x3a3a3a });
    }
    this.worldBounds.zIndex = Z.BOUNDS;

    // Player buildings and animals get a dark blue overlay.
    this.overlayFilter = overlayFilter(BUILDING_OVERLAY);
    const layer = (z, filter = null) => {
      const c = new Container();
      c.zIndex = z;
      if (filter) c.filters = [filter];
      return c;
    };
    this.flatObjects = layer(Z.FLAT_OBJECTS);
    this.flatBuildings = layer(Z.FLAT_BUILDINGS, this.overlayFilter);
    this.buildings = layer(Z.BUILDINGS, this.overlayFilter);
    this.trees = layer(Z.TREES);
    this.animalLayer = layer(Z.ANIMALS, overlayFilter(ANIMAL_OVERLAY));
    this.caves = new CaveLayer(this);
    this.fx = new Effects(this);
    this.cracks = new Map();
    // Surface things hidden while underground.
    this.surfaceLayers = [this.flatObjects, this.flatBuildings, this.objects, this.buildings, this.trees, this.animalLayer];

    this.world.addChild(this.worldBounds, this.ground, ...this.surfaceLayers, this.caves.container, this.fx.container);

    this.textures = {};
    this.objectSprites = new Map();
    this.hpBars = new Map();
    this.projectiles = [];
    this.effects = [];
    this.leaves = [];
    this.footprints = null;

    this._overlayW = -1;
    this._overlayH = -1;
  }

  /** Every texture the game draws. */
  texturePaths() {
    return [
      ...new Set([
        TreeSprite,
        StoneSprite,
        BushSprite,
        GoldSprite,
        CrateSprite,
        CaveSprite,
        LeafParticle,
        Ground0Sprite,
        Ground1Sprite,
        TURRET_BULLET,
        ...PLAYER_TEXTURES,
        ...WEAPON_TEXTURES,
        ...ITEM_TEXTURES,
        ...ANIMAL_TEXTURES,
        ...SHOP_TEXTURES,
      ]),
    ].filter(Boolean);
  }

  /** Loads one texture, using the texture pack's version if there is one. */
  async loadTexture(path) {
    const src = resolveAsset(path);
    if (src === path) return Assets.load(path);
    const img = new Image();
    img.src = src;
    await img.decode();
    return Texture.from(img);
  }

  generateGround() {
    for (let i = 0; i < 512; i++) {
      const texture = Math.random() < 0.5 ? this.textures[Ground0Sprite] : this.textures[Ground1Sprite];
      const sprite = new Sprite(texture);
      sprite.anchor.set(0.5);
      sprite.x = Math.random() * MAP.oceanX;
      sprite.y = MAP.snowEnd + Math.random() * (MAP.grassEnd - MAP.snowEnd);
      sprite.scale.set(0.67 + Math.random() * 0.67);
      this.ground.addChild(sprite);
    }
  }

  async init() {
    await this.app.init({
      view: document.getElementById("mainCanvas"),
      background: "#000",
      resizeTo: window,
      resolution: Math.min(window.devicePixelRatio || 1, 2),
      antialias: true,
      preference: "webgpu",
      powerPreference: "high-performance",
    });

    const footprintGraphic = new Graphics().ellipse(0, 0, 11, 7).fill(0x000000);
    this.footprintTexture = this.app.renderer.generateTexture(footprintGraphic);
    footprintGraphic.destroy();

    await Promise.all(
      this.texturePaths().map(async (path) => {
        try {
          this.textures[path] = await this.loadTexture(path);
        } catch (e) {
          console.warn("failed to load", path, e);
        }
      }),
    );

    this.generateGround();
    this.fx.init(this.app);
    this.footprints = new Footprints(this.world, this.footprintTexture);
    this.footprints.container.zIndex = Z.FOOTPRINTS;

    this.overlay = new Graphics();
    this.overlay.zIndex = UI_Z.OVERLAY;
    this.ui.addChild(this.overlay);

    this.app.stage.addChild(this.world, this.ui);
    this.app.ticker.add(this.draw);
  }

  // ------------------------------------------------------------------
  // Objects

  /** Creates the sprite for a newly seen object. */
  createObjectSprite(object) {
    let texture;
    let size = 2.8;
    let tint = 0xffffff;
    const t = object.type_obj;
    if (NATURAL[t]) {
      texture = this.textures[NATURAL[t].texture];
      size = NATURAL[t].size;
    } else if (t >= OBJ.CRATE && t < OBJ.CRATE + 5) {
      texture = this.textures[CrateSprite];
      size = 2.4;
      tint = CRATE_COLORS[t - OBJ.CRATE];
    } else if (t >= OBJ.ITEM) {
      const item = ITEMS[t - OBJ.ITEM];
      texture = item && this.textures[item.texture];
      size = item?.size ?? 2.2;
    }
    const sprite = new Sprite(texture || Texture.WHITE);
    sprite.anchor.set(0.5);
    sprite.width = sprite.height = object.scale * size;
    sprite.rotation = object.dir;
    sprite.tint = tint;
    // Pads and traps lie flat under everything else.
    const item = t >= OBJ.ITEM ? ITEMS[t - OBJ.ITEM] : null;
    // Your own (or your clan's) pit traps are see-through, like in moomoo.
    if (item?.id === 15) sprite.alpha = 0.6;
    this.objectSprites.set(object.id, sprite);
    let parent = this.objects;
    if (t === OBJ.CAVE) parent = this.flatObjects;
    else if (t === OBJ.TREE) parent = this.trees;
    else if (item) parent = FLAT_ITEMS.has(item.id) ? this.flatBuildings : this.buildings;
    parent.addChild(sprite);
    sprite.baseScale = sprite.scale.x;
    // Buildings placed (or crates dropped) near us pop in. Ones that just
    // came into view at the edge don't.
    const me = this.game.me;
    if (object.showsHealth && me && Math.hypot(object.x - me.x, object.y - me.y) < 900) {
      this.fx.popIn(sprite);
    }
    return sprite;
  }

  /** Removes an object's sprite and HP bar. */
  removeObject(id) {
    const sprite = this.objectSprites.get(id);
    const object = this.game.objects.get(id);
    // Destroyed buildings and crates break apart; anything else just goes.
    if (sprite && object?.showsHealth && sprite.visible && !this.underground) {
      this.fx.breakApart(sprite, object.x, object.y, object.scale, this.fx.kindsFor(object));
    } else {
      sprite?.destroy();
    }
    this.objectSprites.delete(id);
    this.cracks.get(id)?.destroy();
    this.cracks.delete(id);
    this.hpBars.get(id)?.destroy({ children: true });
    this.hpBars.delete(id);
  }

  /** Small health bar under buildings and crates. */
  createHpBar() {
    const bar = new Container();
    bar.zIndex = Z.HEALTHBAR;
    bar.bg = new Graphics().roundRect(-HP_W / 2 - 3, -3, HP_W + 6, HP_H + 6, 4).fill(0x333333);
    bar.fg = new Graphics();
    bar.addChild(bar.bg, bar.fg);
    this.world.addChild(bar);
    return bar;
  }

  drawHpBar(bar, ratio, color) {
    const w = Math.max(0, HP_W * ratio);
    bar.fg.clear();
    if (w > 0) bar.fg.roundRect(-HP_W / 2, 0, w, HP_H, Math.min(2, w / 2)).fill(color);
  }

  // ------------------------------------------------------------------
  // Projectiles and effects

  /**
   * Adds a projectile. It starts at the shooter's on-screen (smoothed)
   * position so it visibly leaves their gun, then slides onto the server's
   * path over a few frames.
   */
  addProjectile({ id, x, y, dir, speed, range, weapon, owner }) {
    const art = weapon === 255 ? TURRET_BULLET : WEAPONS[weapon]?.bullet;
    const sprite = new Sprite(this.textures[art] || Texture.WHITE);
    sprite.anchor.set(0.75, 0.5);
    const isArrow = weapon >= 13 && weapon <= 16;
    sprite.scale.set(isArrow ? 0.45 : 0.8);
    sprite.rotation = dir;
    sprite.zIndex = Z.PROJECTILES;
    this.world.addChild(sprite);

    // Offset from where the server says it started to where the shooter is
    // drawn right now.
    let ox = 0;
    let oy = 0;
    const shooter = this.game.players.get(owner);
    if (shooter) {
      // Start at the muzzle drawn in their hands (the server's spawn
      // distance otherwise).
      const muzzle = shooter.muzzleDistance?.() ?? Math.hypot(x - shooter.x, y - shooter.y);
      ox = shooter._rx + Math.cos(dir) * muzzle - x;
      oy = shooter._ry + Math.sin(dir) * muzzle - y;
      if (Math.hypot(ox, oy) > 300) ox = oy = 0;
    }

    this.projectiles.push({
      id,
      x,
      y,
      ox,
      oy,
      dx: Math.cos(dir),
      dy: Math.sin(dir),
      speed: (speed * 1000) / SERVER_TICK_MS,
      left: range,
      sprite,
    });
    this._placeProjectile(this.projectiles[this.projectiles.length - 1]);
  }

  _placeProjectile(p) {
    p.sprite.x = p.x + p.ox;
    p.sprite.y = p.y + p.oy;
  }

  removeProjectile(id) {
    const i = this.projectiles.findIndex((p) => p.id === id);
    if (i === -1) return;
    this.projectiles[i].sprite.destroy();
    this.projectiles.splice(i, 1);
  }

  /** TNT blast: an expanding, fading ring plus a flash. */
  explosion(x, y, radius) {
    const g = new Graphics();
    g.zIndex = Z.EFFECTS;
    this.world.addChild(g);
    this.effects.push({ g, x, y, radius, t: 0 });
  }

  /** Leaf burst when a tree or bush is hit. */
  /** Particles for an object being hit (leaves, chips, splinters...). */
  hitObject(object, dir) {
    this.fx.hitObject(object, dir);
  }

  // ------------------------------------------------------------------
  // Frame

  draw = (ticker) => {
    const app = this.app;
    const delta = Math.min(ticker.deltaMS * 0.001, 0.1);

    if (this._overlayW !== app.screen.width || this._overlayH !== app.screen.height) {
      this._overlayW = app.screen.width;
      this._overlayH = app.screen.height;
      this.overlay
        .clear()
        .rect(0, 0, this._overlayW, this._overlayH)
        .fill({ color: 0x000046, alpha: 0.1 });
    }

    const game = this.game;
    const me = game.me;

    this.zoom = Math.max(app.screen.width / VIEW_W, app.screen.height / VIEW_H) * this.zoomFactor;
    this.world.scale.set(this.zoom);

    // Camera: follow the player (it stays put while dead or in the menu).
    if (me) {
      let tx = me._rx;
      let ty = me._ry;
      if (game.cameraMouseMode) {
        const halfW = app.screen.width / 2;
        const halfH = app.screen.height / 2;
        const dx = game._mouseX - halfW;
        const dy = game._mouseY - halfH;
        const d = Math.hypot(dx, dy);
        if (d > 1) {
          const mag = Math.min(d * 0.2, 130);
          tx += (dx / d) * mag;
          ty += (dy / d) * mag;
        }
      }
      this.camera.update(delta, tx, ty);
    }

    this.world.x = (app.screen.width >> 1) - this.camera.x * this.zoom;
    this.world.y = (app.screen.height >> 1) - this.camera.y * this.zoom;

    if (me && game.canAim()) {
      const aim = game._computeAim();
      if (aim !== null) game.lastAimDir = aim;
    }

    this.footprints?.update(ticker.deltaMS);

    for (const p of game.players.values()) {
      p.update(delta, p.isLocal ? game.lastAimDir : p.visualAim);
    }

    // In the caves the rock hides the surface (animals and buildings too).
    const underground = !!me?.underground;
    this.underground = underground;
    this.caves.update(underground);
    this.animalLayer.visible = !underground;
    for (const l of this.surfaceLayers) l.visible = !underground;

    const lerp = 1 - Math.pow(0.67, delta / 0.067);
    for (const a of game.animals.values()) {
      a.update(delta, lerp);
      if (underground) a.bar.visible = false;
    }

    this.drawObjects(delta);
    this.drawProjectiles(delta);
    this.drawEffects(delta);
    this.fx.update(delta);

    for (let i = this.leaves.length - 1; i >= 0; i--) {
      const leaf = this.leaves[i];
      leaf.sprite.zIndex = Z.LEAVES;
      leaf.update(delta);
      if (leaf.life <= 0) {
        leaf.sprite.destroy();
        this.leaves.splice(i, 1);
      }
    }
  };

  drawObjects(delta) {
    const app = this.app;
    const halfW = app.screen.width / 2 / this.zoom;
    const halfH = app.screen.height / 2 / this.zoom;
    const left = this.camera.x - halfW - 250;
    const right = this.camera.x + halfW + 250;
    const top = this.camera.y - halfH - 250;
    const bottom = this.camera.y + halfH + 250;
    const now = performance.now();

    for (const object of this.game.objects.values()) {
      let sprite = this.objectSprites.get(object.id) || this.createObjectSprite(object);
      const visible = object.x > left && object.x < right && object.y > top && object.y < bottom;
      sprite.visible = visible;

      let bar = this.hpBars.get(object.id);
      if (!bar && object.showsHealth) {
        bar = this.createHpBar();
        this.hpBars.set(object.id, bar);
      }
      if (!visible || this.underground) {
        if (bar) bar.visible = false;
        continue;
      }

      object.updateWiggle(delta);
      sprite.x = object.x + object.wiggleX;
      sprite.y = object.y + object.wiggleY;

      const item = object.item;
      if (item?.spin) sprite.rotation += item.spin * delta;
      if (item?.id === 24) {
        // TNT blinks faster as the fuse burns.
        const age = (now - object.seenAt) / 1000;
        sprite.tint = Math.sin(age * (4 + age * 6)) > 0 ? 0xffffff : 0xff9a8a;
      }
      if (item?.id === 17 && object.aim !== undefined) sprite.rotation = object.aim;

      // Slide the shown health towards the real one (damage quickly,
      // healing gently).
      if (object.displayHealth !== object.health) {
        const diff = object.health - object.displayHealth;
        const rate = diff < 0 ? 14 : 3;
        object.displayHealth =
          Math.abs(diff) < object.maxHealth * 0.002
            ? object.health
            : object.displayHealth + diff * Math.min(1, delta * rate);
        object.healthDirty = true;
      }
      if (object.showsHealth) this.updateCracks(object, sprite);

      if (bar) {
        if (object.healthDirty) {
          const isCrate = object.type_obj >= OBJ.CRATE && object.type_obj < OBJ.CRATE + 5;
          const color = isCrate ? CRATE_COLORS[object.type_obj - OBJ.CRATE] : object.isMine ? 0x8ecc51 : 0xe74c3c;
          this.drawHpBar(bar, object.displayHealth / object.maxHealth, color);
          object.healthDirty = false;
        }
        const idle = (now - object.lastHealthChange) / 1000;
        const fade = Math.min(1, Math.max(0, (idle - HP_SHOW_S) / HP_FADE_S));
        bar.alpha = 1 - fade * (1 - HP_IDLE_ALPHA);
        bar.visible = true;
        bar.x = object.x;
        bar.y = object.y + object.scale + 12;
      }
    }
  }

  /** Cracks over a damaged building or crate (fewer as it heals). */
  updateCracks(object, sprite) {
    const stage = crackStage(object.displayHealth / object.maxHealth);
    let g = this.cracks.get(object.id);
    if (!g && stage === 0) return;
    if (!g) {
      g = new Graphics();
      sprite.parent.addChild(g);
      this.cracks.set(object.id, g);
    }
    if (g.stage === undefined || Math.abs(g.stage - stage) > 0.02 || (stage === 0 && g.stage !== 0)) {
      drawCracks(g, object.id, object.scale * 0.95, stage);
      g.stage = stage;
    }
    g.position.copyFrom(sprite.position);
    g.rotation = sprite.rotation;
    // Follows the pop-in / break-apart animations.
    g.scale.set(sprite.scale.x / sprite.baseScale);
    g.visible = sprite.visible;
    g.alpha = sprite.alpha;
  }

  drawProjectiles(delta) {
    const decay = Math.exp(-PROJECTILE_CONVERGE * delta);
    for (let i = this.projectiles.length - 1; i >= 0; i--) {
      const p = this.projectiles[i];
      const step = Math.min(p.speed * delta, p.left);
      p.x += p.dx * step;
      p.y += p.dy * step;
      p.left -= step;
      p.ox *= decay;
      p.oy *= decay;
      this._placeProjectile(p);
      if (p.left <= 0) {
        p.sprite.destroy();
        this.projectiles.splice(i, 1);
      }
    }
  }

  drawEffects(delta) {
    for (let i = this.effects.length - 1; i >= 0; i--) {
      const e = this.effects[i];
      e.t += delta / 0.6;
      const r = e.radius * (0.3 + 0.7 * Math.min(1, e.t * 1.6));
      e.g
        .clear()
        .circle(e.x, e.y, r)
        .fill({ color: 0xffb347, alpha: Math.max(0, 0.45 * (1 - e.t)) })
        .circle(e.x, e.y, r)
        .stroke({ width: 10, color: 0xff6a3d, alpha: Math.max(0, 1 - e.t) });
      if (e.t >= 1) {
        e.g.destroy();
        this.effects.splice(i, 1);
      }
    }
  }
}
