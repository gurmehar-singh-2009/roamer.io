import { Container, Graphics, Sprite, Text, TextStyle } from "pixi.js";
import { WEAPONS, VARIANTS } from "../data/weapons";
import { SKINS, hatById, accessoryById } from "../data/shop";
import { Z } from "../render/layers";

const weaponIdOf = (w) => (Array.isArray(w) ? w[0] : w);

// --- network smoothing -------------------------------------------------------
// Simple frame-rate-independent lerp toward the latest server position.
// 8 = floaty, 12 = balanced, 20 = snappy.
const SMOOTH = 12;
// Distance treated as a teleport (respawn) → snap instead of sliding.
// Keep in sync with the camera's snap threshold.
const SNAP_DIST = 500;
// Remote aim follow rate.
const AIM_SMOOTH = 14;
// Player radius (half the head sprite).
const PLAYER_RADIUS = 35;
// moomoo.io weapon art is drawn at this scale, so weapons match the size of
// the original hammer next to our head sprite.
const WEAPON_SCALE = 0.7;
// Gap the server leaves between the player and a placed building.
const PLACE_GAP = 5;
// Draw order of a player's parts, back to front. Like moomoo.io, hands go
// over the weapon and the body over both.
const PART = { BACKPACK: 0, WEAPON: 1, ARMS: 2, HEAD: 3, HELD: 4 };
// How far the fist is in front of the glove sprite's center (the skin-colored
// part of arm_1.png / arm_2.png, at 50 px wide).
const GLOVE_TIP = 16.8;
// moomoo's weapon frame is moved this far forward so the grip lands at the
// edge of our (smaller) head.
const FRAME_SHIFT = 14;
// Where a gun's stock starts, in front of the player's center.
const GUN_BASE = 16;
// How far a swing turns, in radians.
const SWING_ARC = Math.PI / 2;
// Ammo shown as a full bar.
const AMMO_BAR_FULL = 60;

export default class Player {
  constructor(
    { id, name, x, y, weapon, aim, health },
    [playerTexture, arm1Texture, arm2Texture, backpackTexture],
    world,
    utils,
  ) {
    this.id = id;
    this.name = name;
    this.x = x;
    this.y = y;
    this.aim = aim ?? 0;
    this.visualAim = this.aim;
    this.isLocal = false;
    this.score = 0;

    this.attackAnim = 0;
    this.animateRightArm = false;
    this.weapon = weapon;
    this.chatMessage = "";
    this.timeSinceChat = 0;
    this.health = health ?? 100;
    this.maxHealth = 100;

    this.wood = 0;
    this.stone = 0;
    this.food = 0;

    this.punchTime = 0;
    this.punchDuration = 250;

    this._worldRef = world;
    this._utilsRef = utils;

    // --- network motion state --------------------------------------------
    this._tx = x; // target position from the server
    this._ty = y;
    this._rx = x; // rendered (smoothed) position — everything renders from this
    this._ry = y;

    // Footprints
    this._footprints = utils?.game?.renderer?.footprints ?? null;
    this._lastStepX = x;
    this._lastStepY = y;
    this._stepLength = 42;

    this.initSprites([
      playerTexture,
      arm1Texture,
      arm2Texture,
      backpackTexture,
    ]);

    // Everything that makes up the body lives in one container, so players
    // never get drawn in between each other's parts.
    this.root = new Container();
    this.root.sortableChildren = true;
    this.root.zIndex = Z.PLAYERS;
    world.addChild(this.root);
    this._bodyRef = this.root;

    this.root.addChild(this.sprite.backpackSprite, this.sprite.arm1Sprite, this.sprite.arm2Sprite, this.sprite);
    this.sprite.zIndex = PART.HEAD;
    this.sprite.backpackSprite.zIndex = PART.BACKPACK;
    this.sprite.arm1Sprite.zIndex = PART.ARMS;
    this.sprite.arm2Sprite.zIndex = PART.ARMS;

    this.initHealthBar(world);
    this.initNameLabel(
      this.name,
      new TextStyle({
        fontSize: 20,
        fontFamily: "GameFont",
        fontWeight: "normal",
        fill: "#fff",
        stroke: { color: "#454545", width: 6.7, join: "round" },
        align: "center",
        letterSpacing: 0.67,
      }),
      world,
    );
    this.initChatBox(
      new TextStyle({
        fontSize: 20,
        fontFamily: "GameFont",
        fontWeight: "normal",
        fill: "#fff",
        stroke: { color: "#454545", width: 6.7, join: "round" },
        align: "center",
        letterSpacing: 0.67,
      }),
      world,
    );

    this.healthBarContainer.zIndex = Z.HEALTHBAR;
    this.sprite.nameLabel.zIndex = Z.NAME;
    this.sprite.chatBox.zIndex = Z.CHAT;

    // Extras: weapon variant, hat, accessory, skin, clan tag, icons.
    this.variant = 0;
    this.hat = 0;
    this.accessory = 0;
    this.skin = 0;
    this.clan = null;
    this.clanTag = null;
    this.icons = { crown: false, skull: false, star: false };
  }

