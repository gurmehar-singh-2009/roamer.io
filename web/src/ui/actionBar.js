import { Assets, Container, Graphics, Rectangle, Sprite, Text } from "pixi.js";
import { UI, easeOutBack, uiTextStyle } from "./theme";
import { WEAPONS, VARIANTS, weaponCost } from "../data/weapons";
import { ITEMS, costText } from "../data/items";
import Arm1 from "../assets/arm_1.png";
import Arm2 from "../assets/arm_2.png";

const SLOT = 64;
const GAP = 8;
const PAD = 8;
const PANEL_H = PAD * 2 + SLOT;

const weaponIdOf = (w) => (Array.isArray(w) ? w[0] : w);

export class ActionBar {
  constructor(game) {
    this.game = game;
    this.container = new Container();
    this.container.visible = false;
    game.renderer.ui.addChild(this.container);

    this.panelG = new Graphics();
    this.container.addChild(this.panelG);

    this.slots = [];
    this._entries = [];
    this._key = "";
    this._total = 0;
  }

  /** The hotbar's contents, in order: `{ weapon }` or `{ item }`. */
  entries() {
    return this._entries;
  }

  /** Builds the hotbar from a loadout packet: weapons first, then items. */
  setLoadout(loadout) {
    const variantOf = (id) => loadout.progress.find((p) => p.weapon === id)?.variant ?? 0;
    const entries = [{ weapon: loadout.primary, variant: variantOf(loadout.primary) }];
    if (loadout.secondary != null) {
      entries.push({ weapon: loadout.secondary, variant: variantOf(loadout.secondary) });
    }
    for (const item of loadout.items) entries.push({ item });
    // Only rebuild (and replay the pop-in) when something changed.
    const key = JSON.stringify(entries);
    if (key === this._key) return;
    this._key = key;
    this._entries = entries;
    this.setWeapons(entries.map((e) => slotInfo(e)));
  }

  setWeapons(weapons) {
    for (const s of this.slots) {
      // A slot removed under the mouse never gets its pointerout.
      if (s.hover) this.game.hud?.uiHoverOut();
      s.container.destroy({ children: true });
    }
    this.slots = [];

    if (!weapons || weapons.length === 0) {
      this.container.visible = false;
      return;
    }

    this._total = weapons.length * SLOT + (weapons.length - 1) * GAP + PAD * 2;

    this.panelG.clear();
    this.panelG
      .roundRect(-this._total / 2, 0, this._total, PANEL_H, UI.radius)
      .fill({ color: UI.panel, alpha: UI.alpha.panel });

    weapons.forEach((weapon, i) => {
      if (weapon) this.slots.push(this._createSlot(weapon, i));
    });

    this.container.visible = true;
  }

  _createSlot(weapon, i) {
    const container = new Container();
    container.x = -this._total / 2 + PAD + i * (SLOT + GAP) + SLOT / 2;
    container.y = PAD + SLOT / 2;

    const bg = new Graphics();
    container.addChild(bg);

    const iconHolder = new Container();
    container.addChild(iconHolder);

    // Hotkey number, top-left.
    if (i < 9) {
      const keyLabel = new Text({ text: String(i + 1), style: uiTextStyle({ size: 12, weight: "bold" }) });
      keyLabel.alpha = 0.7;
      keyLabel.position.set(-SLOT / 2 + 5, -SLOT / 2 + 2);
      container.addChild(keyLabel);
    }

    const paths = weapon.textures || (weapon.texture ? [weapon.texture] : []);
    const isHands = weapon.id === 0;

    paths.forEach((path, j) => {
      Assets.load(path)
        .then((tex) => {
          const s = new Sprite(tex);
          s.anchor.set(0.5);
          if (isHands) {
            // replicate the old canvas layout: 50px images, 23.125px steps
            const k = SLOT / 67;
            s.width = s.height = 50 * k;
            s.x = (35 - 33.5) * k;
            s.y = (23.125 * j + 25 - 33.5) * k;
          } else {
            // Fit inside the slot, keeping the image's shape.
            const k = (SLOT - 12) / Math.max(tex.width, tex.height);
            s.scale.set(k);
            if (weapon.tint) s.tint = weapon.tint;
          }
          iconHolder.addChild(s);
        })
        .catch(() => {});
    });

    // Ammo left, on gun slots.
    let badge = null;
    if (weapon.gun) {
      badge = new Text({ text: String(this.ammo ?? 0), style: uiTextStyle({ size: 14, weight: "bold" }) });
      badge.anchor.set(1, 1);
      badge.position.set(SLOT / 2 - 4, SLOT / 2 - 2);
      container.addChild(badge);
    }

    container.eventMode = "static";
    container.hitArea = new Rectangle(-SLOT / 2, -SLOT / 2, SLOT, SLOT);
    container.cursor = "pointer";

    const slot = {
      badge,
      container,
      bg,
      weapon,
      hover: false,
      hoverT: 0,
      pop: 0,
      selected: null,
      hoveredDrawn: null,
    };

    container.on("pointerover", () => {
      slot.hover = true;
      this.game.hud?.uiHoverIn();
      const [name, desc] = weapon.hoverDetails;
      const x = this.container.x + container.x;
      this.game.hud?.upgradeBar.showTip(name, desc, x, this.container.y - 10, true);
    });
    container.on("pointerout", () => {
      slot.hover = false;
      this.game.hud?.uiHoverOut();
      this.game.hud?.upgradeBar.hideTip();
    });
    container.on("pointerdown", () => {
      this.game.selectSlot(i);
    });

    this._drawSlotBg(slot);
    this.container.addChild(container);
    return slot;
  }

