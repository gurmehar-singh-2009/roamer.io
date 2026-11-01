import { OBJ } from "../protocol";
import { ITEMS } from "../data/items";

/** A map object: tree, stone, bush, gold rock, crate or building. */
export default class GameObject {
  constructor({ id, x, y, dir, scale, type_obj, health, max_health, owner }, myId) {
    this.id = id;
    this.x = x;
    this.y = y;
    this.dir = dir;
    this.scale = scale;
    this.type_obj = type_obj;
    this.owner = owner;
    this.health = health;
    this.maxHealth = max_health;
    // Shown health: slides towards `health` so bars and cracks change smoothly.
    this.displayHealth = health;
    this.item = type_obj >= OBJ.ITEM ? ITEMS[type_obj - OBJ.ITEM] : null;
    this.isMine = owner === myId;
    this.seenAt = performance.now();
    // When the health last changed (performance.now ms), drives the HP bar fade.
    this.lastHealthChange = -Infinity;
    this.healthDirty = true;
    this.wiggleX = 0;
    this.wiggleY = 0;
    this.hitDir = 0;
    this.hit = false;
  }

  /** Buildings and crates show a health bar. */
  get showsHealth() {
    return this.type_obj >= OBJ.CRATE;
  }

  setHealth(health, maxHealth) {
    this.health = health;
    this.maxHealth = maxHealth;
    this.lastHealthChange = performance.now();
    this.healthDirty = true;
  }

  onHit(dir) {
    this.hit = true;
    this.hitDir = dir;
    this.wiggleProgress = 0;
  }

  updateWiggle(dt) {
    if (!this.hit) return;

    const DURATION = 0.5;
    const AMPLITUDE = 6.7;

    this.wiggleProgress += dt / DURATION;
    if (this.wiggleProgress >= 1) {
      this.wiggleProgress = 0;
      this.wiggleX = 0;
      this.wiggleY = 0;
      this.hit = false;
      return;
    }

    const t = this.wiggleProgress;
    const displacement = AMPLITUDE * Math.sin(t * Math.PI * 2.5 + Math.PI) * Math.pow(1 - t, 1.5);
    this.wiggleX = displacement * Math.cos(this.hitDir);
    this.wiggleY = displacement * Math.sin(this.hitDir);
  }
}