  /** New authoritative position from the server. Sets the target only. */
  setTargetPosition(x, y) {
    // Teleport / respawn → snap instead of sliding across the map.
    if (Math.hypot(x - this._rx, y - this._ry) > SNAP_DIST) {
      this._rx = x;
      this._ry = y;
    }

    this._tx = x;
    this._ty = y;
    this.x = x;
    this.y = y;
  }

  /**
   * Per-frame: lerp the rendered position toward the target.
   * Frame-rate independent, can't overshoot.
   */
  _updatePosition(delta) {
    const t = delta * Math.E;

    this._rx += (this._tx - this._rx) * t;
    this._ry += (this._ty - this._ry) * t;

    this.sprite.x = this._rx;
    this.sprite.y = this._ry;
  }
  initChatBox(chatTextStyle, world) {
    const chatBoxHolder = new Container();

    const chatBoxBg = new Graphics();
    chatBoxHolder.addChild(chatBoxBg);

    const chatBoxLabel = new Text({ text: "", style: chatTextStyle });
    chatBoxLabel.anchor.set(0.5, 0.5);
    chatBoxHolder.addChild(chatBoxLabel);

    this.sprite.chatBoxBg = chatBoxBg;
    this.sprite.chatBoxLabel = chatBoxLabel;
    this.sprite.chatBox = chatBoxHolder;

    world.addChild(chatBoxHolder);
  }

  initNameLabel(name, nameTextStyle, world) {
    const label = new Text({ text: name, style: nameTextStyle });
    label.anchor.set(0.5, 1);
    this.sprite.nameLabel = label;
    world.addChild(label);
  }

  initSprites([playerTexture, arm1Texture, arm2Texture, backpackTexture]) {
    const container = new Container();

    const shadow = new Graphics();
    shadow.circle(0, 0, 45).fill(0x211e1e);
    shadow.alpha = 0.05;

    const playerSprite = new Sprite(playerTexture);
    playerSprite.anchor.set(0.5);
    playerSprite.width = playerSprite.height = 70;

    const arm1 = new Sprite(arm1Texture);
    const arm2 = new Sprite(arm2Texture);
    const backpack = new Sprite(backpackTexture);

    arm1.anchor.set(0.5);
    arm1.width = 50;
    arm1.height = 60;

    arm2.anchor.set(0.5);
    arm2.width = 50;
    arm2.height = 60;

    backpack.anchor.set(0.5);
    backpack.width = 60;
    backpack.height = 60;

    // Hat on top of the head, accessory behind it (both from the shop).
    const hat = new Sprite();
    hat.anchor.set(0.5);
    hat.visible = false;
    const accessory = new Sprite();
    accessory.anchor.set(0.5);
    accessory.visible = false;

    container.addChild(accessory);
    container.addChild(shadow);
    container.addChild(playerSprite);
    container.addChild(hat);

    container.playerSprite = playerSprite;
    container.shadow = shadow;
    container.hatSprite = hat;
    container.accessorySprite = accessory;
    container.arm1Sprite = arm1;
    container.arm2Sprite = arm2;
    container.backpackSprite = backpack;

    container.x = this._rx;
    container.y = this._ry;

    this.sprite = container;
  }

