import { Container, Graphics, Sprite, Text, TextStyle } from "pixi.js";
import { WEAPONS } from "../data/weapons";

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
    this.aim = aim;
    this.lastAim = aim;
    this.visualAim = aim;
    this.attackAnim = 0;
    this.visualAim = this.aim ?? 0;
    this.animateRightArm = false;
    this.lastX = x;
    this.lastY = y;
    this.lerpX = x;
    this.lerpY = y;
    this.weapon = weapon;
    this.chatMessage = "";
    this.timeSinceChat = 0;
    this.health = health;
    this.weaponRotationExtra = 0;

    // this.setHealth(this.health, 100);

    this.sprite;
    this.arm1Sprite;
    this.arm2Sprite;
    this.healthBarContainer;
    this.healthBarBackground;
    this.healthBar;
    this.nameLabel;
    this.chatBox;
    this.weaponSprite;
    this.weapon;

    this.wood = 0;
    this.stone = 0;
    this.food = 0;

    this.punchTime = 0;
    this.punchDuration = 250;

    this.sprite;
    this.arm1Sprite;
    this.arm2Sprite;
    this.backpackSprite;
    this.healthBarContainer;
    this.healthBarBackground;
    this.healthBar;
    this.nameLabel;
    this.chatBox;
    this.weaponSprite;
    this.weapon;

    this._worldRef = world;
    this._utilsRef = utils;

    this.initSprites([
      playerTexture,
      arm1Texture,
      arm2Texture,
      backpackTexture,
    ]);

    world.addChild(this.sprite.backpackSprite);
    world.addChild(this.sprite.arm1Sprite);
    world.addChild(this.sprite.arm2Sprite);
    // world.addChild(this.sprite.weaponSprite);
    world.addChild(this.sprite);

    this.initHealthBar(world);
    this.initNameLabel(
      this.name,
      new TextStyle({
        fontSize: 20,
        fontFamily: "GameFont",
        fontWeight: "normal",
        fill: "#fff",
        stroke: {
          color: "#454545",
          width: 6.7,
          join: "round",
        },
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
        stroke: {
          color: "#454545",
          width: 6.7,
          join: "round",
        },
        align: "center",
        letterSpacing: 0.67,
      }),
      world,
    );

    this.sprite.zIndex = 1; // head, weapon, arms, backpack
    this.healthBarContainer.zIndex = 10; // above player
    this.sprite.nameLabel.zIndex = 10; // above player
    this.sprite.chatBox.zIndex = 20; // above everything
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
    const label = new Text({
      text: name,
      style: nameTextStyle,
    });
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

    container.addChild(shadow);
    container.addChild(playerSprite);

    container.playerSprite = playerSprite;
    container.shadow = shadow;
    container.arm1Sprite = arm1;
    container.arm2Sprite = arm2;
    container.backpackSprite = backpack;

    container._rx = this.x;
    container._ry = this.y;

    this.sprite = container;
  }

  setWeaponTexture(texture) {
    if (!texture) {
      if (this.sprite.weaponSprite) {
        this.sprite.weaponSprite.visible = false;
      }
      return;
    }

    const weaponData = WEAPONS.find((w) => w.id === this.weapon?.[0]) || {};

    if (!this.sprite.weaponSprite) {
      const ws = new Sprite(texture);
      ws.anchor.set(0.5, 0.85);
      this.sprite.weaponSprite = ws;
    } else {
      this.sprite.weaponSprite.texture = texture;
    }

    this.sprite.weaponSprite.width = weaponData.width || 55;
    this.sprite.weaponSprite.height = weaponData.length || 88;

    this._worldRef.addChildAt(this.sprite.weaponSprite, 2);

    this.sprite.weaponSprite.visible = true;
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

  update(delta, rotation, lerpAlpha) {
    this.sprite.rotation = rotation;

    // interpolate
    this.sprite._rx += (this.x - this.sprite._rx) * lerpAlpha;
    this.sprite._ry += (this.y - this.sprite._ry) * lerpAlpha;
    this.sprite.x = this.sprite._rx;
    this.sprite.y = this.sprite._ry;

    if (this.sprite.nameLabel) {
      this.sprite.nameLabel.x = this.sprite._rx;
      this.sprite.nameLabel.y = this.sprite._ry - 40;
    }

    const chatAlive =
      this.chatMessage && performance.now() - this.timeSinceChat < 3e3;
    if (this.sprite.chatBox) {
      this.sprite.chatBox.x = this.sprite._rx;
      this.sprite.chatBox.y = this.sprite._ry - 90;
      this.sprite.chatBox.visible = chatAlive;
      if (!chatAlive && this.chatMessage) {
        this.chatMessage = "";
        this.sprite.chatBoxLabel.text = "";
      }
    }

    this.healthBarContainer.x = this.sprite._rx;
    this.healthBarContainer.y = this.sprite._ry + 45;

    this.attackAnim -= delta * 1.75;
    this.attackAnim = Math.max(0, Math.min(this.attackAnim, 1));

    let aimDelta = this.aim - this.visualAim;
    if (aimDelta > Math.PI) aimDelta -= Math.PI * 2;
    if (aimDelta < -Math.PI) aimDelta += Math.PI * 2;
    this.visualAim += aimDelta * lerpAlpha;

    if (this.sprite.shadow) {
      this.sprite.shadow.x = Math.cos(this.visualAim - rotation) * 6.7;
      this.sprite.shadow.y = Math.sin(this.visualAim - rotation) * 6.7;
    }

    if (this?.weapon?.length && this?.weapon[0] === 1) {
      this.updateHammer(rotation);
    } else {
      if (this.sprite.weaponSprite) {
        this.sprite.weaponSprite.visible = false;
      }
      this.updateArms(rotation);
    }
  }

  updateHammer(rotation, restDistance = 25, restAngle = Math.PI / 3) {
    const t = this.attackAnim;

    let swing;
    if (t < 0.6) {
      const p = t / 0.6;
      swing = p * p * p;
    } else {
      const p = (t - 0.6) / 0.4;

      swing = 1 - p * p * (3 - 2 * p);
    }
    const aim = rotation - (Math.PI / 2) * swing;

    if (this.sprite?.arm1Sprite) {
      const dist = restDistance;
      const currentAngleOffset = -restAngle;

      this.sprite.arm1Sprite.x =
        this.sprite._rx + dist * Math.cos(aim + currentAngleOffset);
      this.sprite.arm1Sprite.y =
        this.sprite._ry + dist * Math.sin(aim + currentAngleOffset);
      this.sprite.arm1Sprite.rotation = aim;
    }

    if (this.sprite?.arm2Sprite) {
      const dist = restDistance;
      const currentAngleOffset = restAngle;

      this.sprite.arm2Sprite.x =
        this.sprite._rx + dist * Math.cos(aim + currentAngleOffset);
      this.sprite.arm2Sprite.y =
        this.sprite._ry + dist * Math.sin(aim + currentAngleOffset);
      this.sprite.arm2Sprite.rotation = aim;
    }

    if (this.sprite?.backpackSprite) {
      this.sprite.backpackSprite.x =
        this.sprite._rx + Math.cos(this.visualAim - Math.PI) * 10;
      this.sprite.backpackSprite.y =
        this.sprite._ry + Math.sin(this.visualAim - Math.PI) * 10;
      this.sprite.backpackSprite.rotation = this.visualAim;
    }

    if (this.sprite.weaponSprite) {
      let w = WEAPONS.find((x) => x.id == this.weapon[0]) || null;

      if (w) {
        const weaponDistance = restDistance + 18;

        const xOff = w.xOff || 0;
        const yOff = w.yOff || 0;

        this.sprite.weaponSprite.x =
          this.sprite._rx +
          weaponDistance * Math.cos(aim) +
          xOff * Math.cos(aim) -
          yOff * Math.sin(aim);

        this.sprite.weaponSprite.y =
          this.sprite._ry +
          weaponDistance * Math.sin(aim) +
          xOff * Math.sin(aim) +
          yOff * Math.cos(aim);

        this.sprite.weaponSprite.rotation = aim;
        this.sprite.weaponSprite.visible = true;
      }
    }
  }

  updateArms(
    rotation,
    restDistance = 25,
    punchDistance = 35,
    restAngle = Math.PI / 3,
  ) {
    const aim = rotation;
    const t = this.attackAnim;
    const s = t * t * (3 - 2 * t);

    const elapsed = performance.now() - this.punchTime;

    let extension = 0;

    if (elapsed < this.punchDuration) {
      const t = elapsed / this.punchDuration;

      // 0 -> 1 -> 0
      extension = Math.sin(t * Math.PI);
    }

    // const extension = 4 * s * (1 - s);
    if (this.sprite?.arm1Sprite) {
      const isPunching = this.animateRightArm;
      const currentExt = isPunching ? extension : 0;
      const dist = restDistance + (punchDistance - restDistance) * currentExt;
      const currentAngleOffset = -restAngle * (1 - currentExt);

      this.sprite.arm1Sprite.x =
        this.sprite._rx + dist * Math.cos(aim + currentAngleOffset);
      this.sprite.arm1Sprite.y =
        this.sprite._ry + dist * Math.sin(aim + currentAngleOffset);
      this.sprite.arm1Sprite.rotation = aim + (isPunching ? extension : 0);
    }

    if (this.sprite?.arm2Sprite) {
      const isPunching = !this.animateRightArm;
      const currentExt = isPunching ? extension : 0;
      const dist = restDistance + (punchDistance - restDistance) * currentExt;
      const currentAngleOffset = restAngle * (1 - currentExt);

      this.sprite.arm2Sprite.x =
        this.sprite._rx + dist * Math.cos(aim + currentAngleOffset);
      this.sprite.arm2Sprite.y =
        this.sprite._ry + dist * Math.sin(aim + currentAngleOffset);
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
    this._drawBar(Math.max(0, current / max));
  }

  setResources(wood, stone, food) {
    this.wood = wood;
    this.stone = stone;
    this.food = food;
    // In the future, update a UI element here. For now, log for debugging.
    // console.log(`resources for ${this.name}: wood=${wood} stone=${stone} food=${food}`);
  }

  destroy() {
    try {
      if (this.sprite?.arm1Sprite) this.sprite.arm1Sprite.destroy();
      if (this.sprite?.arm2Sprite) this.sprite.arm2Sprite.destroy();
      if (this.sprite?.weaponSprite) this.sprite.weaponSprite.destroy();
      if (this.sprite?.nameLabel) this.sprite.nameLabel.destroy();
      if (this.sprite?.chatBox) this.sprite.chatBox.destroy();
      if (this.healthBarContainer) this.healthBarContainer.destroy();
      if (this.sprite) this.sprite.destroy();
    } catch (e) {
      console.warn("failed to destroy player sprites", e);
    }
  }
}
