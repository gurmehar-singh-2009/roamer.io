import { Assets, Container, Graphics, Rectangle, Sprite, Text } from "pixi.js";
import { UI, titleStyle, uiTextStyle, easeOutBack } from "./theme";
import { WEAPONS, weaponCost } from "../data/weapons";
import { ITEMS, costText } from "../data/items";

const SLOT = 72;
const GAP = 10;
const PAD = 10;
const TITLE_H = 30;
const PANEL_H = PAD * 2 + SLOT;

export class UpgradeBar {
  constructor(game) {
    this.game = game;
    this.isActive = false;

    this.container = new Container();
    this.container.visible = false;
    game.renderer.ui.addChild(this.container);

    this.panelG = new Graphics();
    this.container.addChild(this.panelG);

    this.title = new Text({ text: "CHOOSE AN UPGRADE", style: titleStyle() });
    this.title.anchor.set(0.5, 0);
    this.title.y = PAD;
    this.container.addChild(this.title);

    this.slots = [];
    this._total = 0;

    // shared tooltip (on the UI layer so it's never clipped by the panel)
    this.tooltip = new Container();
    this.tooltip.visible = false;

    this.ttPanel = new Graphics();
    this.ttName = new Text({
      text: "",
      style: uiTextStyle({ size: 16, weight: "bold" }),
    });
    this.ttName.x = 14;
    this.ttName.y = 12;

    this.ttDesc = new Text({
      text: "",
      style: uiTextStyle({ size: 13, wrap: 226 }),
    });
    this.ttDesc.x = 14;
    this.ttDesc.y = 38;
    this.ttDesc.alpha = 0.85;

    this.tooltip.addChild(this.ttPanel, this.ttName, this.ttDesc);
    game.renderer.ui.addChild(this.tooltip);
  }

  toggleVisibility(active) {
    this.isActive = active;
    this.container.visible = active;
    if (!active) this.tooltip.visible = false;
  }

  /** Options from the server's upgrades packet (empty lists hide the bar). */
  setOptions({ age, weapons, items, pending }) {
    // The server resends the options whenever XP changes; only rebuild (and
    // replay the pop-in) when they actually changed.
    const key = JSON.stringify([age, weapons, items, pending]);
    if (key === this._key && this.container.visible) return;
    this._key = key;
    const options = [
      ...weapons.map((id) => {
        const w = WEAPONS[id];
        const cost = weaponCost(id);
        return {
          id,
          isItem: false,
          texture: w.texture,
          hoverDetails: [w.name, cost ? `${w.desc}\n${cost}` : w.desc],
        };
      }),
      ...items.map((id) => {
        const item = ITEMS[id];
        return {
          id,
          isItem: true,
          texture: item.texture,
          hoverDetails: [item.name, `${item.desc}\nCosts ${costText(item)}.`],
        };
      }),
    ];
    const more = pending > 0 ? ` (${pending} more)` : "";
    this.title.text = `AGE ${age}: CHOOSE AN UPGRADE${more}`;
    this.setWeapons(options);
  }

  setWeapons(weapons) {
    for (const s of this.slots) {
      // A slot removed under the mouse never gets its pointerout.
      if (s.hover) this.game.hud?.uiHoverOut();
      s.container.destroy({ children: true });
    }
    this.slots = [];

    weapons = (weapons || []).filter((w) => w);
    if (!weapons.length) {
      this.toggleVisibility(false);
      return;
    }

    this._total = weapons.length * SLOT + (weapons.length - 1) * GAP + PAD * 2;

    // Wide enough for the title too; the slots stay centered.
    const panelW = Math.max(this._total, this.title.width + PAD * 4);
    this.panelG.clear();
    this.panelG
      .roundRect(-panelW / 2, 0, panelW, TITLE_H + PANEL_H - PAD, UI.radius)
      .fill({ color: UI.panel, alpha: UI.alpha.panel });

    weapons.forEach((weapon, i) =>
      this.slots.push(this._createSlot(weapon, i)),
    );

    this.toggleVisibility(true);
  }

  _createSlot(weapon, i) {
    const container = new Container();
    container.x = -this._total / 2 + PAD + i * (SLOT + GAP) + SLOT / 2;
    container.y = TITLE_H + SLOT / 2;

    const bg = new Graphics();
    bg.roundRect(-SLOT / 2, -SLOT / 2, SLOT, SLOT, UI.radius)
      .fill({ color: UI.panel, alpha: UI.alpha.slot });
    container.addChild(bg);

    const icon = new Sprite();
    icon.anchor.set(0.5);
    icon.width = icon.height = SLOT - 18;
    container.addChild(icon);

    if (weapon.texture) {
      Assets.load(weapon.texture)
        .then((tex) => (icon.texture = tex))
        .catch(() => {});
    }

    container.eventMode = "static";
    container.hitArea = new Rectangle(-SLOT / 2, -SLOT / 2, SLOT, SLOT);
    container.cursor = "pointer";

    const slot = {
      container,
      icon,
      weapon,
      pop: 0,
      spin: Math.random() * Math.PI * 2,
      speed: 0.9 + i * 0.12,
    };

    container.on("pointerover", () => {
      slot.hover = true;
      this.game.hud?.uiHoverIn();
      this._showTooltip(slot);
    });
    container.on("pointerout", () => {
      if (!slot.hover) return;
      slot.hover = false;
      this.game.hud?.uiHoverOut();
      this.tooltip.visible = false;
    });
    container.on("pointerdown", () => {
      this.toggleVisibility(false);
      if (slot.hover) this.game.hud?.uiHoverOut();
      slot.hover = false;
      this.game.chooseUpgrade(weapon.isItem, weapon.id);
    });

    this.container.addChild(container);
    return slot;
  }

  _showTooltip(slot) {
    const [name, desc] = slot.weapon.hoverDetails || ["", ""];
    const x = this.container.x + slot.container.x;
    this.showTip(name, desc, x, this.container.y + TITLE_H + PANEL_H, false);
  }

  /** Shows the tooltip centered on x, below y (or above it). Also used by
   * the hotbar. */
  showTip(name, desc, x, y, above = false) {
    this.ttName.text = name;
    this.ttDesc.text = desc;

    const w = 254;
    const h = 60 + this.ttDesc.height;
    this.ttPanel.clear();
    this.ttPanel
      .roundRect(0, 0, w, h, UI.radius)
      .fill({ color: UI.panel, alpha: UI.alpha.tooltip });

    const screen = this.game.renderer.app.screen;
    this.tooltip.pivot.set(w / 2, 0);
    this.tooltip.x = Math.max(w / 2 + 8, Math.min(screen.width - w / 2 - 8, x));
    this.tooltip.y = above ? y - h : y;
    this.tooltip.visible = true;
  }

  hideTip() {
    this.tooltip.visible = false;
  }

  layout(W, H) {
    this.container.position.set(W / 2, UI.margin);
  }

  update(delta) {
    if (!this.container.visible) return;

    for (const s of this.slots) {
      s.spin += delta * s.speed;
      s.icon.rotation = s.spin;

      if (s.pop < 1) {
        s.pop = Math.min(1, s.pop + delta * 3);
        s.container.scale.set(Math.max(0.01, easeOutBack(s.pop)));
      }
    }
  }

  destroy() {
    this.container.destroy({ children: true });
    this.tooltip.destroy({ children: true });
  }
}