  setWeaponTexture(texture, handTexture = null) {
    if (this.sprite.weaponSprite2) this.sprite.weaponSprite2.visible = false;
    this._dual = false;
    if (!texture) {
      if (this.sprite.weaponSprite) this.sprite.weaponSprite.visible = false;
      return;
    }

    const weaponData =
      WEAPONS.find((w) => w.id === weaponIdOf(this.weapon)) || {};

    if (!this.sprite.weaponSprite) {
      const ws = new Sprite(texture);
      ws.anchor.set(0.5, 0.85);
      this.sprite.weaponSprite = ws;
    } else {
      this.sprite.weaponSprite.texture = texture;
    }

    const ws = this.sprite.weaponSprite;
    if (weaponData.dual && handTexture) {
      // One dagger in each hand (see updateDaggers).
      this._dual = true;
      ws.texture = handTexture;
      ws.anchor.set(0.2, 0.5);
      ws.width = 56;
      ws.height = 15;
      if (!this.sprite.weaponSprite2) {
        this.sprite.weaponSprite2 = new Sprite(handTexture);
        this._bodyRef.addChild(this.sprite.weaponSprite2);
      }
      const ws2 = this.sprite.weaponSprite2;
      ws2.texture = handTexture;
      ws2.anchor.set(0.2, 0.5);
      ws2.width = 56;
      ws2.height = 15;
      ws2.zIndex = PART.WEAPON;
      ws2.visible = true;
    } else if (weaponData.isGun) {
      // Gun images have the stock at the top; hold them from there.
      ws.anchor.set(0.5, 0);
      ws.height = weaponData.length;
      ws.width = (weaponData.length * texture.width) / texture.height;
    } else {
      // moomoo.io art: `length` along the aim, `width` across it, placed
      // in updateHammer.
      ws.anchor.set(0, 0);
      ws.width = weaponData.length * WEAPON_SCALE;
      ws.height = weaponData.width * WEAPON_SCALE;
    }
    // Gold, diamond, ruby and emerald guns are tinted (their art has no
    // variants).
    ws.tint =
      weaponData.isGun && this.variant > 0
        ? VARIANTS[this.variant].color
        : 0xffffff;
    ws.zIndex = PART.WEAPON;

    this._bodyRef.addChild(ws);
    ws.visible = true;
  }

  /** Shows the selected building in hand instead of the weapon, at the size
   * and spot it will be placed. */
  setHeldItem(texture, item = null, overlay = null) {
    this.holdingItem = !!texture;
    // Buildings show at the size and spot they'll be placed; food is held
    // close, like moomoo.
    this.heldItemDist = item?.radius ? PLAYER_RADIUS + item.radius + PLACE_GAP : 52;
    if (!texture) {
      if (this.heldItemSprite) this.heldItemSprite.visible = false;
      return;
    }
    if (!this.heldItemSprite) {
      const s = new Sprite(texture);
      s.anchor.set(0.5);
      s.zIndex = PART.HELD;
      // Same dark overlay as placed buildings.
      if (overlay) s.filters = [overlay];
      this._bodyRef.addChild(s);
      this.heldItemSprite = s;
    }
    this.heldItemSprite.texture = texture;
    this.heldItemSprite.width = this.heldItemSprite.height = item?.radius ? item.radius * item.size : 46;
    // Only buildings get the building overlay.
    this.heldItemSprite.filters = item?.radius && overlay ? [overlay] : [];
    this.heldItemSprite.visible = true;
  }

  /** Shows or hides everything drawn for this player. */
  setVisible(on) {
    if (this._shown === on) return;
    this._shown = on;
    const s = this.sprite;
    // `renderable`, not `visible`: the per-frame code sets `visible` itself.
    for (const part of [this.root, s.nameLabel, s.chatBox, this.healthBarContainer]) {
      if (part) part.renderable = on;
    }
  }

  /** Hat (moomoo art), on top of the head. */
  setHat(id, textures) {
    this.hat = id;
    const def = hatById(id);
    const s = this.sprite.hatSprite;
    s.visible = !!def;
    if (!def) return;
    s.texture = textures[def.texture];
    s.width = s.height = 120;
    s.rotation = Math.PI / 2;
  }

  /** Accessory (moomoo art), behind the head. */
  setAccessory(id, textures) {
    this.accessory = id;
    const def = accessoryById(id);
    const s = this.sprite.accessorySprite;
    s.visible = !!def;
    if (!def) return;
    s.texture = textures[def.texture];
    s.width = s.height = 110;
    s.rotation = Math.PI / 2;
    s.x = -20;
  }

  /** Skin color: tints the head. */
  setSkin(id) {
    this.skin = id;
    this.skinTint = id ? SKINS[id] : 0xffffff;
    this.sprite.playerSprite.tint = this.skinTint;
  }

  /** Scales the body around the player (for the spawn and death animations). */
  centerRoot() {
    this.root.pivot.set(this._rx, this._ry);
    this.root.position.set(this._rx, this._ry);
  }

