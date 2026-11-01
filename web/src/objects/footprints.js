import { Container, Sprite } from "pixi.js";

/**
 * Pooled footprint particles — subtle trails left behind moving players.
 * Fixed pool: no runtime allocations, so no GC hitches.
 */
export default class Footprints {
  constructor(world, texture, opts = {}) {
    const {
      count = 48, // max footprints alive at once
      life = 6000, // ms until a footprint fully fades
      alpha = 0.08, // starting opacity (deliberately subtle)
      stepOffset = 9, // left/right offset from player centre
      width = 18,
      height = 10,
    } = opts;

    this.life = life;
    this.baseAlpha = alpha;
    this.stepOffset = stepOffset;

    this.container = new Container();
    this.container.zIndex = 0; // under players / shadows
    world.addChild(this.container);

    this.items = new Array(count);
    this.index = 0;
    this._side = 1;

    for (let i = 0; i < count; i++) {
      const sprite = new Sprite(texture);
      sprite.anchor.set(0.5);
      sprite.width = width;
      sprite.height = height;
      sprite.alpha = 0;
      sprite.visible = false;
      this.container.addChild(sprite);
      this.items[i] = { sprite, life: 0, max: life };
    }
  }

  /** Stamp a footprint at (x, y) facing the movement direction. */
  spawn(x, y, dir) {
    const item = this.items[this.index];
    this.index = (this.index + 1) % this.items.length;

    const perp = dir + Math.PI / 2;
    item.sprite.x = x + Math.cos(perp) * this.stepOffset * this._side;
    item.sprite.y = y + Math.sin(perp) * this.stepOffset * this._side;
    item.sprite.rotation = dir;
    item.sprite.visible = true;
    item.sprite.alpha = this.baseAlpha;
    item.max = this.life;
    item.life = this.life;

    this._side = -this._side; // alternate feet
  }

  /** Fade footprints out; called once per frame. */
  update(deltaMS) {
    const items = this.items;
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (!item.sprite.visible) continue;

      item.life -= deltaMS;
      if (item.life <= 0) {
        item.sprite.visible = false;
        item.sprite.alpha = 0;
      } else {
        item.sprite.alpha = this.baseAlpha * (item.life / item.max);
      }
    }
  }
}
