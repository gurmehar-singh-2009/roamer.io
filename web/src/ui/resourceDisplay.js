import { Assets, Container, Graphics, Sprite, Text } from "pixi.js";
import { UI, uiTextStyle } from "./theme";
import WoodIcon from "../assets/resources/wood.png";
import StoneIcon from "../assets/resources/stone.png";
import FoodIcon from "../assets/resources/food.png";
import GoldIcon from "../assets/resources/gold.png";

// moomoo.io resource icons.
const RESOURCES = [
  { key: "wood", icon: WoodIcon },
  { key: "stone", icon: StoneIcon },
  { key: "berry", icon: FoodIcon },
  { key: "gold", icon: GoldIcon },
];

/** Row size and spacing. */
export const ROW_W = 132;
const ROW_H = 44;
const GAP = 6;
const ICON = 32;

/** Total height of the panel stack. */
export const RESOURCES_H = RESOURCES.length * ROW_H + (RESOURCES.length - 1) * GAP;

/**
 * Resource counters (wood / stone / food / gold), bottom-right: one panel
 * row each, icon on the left and count on the right. The icon and count
 * bump whenever the value changes.
 */
export class ResourceDisplay {
  constructor(game) {
    this.game = game;
    this.container = new Container();
    this.container.visible = false;
    game.renderer.ui.addChild(this.container);

    this.values = { wood: 0, stone: 0, berry: 0, gold: 0 };
    this.items = RESOURCES.map((res, i) => {
      const row = new Container();
      row.y = i * (ROW_H + GAP);
      const bg = new Graphics().roundRect(0, 0, ROW_W, ROW_H, UI.radius).fill({ color: UI.panel, alpha: UI.alpha.panel });
      const icon = new Sprite();
      icon.anchor.set(0.5);
      icon.position.set(UI.pad + ICON / 2, ROW_H / 2);
      const count = new Text({ text: "0", style: uiTextStyle({ size: 20, weight: "bold" }) });
      count.anchor.set(1, 0.5);
      count.position.set(ROW_W - UI.pad, ROW_H / 2);
      row.addChild(bg, icon, count);
      this.container.addChild(row);
      Assets.load(res.icon)
        .then((tex) => {
          icon.texture = tex;
          icon.width = icon.height = ICON;
          item.iconScale = icon.scale.x;
        })
        .catch(() => {});
      const item = { key: res.key, icon, iconScale: 1, count, value: 0, pop: 0 };
      return item;
    });
  }

  show() {
    this.container.visible = true;
  }

  hide() {
    this.container.visible = false;
  }

  set(key, value) {
    if (value == null) return;
    this.values[key] = value;
    const item = this.items.find((i) => i.key === key);
    if (!item || item.value === value) return;
    item.pop = value > item.value ? 1 : 0.5; // bigger bump when gaining
    item.value = value;
    item.count.text = formatCount(value);
  }

  setValues(wood, stone, food, gold) {
    this.set("wood", wood);
    this.set("stone", stone);
    this.set("berry", food);
    this.set("gold", gold);
  }

  update(delta) {
    if (!this.container.visible) return;
    for (const item of this.items) {
      if (item.pop <= 0) continue;
      item.pop = Math.max(0, item.pop - delta * 2.4);
      const bump = Math.sin((1 - item.pop) * Math.PI); // 0 -> 1 -> 0
      item.icon.scale.set(item.iconScale * (1 + 0.2 * bump));
      item.count.scale.set(1 + 0.25 * bump);
    }
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}

function formatCount(n) {
  n = Math.floor(n);
  if (n >= 1e6) return (n / 1e6).toFixed(1).replace(/\.0$/, "") + "m";
  if (n >= 1e4) return (n / 1e3).toFixed(1).replace(/\.0$/, "") + "k";
  return String(n);
}