  /** Removes the name, health bar and chat right away; the body is left for
   * the death animation (which destroys it). */
  destroyLabels() {
    this.sprite?.nameLabel?.destroy();
    this.sprite?.chatBox?.destroy({ children: true });
    this.healthBarContainer?.destroy({ children: true });
  }

  /** Name label: crown / skull / star icons, clan tag and name. */
  refreshName() {
    const i = this.icons;
    const prefix = `${i.crown ? "♛ " : ""}${i.skull ? "☠ " : ""}${i.star ? "★ " : ""}`;
    const tag = this.clanTag ? `[${this.clanTag}] ` : "";
    this.sprite.nameLabel.text = prefix + tag + this.name;
  }

  setChatMessage(text) {
    this.chatMessage = text;
    this.timeSinceChat = performance.now();
    if (this.sprite.chatBoxLabel) {
      this.sprite.chatBoxLabel.text = text;

      const pad = { x: 10, y: 4 };
      const w = this.sprite.chatBoxLabel.width + pad.x * 2;
      const h = this.sprite.chatBoxLabel.height + pad.y * 2;

      this.sprite.chatBoxBg
        .clear()
        .roundRect(-w / 2, -h / 2, w, h, 6)
        .fill({ color: 0x000000, alpha: 0.2 });
    }
  }

  update(delta, rotation) {
    this.sprite.rotation = rotation;
    const hurt = performance.now() - (this.hurtAt ?? -1e9) < 140;
    this.sprite.playerSprite.tint = hurt ? 0xff7a7a : (this.skinTint ?? 0xffffff);

    // Smooth networked position.
    this._updatePosition(delta);

    // Aim: the local player snaps instantly to the live mouse aim
    // (no lerp) so the shadow + backpack react immediately.
    if (this.isLocal) {
      this.visualAim = rotation;
    } else {
      let aimDelta = this.aim - this.visualAim;
      if (aimDelta > Math.PI) aimDelta -= Math.PI * 2;
      else if (aimDelta < -Math.PI) aimDelta += Math.PI * 2;
      this.visualAim += aimDelta * (1 - Math.exp(-AIM_SMOOTH * delta));
      if (this.visualAim > Math.PI) this.visualAim -= Math.PI * 2;
      else if (this.visualAim < -Math.PI) this.visualAim += Math.PI * 2;
    }

    if (this.sprite.nameLabel) {
      this.sprite.nameLabel.x = this._rx;
      this.sprite.nameLabel.y = this._ry - 40;
    }

    const chatAlive =
      this.chatMessage && performance.now() - this.timeSinceChat < 3e3;
    if (this.sprite.chatBox) {
      this.sprite.chatBox.x = this._rx;
      this.sprite.chatBox.y = this._ry - 90;
      this.sprite.chatBox.visible = chatAlive;
      if (!chatAlive && this.chatMessage) {
        this.chatMessage = "";
        this.sprite.chatBoxLabel.text = "";
      }
    }

    this.healthBarContainer.x = this._rx;
    this.healthBarContainer.y = this._ry + 45;

    // A swing lasts a little less than the weapon's reload, so the next
    // swing always starts from rest.
    const held = WEAPONS[weaponIdOf(this.weapon)];
    const swingMs = Math.min(550, Math.max(200, (held?.reload ?? 300) * 0.9));
    this.attackAnim -= (delta * 1000) / swingMs;
    this.attackAnim = Math.max(0, Math.min(this.attackAnim, 1));

    this.updateFootprints();

    if (this.sprite.shadow) {
      this.sprite.shadow.x = Math.cos(this.visualAim - rotation) * 6.7;
      this.sprite.shadow.y = Math.sin(this.visualAim - rotation) * 6.7;
    }

    // Backpack is always equipped and always follows the aim
    // (instant for the local player).
    this.updateBackpack();

    const weaponData = WEAPONS.find((w) => w.id === weaponIdOf(this.weapon));
    if (this.holdingItem) {
      // Holding an item to place: arms at rest, item in front.
      if (this.sprite.weaponSprite) this.sprite.weaponSprite.visible = false;
      if (this.sprite.weaponSprite2) this.sprite.weaponSprite2.visible = false;
      this.updateArms(rotation);
      this.heldItemSprite.x = this._rx + Math.cos(rotation) * this.heldItemDist;
      this.heldItemSprite.y = this._ry + Math.sin(rotation) * this.heldItemDist;
      this.heldItemSprite.rotation = rotation;
    } else if (this._dual) {
      this.updateDaggers(rotation);
    } else if (weaponData?.isGun && this.sprite.weaponSprite) {
      this.updateGun(rotation, weaponData);
    } else if (weaponData && weaponData.id !== 0 && this.sprite.weaponSprite) {
      // Every melee weapon swings like the hammer; bows don't swing.
      this.updateHammer(rotation, weaponData);
    } else {
      if (this.sprite.weaponSprite) this.sprite.weaponSprite.visible = false;
      this.updateArms(rotation);
    }
  }

