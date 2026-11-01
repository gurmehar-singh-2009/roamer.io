import { Container, Graphics, Sprite } from "pixi.js";
import { Z } from "./layers";
import { OBJ } from "../protocol";

/** Particle looks for each kind of hit. Colors are picked at random. */
const KINDS = {
  leaves: { shape: "leaf", colors: [0x6f9a43, 0x86b356, 0x5c8a38], count: [4, 6], size: [16, 26], speed: [120, 260], life: 0.7 },
  berries: { shape: "circle", colors: [0xd8383f, 0xe85a5a], count: [2, 3], size: [8, 11], speed: [110, 220], life: 0.6 },
  chips: { shape: "square", colors: [0x9a9a9a, 0x7d7d7d, 0xb5b5b5], count: [5, 8], size: [6, 11], speed: [160, 320], life: 0.5 },
  gold: { shape: "square", colors: [0xf2cd4b, 0xd9a92b, 0xfff1a8], count: [5, 8], size: [5, 10], speed: [160, 320], life: 0.55 },
  sparks: { shape: "spark", colors: [0xffffff, 0xfff1a8], count: [2, 4], size: [10, 16], speed: [40, 90], life: 0.35 },
  splinters: { shape: "stick", colors: [0xa5734a, 0x8c5f3b, 0xc28b5c], count: [4, 7], size: [12, 18], speed: [150, 300], life: 0.55 },
  dust: { shape: "circle", colors: [0xcfc6b8, 0xb8ae9e, 0xe0d8cc], count: [5, 8], size: [10, 18], speed: [60, 160], life: 0.6, grow: true },
  blood: { shape: "circle", colors: [0xd23b3b, 0xb02a2a], count: [3, 5], size: [6, 10], speed: [90, 200], life: 0.4 },
};

/** Building item ids made of wood (splinters); the rest crumble into dust. */
const WOODEN_ITEMS = new Set([3, 6, 7, 8, 9, 10, 11, 12, 14, 18, 23]);

const rand = (a, b) => a + Math.random() * (b - a);
const pick = (list) => list[Math.floor(Math.random() * list.length)];
const easeOutBack = (t) => 1 + 2.7 * (t - 1) ** 3 + 1.7 * (t - 1) ** 2;

/**
 * Short-lived visual effects: hit particles, and the pop-in / break-apart
 * animations of buildings and players.
 */
export default class Effects {
  constructor(renderer) {
    this.renderer = renderer;
    this.container = new Container();
    this.container.zIndex = Z.EFFECTS;
    this.particles = [];
    this.anims = [];
  }

  /** Makes the particle textures (needs the Pixi renderer). */
  init(app) {
    const make = (draw) => {
      const g = new Graphics();
      draw(g);
      const t = app.renderer.generateTexture(g);
      g.destroy();
      return t;
    };
    this.textures = {
      circle: make((g) => g.circle(16, 16, 16).fill(0xffffff)),
      square: make((g) => g.roundRect(0, 0, 24, 24, 4).fill(0xffffff)),
      stick: make((g) => g.roundRect(0, 0, 32, 9, 4).fill(0xffffff)),
      leaf: make((g) => g.ellipse(16, 9, 16, 9).fill(0xffffff)),
      spark: make((g) => g.star(16, 16, 4, 16, 5).fill(0xffffff)),
    };
  }

  /** A burst of particles of one kind at (x, y), flying away around `dir`. */
  burst(kind, x, y, dir, spread = Math.PI * 0.9) {
    const k = KINDS[kind];
    if (!k || !this.textures) return;
    const count = Math.round(rand(k.count[0], k.count[1]));
    for (let n = 0; n < count; n++) {
      const s = new Sprite(this.textures[k.shape]);
      s.anchor.set(0.5);
      s.tint = pick(k.colors);
      const size = rand(k.size[0], k.size[1]);
      s.scale.set(size / 32);
      s.rotation = Math.random() * Math.PI * 2;
      s.position.set(x, y);
      this.container.addChild(s);
      const a = dir + (Math.random() - 0.5) * spread;
      const speed = rand(k.speed[0], k.speed[1]);
      this.particles.push({
        s,
        vx: Math.cos(a) * speed,
        vy: Math.sin(a) * speed,
        spin: (Math.random() - 0.5) * 10,
        life: k.life * rand(0.8, 1.2),
        age: 0,
        scale: size / 32,
        grow: !!k.grow,
      });
    }
  }

  /** The right particles for hitting an object. `dir` points at the hitter. */
  hitObject(object, dir) {
    const x = object.x + Math.cos(dir) * object.scale * 0.9;
    const y = object.y + Math.sin(dir) * object.scale * 0.9;
    for (const kind of this.kindsFor(object)) this.burst(kind, x, y, dir);
  }

  kindsFor(object) {
    switch (object.type_obj) {
      case OBJ.TREE:
        return ["leaves"];
      case OBJ.BUSH:
        return ["leaves", "berries"];
      case OBJ.STONE:
        return ["chips"];
      case OBJ.GOLD:
        return ["gold", "sparks"];
      default:
        if (object.item) return WOODEN_ITEMS.has(object.item.id) ? ["splinters"] : ["dust", "chips"];
        return ["splinters"]; // crates
    }
  }

  /** A player or animal was hurt. */
  hitCreature(x, y) {
    this.burst("blood", x, y, Math.random() * Math.PI * 2, Math.PI * 2);
  }

  /** Grows a sprite in from nothing (new buildings and players). */
  popIn(target, duration = 0.3) {
    const base = target.scale.x;
    target.scale.set(0.01);
    this.anims.push({ target, t: 0, duration, kind: "in", base });
  }

  /**
   * Breaks a display object apart: it swells, fades and is destroyed, with
   * a burst of `kinds` particles.
   */
  breakApart(target, x, y, radius, kinds) {
    for (const kind of kinds) {
      for (let i = 0; i < 2; i++) this.burst(kind, x, y, Math.random() * Math.PI * 2, Math.PI * 2);
    }
    this.burst("dust", x, y, 0, Math.PI * 2);
    const base = target.scale.x;
    this.anims.push({ target, t: 0, duration: 0.3, kind: "out", base, radius });
  }

  update(dt) {
    for (let i = this.particles.length - 1; i >= 0; i--) {
      const p = this.particles[i];
      p.age += dt;
      const drag = Math.exp(-4 * dt);
      p.vx *= drag;
      p.vy *= drag;
      p.s.x += p.vx * dt;
      p.s.y += p.vy * dt;
      p.s.rotation += p.spin * dt;
      const t = p.age / p.life;
      p.s.alpha = Math.max(0, 1 - t * t);
      p.s.scale.set(p.scale * (p.grow ? 1 + t : 1 - 0.4 * t));
      if (t >= 1) {
        p.s.destroy();
        this.particles.splice(i, 1);
      }
    }
    for (let i = this.anims.length - 1; i >= 0; i--) {
      const a = this.anims[i];
      a.t += dt / a.duration;
      const t = Math.min(1, a.t);
      if (a.target.destroyed) {
        this.anims.splice(i, 1);
        continue;
      }
      if (a.kind === "in") {
        a.target.scale.set(Math.max(0.01, easeOutBack(t)) * a.base);
        a.target.alpha = Math.min(1, t * 2);
      } else {
        a.target.scale.set(a.base * (1 + 0.25 * t));
        a.target.alpha = 1 - t;
      }
      if (t >= 1) {
        if (a.kind === "out") a.target.destroy({ children: true });
        this.anims.splice(i, 1);
      }
    }
  }
}
