import { Container, Graphics, Sprite, Text, Texture } from "pixi.js";
import { ANIMALS } from "../data/animals";
import { Z } from "../render/layers";
import { uiTextStyle } from "../ui/theme";

const BAR_W = 70;
const BAR_H = 6;

/** An animal (or boss). Positions are smoothed towards server updates. */
export default class Animal {
  /** `layer` holds the sprite (it has the overlay filter); bars go in `world`. */
  constructor({ id, kind, x, y, dir, health }, world, textures, layer = world) {
    this.id = id;
    this.kind = kind;
    this.def = ANIMALS[kind] || ANIMALS[1];
    this.x = x;
    this.y = y;
    this.dir = dir;
    this.health = health;
    this.lastUpdate = performance.now();

    this.sprite = new Sprite(textures[this.def.texture] || Texture.WHITE);
    this.sprite.anchor.set(0.5);
    this.sprite.width = this.sprite.height = this.def.size;
    if (this.def.tint) this.sprite.tint = this.def.tint;
    this.sprite.zIndex = Z.ANIMALS;
    this.sprite._rx = x;
    this.sprite._ry = y;
    this.sprite._rot = dir - Math.PI / 2;
    layer.addChild(this.sprite);

    // Health bar (and a name for bosses).
    this.bar = new Container();
    this.bar.zIndex = Z.HEALTHBAR;
    this.barBg = new Graphics().roundRect(-BAR_W / 2 - 3, -3, BAR_W + 6, BAR_H + 6, 4).fill(0x333333);
    this.barFg = new Graphics();
    this.bar.addChild(this.barBg, this.barFg);
    if (this.def.boss) {
      const name = new Text({ text: this.def.name, style: uiTextStyle({ size: 18, weight: "bold", fill: "#ffcf5a" }) });
      name.anchor.set(0.5, 1);
      name.y = -8;
      this.bar.addChild(name);
    }
    world.addChild(this.bar);
    this.setHealth(health);
  }

  setHealth(health) {
    if (health < this.health - 0.001) this.hurtAt = performance.now();
    this.health = health;
    const w = Math.max(0, BAR_W * health);
    this.barFg.clear();
    if (w > 0) this.barFg.roundRect(-BAR_W / 2, 0, w, BAR_H, 2).fill(this.def.boss ? 0xffa53d : 0xe74c3c);
    this.bar.visible = this.def.boss || health < 0.999;
  }

  /** New state from the server. */
  apply({ x, y, dir, health }) {
    this.x = x;
    this.y = y;
    this.dir = dir;
    if (health !== this.health) this.setHealth(health);
    this.lastUpdate = performance.now();
    this.sprite.visible = true;
  }

  update(delta, lerpAlpha) {
    const s = this.sprite;
    if (Math.hypot(this.x - s._rx, this.y - s._ry) > 500) {
      s._rx = this.x;
      s._ry = this.y;
    }
    // Face the way they're moving (sprites face "down").
    const targetRot = this.dir - Math.PI / 2;
    let diff = targetRot - s._rot;
    while (diff > Math.PI) diff -= Math.PI * 2;
    while (diff < -Math.PI) diff += Math.PI * 2;
    s._rot += diff * lerpAlpha;
    s.rotation = s._rot;

    s._rx += (this.x - s._rx) * lerpAlpha;
    s._ry += (this.y - s._ry) * lerpAlpha;
    s.x = s._rx;
    s.y = s._ry;
    // Flash red when hurt.
    const hurt = performance.now() - (this.hurtAt ?? -1e9) < 140;
    s.tint = hurt ? 0xff7a7a : (this.def.tint ?? 0xffffff);

    this.bar.x = s._rx;
    this.bar.y = s._ry + this.def.size * 0.42;
    this.bar.visible = s.visible && (this.def.boss || this.health < 0.999);
  }

  destroy() {
    this.sprite.destroy();
    this.bar.destroy({ children: true });
  }
}