  /** Daggers: one in each hand. Each attack slashes with one hand, sweeping
   * from its side across the front and back, alternating hands. */
  updateDaggers(rotation, restDistance = 25, restAngle = Math.PI / 3) {
    const SLASH_MS = 180;
    const elapsed = performance.now() - this.punchTime;
    const sweep = elapsed < SLASH_MS ? Math.sin((elapsed / SLASH_MS) * Math.PI) : 0;
    const hands = [
      [this.sprite.arm1Sprite, this.sprite.weaponSprite, -1, this.animateRightArm],
      [this.sprite.arm2Sprite, this.sprite.weaponSprite2, 1, !this.animateRightArm],
    ];
    for (const [arm, dagger, side, active] of hands) {
      const s = active ? sweep : 0;
      // From the hand's own side (s = 0) to past the middle (s = 1).
      const offset = side * restAngle * (1 - 1.7 * s);
      const dist = restDistance + 14 * s;
      const angle = rotation + offset;
      arm.x = this._rx + dist * Math.cos(angle);
      arm.y = this._ry + dist * Math.sin(angle);
      arm.rotation = rotation;
      // The dagger sits in the hand and points forward; during a slash the
      // blade sweeps across the front with the hand.
      dagger.x = arm.x + 8 * Math.cos(rotation);
      dagger.y = arm.y + 8 * Math.sin(rotation);
      dagger.rotation = rotation + offset * 0.2 - side * 0.9 * s;
      dagger.visible = true;
    }
  }

  /** Ammo bar under the health bar (shown while holding a gun). */
  setAmmo(ammo, show) {
    this.ammoBar.visible = show;
    this.ammoText.visible = show;
    if (!show) return;
    const W = this._hbW;
    const ratio = Math.min(1, ammo / AMMO_BAR_FULL);
    this.ammoBar
      .clear()
      .roundRect(-W / 2 - 3, -2, W + 6, 8, 3)
      .fill({ color: 0x000000, alpha: 0.35 });
    if (ratio > 0) this.ammoBar.roundRect(-W / 2, 0, W * ratio, 4, 2).fill(ammo > 10 ? 0xf2d16b : 0xe74c3c);
    this.ammoText.text = String(ammo);
  }

  updateBackpack() {
    const bp = this.sprite.backpackSprite;
    if (!bp) return;

    bp.x = this._rx + Math.cos(this.visualAim - Math.PI) * 10;
    bp.y = this._ry + Math.sin(this.visualAim - Math.PI) * 10;
    bp.rotation = this.visualAim;
    bp.visible = true;
  }

  updateFootprints() {
    if (!this._footprints) return;

    const dx = this._rx - this._lastStepX;
    const dy = this._ry - this._lastStepY;

    if (dx * dx + dy * dy < this._stepLength * this._stepLength) return;

    this._footprints.spawn(this._rx, this._ry, Math.atan2(dy, dx));
    this._lastStepX = this._rx;
    this._lastStepY = this._ry;
  }