  /** Updates the ammo count on gun slots. */
  setAmmo(ammo) {
    this.ammo = ammo;
    for (const s of this.slots) if (s.badge) s.badge.text = String(ammo);
  }

  _isSelected(slot) {
    const held = this.game.selectedItem;
    if (slot.weapon.item != null) return held === slot.weapon.item;
    return held == null && weaponIdOf(this.game.my_player?.weapon) === slot.weapon.id;
  }

  _drawSlotBg(slot) {
    const selected = this._isSelected(slot);
    slot.selected = selected;
    slot.hoveredDrawn = slot.hover;

    slot.bg.clear();
    slot.bg
      .roundRect(-SLOT / 2, -SLOT / 2, SLOT, SLOT, UI.radius)
      // No outlines: the selected slot is lighter, hover a little darker.
      .fill(
        selected
          ? { color: 0xffffff, alpha: 0.28 }
          : { color: UI.panel, alpha: slot.hover ? UI.alpha.active : UI.alpha.slot },
      );
  }

  layout(W, H) {
    this.container.position.set(W / 2, H - PANEL_H - UI.margin);
  }

  update(delta) {
    if (!this.container.visible) return;

    for (const s of this.slots) {
      // pop-in animation
      if (s.pop < 1) {
        s.pop = Math.min(1, s.pop + delta * 3.2);
        s.container.scale.set(Math.max(0.01, easeOutBack(s.pop)));
      }

      // hover lift
      s.hoverT += ((s.hover ? 1 : 0) - s.hoverT) * Math.min(1, delta * 14);
      s.container.y = PAD + SLOT / 2 - 5 * s.hoverT;
      s.container.alpha = 0.85 + 0.15 * s.hoverT;

      // selection highlight
      const sel = this._isSelected(s);
      if (sel !== s.selected || s.hover !== s.hoveredDrawn) {
        this._drawSlotBg(s);
      }
    }
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}

/** Icon, tooltip and id for a hotbar entry. */
function slotInfo(entry) {
  if (entry.item != null) {
    const item = ITEMS[entry.item];
    return {
      id: null,
      item: entry.item,
      texture: item.texture,
      hoverDetails: [item.name, `${item.desc}\nCosts ${costText(item)}.`],
    };
  }
  const w = WEAPONS[entry.weapon];
  if (entry.weapon === 0) {
    return { id: 0, textures: [Arm1, Arm2], hoverDetails: [w.name, w.desc] };
  }
  const variant = VARIANTS[entry.variant] || VARIANTS[0];
  const name = variant.name ? `${variant.name} ${w.name}` : w.name;
  const cost = weaponCost(w.id);
  return {
    id: w.id,
    texture: w.variants?.[entry.variant] || w.texture,
    // Gun art has no variants, so tint it.
    tint: w.isGun && entry.variant > 0 ? variant.color : 0,
    gun: !!w.isGun,
    hoverDetails: [name, cost ? `${w.desc}\n${cost}` : w.desc],
  };
}