  /**
   * Swung weapons (and bows): drawn the way moomoo.io does it, scaled to our
   * body. The weapon art is placed in moomoo's frame (radius 35, +x = aim),
   * shrunk by WEAPON_SCALE and moved forward by FRAME_SHIFT so the grip sits
   * at the edge of our head; the hands go on the weapon's grip points. A
   * swing turns hands and weapon together towards the front.
   */
  updateHammer(rotation, w) {
    let aim = rotation;
    let kick = 0;
    if (w.ranged) {
      // Bows and the musket kick back a little instead of swinging.
      const elapsed = performance.now() - this.punchTime;
      kick = elapsed < 150 ? 6 * (1 - elapsed / 150) : 0;
    } else {
      const t = this.attackAnim;
      let swing;
      if (t > 0.6) {
        // Fast swing out...
        const p = (1 - t) / 0.4;
        swing = 1 - (1 - p) * (1 - p);
      } else {
        // ...and an eased return.
        const p = t / 0.6;
        swing = p * p * (3 - 2 * p);
      }
      aim = rotation - SWING_ARC * swing;
    }

    const cos = Math.cos(aim);
    const sin = Math.sin(aim);
    // moomoo frame -> world.
    const toWorld = (mx, my) => {
      const fx = FRAME_SHIFT - kick + WEAPON_SCALE * mx;
      const fy = WEAPON_SCALE * my;
      return [this._rx + fx * cos - fy * sin, this._ry + fx * sin + fy * cos];
    };

    // Hands: the fist (front of the glove) on each grip point. arm1 takes the
    // left grip, arm2 the right.
    const [left, right] = [...w.grip].sort((a, b) => a[1] - b[1]);
    for (const [arm, [gx, gy]] of [
      [this.sprite?.arm1Sprite, left],
      [this.sprite?.arm2Sprite, right],
    ]) {
      if (!arm) continue;
      const [hx, hy] = toWorld(gx, gy);
      arm.x = hx - GLOVE_TIP * cos;
      arm.y = hy - GLOVE_TIP * sin;
      arm.rotation = aim;
    }

    if (this.sprite.weaponSprite) {
      // moomoo puts the image's top-left corner at
      // (radius + xOff - length / 2, yOff - width / 2).
      const [x, y] = toWorld(PLAYER_RADIUS + (w.xOff || 0) - w.length / 2, (w.yOff || 0) - w.width / 2);
      const ws = this.sprite.weaponSprite;
      ws.x = x;
      ws.y = y;
      ws.rotation = aim;
      ws.visible = true;
    }
  }

  /** How far in front of the player shots appear: the end of the gun
   * barrel, or the middle of a bow (null for other weapons). */
  muzzleDistance() {
    const w = WEAPONS[weaponIdOf(this.weapon)];
    if (!w) return null;
    if (w.isGun) return (w.length < 70 ? GUN_BASE + 8 : GUN_BASE) + w.length - 4;
    if (w.ranged) {
      // The musket's barrel end, or where the arrow sits on a bow.
      const tip = w.id === 16 ? PLAYER_RADIUS + w.xOff + w.length / 2 - 10 : PLAYER_RADIUS + (w.xOff || 0);
      return FRAME_SHIFT + WEAPON_SCALE * tip;
    }
    return null;
  }

  /** Holds a gun pointing at the aim, with a short kick back when firing.
   * The shooting hand is on the grip; long guns get a support hand under
   * the barrel, pistols are held with both hands on the grip. */
  updateGun(rotation, weaponData) {
    const aim = rotation;
    const cos = Math.cos(aim);
    const sin = Math.sin(aim);

    const RECOIL_MS = 120;
    const elapsed = performance.now() - this.punchTime;
    const kick = elapsed < RECOIL_MS ? 8 * (1 - elapsed / RECOIL_MS) : 0;

    // `forward` is along the aim, `side` is to the right of it.
    const place = (s, forward, side) => {
      s.x = this._rx + cos * forward - sin * side;
      s.y = this._ry + sin * forward + cos * side;
      s.rotation = aim;
    };

    const len = weaponData.length;
    // Pistols are held further out, so the hands on the grip show.
    const base = (len < 70 ? GUN_BASE + 8 : GUN_BASE) - kick;
    const gun = this.sprite.weaponSprite;
    place(gun, base, 0);
    gun.rotation = aim - Math.PI / 2;
    gun.visible = true;

    // Fist positions (the glove sprites sit GLOVE_TIP behind them).
    const grip = base + len * 0.25;
    const support = len < 70 ? grip + 3 : base + len * 0.62;
    if (this.sprite?.arm2Sprite) place(this.sprite.arm2Sprite, grip - GLOVE_TIP, 7);
    if (this.sprite?.arm1Sprite) place(this.sprite.arm1Sprite, support - GLOVE_TIP, len < 70 ? -7 : -5);
  }

  updateArms(
    rotation,
    restDistance = 25,
    punchDistance = 35,
    restAngle = Math.PI / 3,
  ) {
    const aim = rotation;

    const elapsed = performance.now() - this.punchTime;
    let extension = 0;
    if (elapsed < this.punchDuration) {
      const t = elapsed / this.punchDuration;
      extension = Math.sin(t * Math.PI); // 0 -> 1 -> 0
    }

    if (this.sprite?.arm1Sprite) {
      const isPunching = this.animateRightArm;
      const currentExt = isPunching ? extension : 0;
      const dist = restDistance + (punchDistance - restDistance) * currentExt;
      const currentAngleOffset = -restAngle * (1 - currentExt);

      this.sprite.arm1Sprite.x =
        this._rx + dist * Math.cos(aim + currentAngleOffset);
      this.sprite.arm1Sprite.y =
        this._ry + dist * Math.sin(aim + currentAngleOffset);
      this.sprite.arm1Sprite.rotation = aim + (isPunching ? extension : 0);
    }

    if (this.sprite?.arm2Sprite) {
      const isPunching = !this.animateRightArm;
      const currentExt = isPunching ? extension : 0;
      const dist = restDistance + (punchDistance - restDistance) * currentExt;
      const currentAngleOffset = restAngle * (1 - currentExt);

      this.sprite.arm2Sprite.x =
        this._rx + dist * Math.cos(aim + currentAngleOffset);
      this.sprite.arm2Sprite.y =
        this._ry + dist * Math.sin(aim + currentAngleOffset);
      this.sprite.arm2Sprite.rotation = aim - (isPunching ? extension : 0);
    }
  }

  initHealthBar(world) {
    const W = 60,
      H = 6,
      R = 2;

    this.healthBarBg = new Graphics()
      .roundRect(-W / 2 - 3, 0 - 3, W + 6, H + 6, R + 2)
      .fill(0x333333);

    this.healthBarFg = new Graphics();
    this._hbW = W;
    this._hbH = H;
    this._hbR = R;
    this._drawBar(this.health / 100);

    this.healthBarContainer = new Container();
    this.healthBarContainer.addChild(this.healthBarBg);
    this.healthBarContainer.addChild(this.healthBarFg);

    // Ammo (local player with a gun only).
    this.ammoBar = new Graphics();
    this.ammoBar.y = H + 8;
    this.ammoText = new Text({
      text: "",
      style: new TextStyle({
        fontSize: 13,
        fontFamily: "GameFont",
        fill: "#fff",
        stroke: { color: "#454545", width: 4, join: "round" },
      }),
    });
    this.ammoText.anchor.set(0, 0.5);
    this.ammoText.position.set(W / 2 + 8, H + 10);
    this.ammoBar.visible = this.ammoText.visible = false;
    this.healthBarContainer.addChild(this.ammoBar, this.ammoText);

    world.addChild(this.healthBarContainer);
  }

  _drawBar(ratio) {
    const { _hbW: W, _hbH: H, _hbR: R } = this;
    const color = ratio > 0.5 ? 0x8ecc51 : ratio > 0.25 ? 0xf0a30a : 0xe74c3c;
    const w = Math.max(0, W * ratio);
    this.healthBarFg.clear();
    if (w > 0) {
      this.healthBarFg
        .roundRect(-W / 2, 0, w, H, Math.min(R, w / 2))
        .fill(color);
    }
  }

  setHealth(current, max) {
    // Flash red when hurt.
    if (current < this.health - 0.5) this.hurtAt = performance.now();
    this.health = current;
    this.maxHealth = max;
    this._drawBar(Math.max(0, current / max));
  }

  setResources(wood, stone, food) {
    this.wood = wood;
    this.stone = stone;
    this.food = food;
  }

  destroy() {
    try {
      if (this.sprite?.arm1Sprite) this.sprite.arm1Sprite.destroy();
      if (this.sprite?.arm2Sprite) this.sprite.arm2Sprite.destroy();
      if (this.sprite?.backpackSprite) this.sprite.backpackSprite.destroy();
      if (this.sprite?.weaponSprite) this.sprite.weaponSprite.destroy();
      if (this.heldItemSprite) this.heldItemSprite.destroy();
      if (this.sprite?.nameLabel) this.sprite.nameLabel.destroy();
      if (this.sprite?.chatBox) this.sprite.chatBox.destroy({ children: true });
      if (this.healthBarContainer)
        this.healthBarContainer.destroy({ children: true });
      if (this.sprite?.weaponSprite2) this.sprite.weaponSprite2.destroy();
      if (this.sprite) this.sprite.destroy({ children: true });
      this.root?.destroy({ children: true });
    } catch (e) {
      console.warn("failed to destroy player sprites", e);
    }
  }
}
