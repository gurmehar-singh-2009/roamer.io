import { encode_into_bytes, HandshakeState, solve_pow } from "../parser/pkg/parser";
import { Render, PLAYER_TEXTURES } from "./render/renderer";
import Player from "./objects/player";
import Animal from "./objects/animal";
import GameObject from "./objects/object";
import { decodePacket, initDecoder } from "./packetHandler";
import { HUD } from "./ui/hud";
import { Dom } from "./ui/dom";
import { C, S } from "./protocol";
import { ITEMS } from "./data/items";
import { WEAPONS } from "./data/weapons";
import { loadTexturePack } from "./texturePack";
import { isEntrance } from "./render/caves";

/**
 * Where the server is. In development these come from the defaults (or
 * VITE_GAME_PORT / VITE_POW_PORT); a release build is told by the
 * /config.json file the client host serves (see release/roamer.sh):
 * { "gamePort", "powPort", "powPath"?, "host"?, "certHashes"? }. With
 * `powPath` the challenge comes from the page's own origin (the client host
 * passes it on to the game server).
 */
const SERVER = {
  host: "127.0.0.1",
  gamePort: Number(import.meta.env?.VITE_GAME_PORT ?? 6767),
  powPort: Number(import.meta.env?.VITE_POW_PORT ?? 3001),
  powPath: null,
  // SHA-256 hashes (base64) of a self-signed certificate to accept.
  certHashes: [],
};

/** Reads /config.json if the page was served with one. */
async function loadServerConfig() {
  try {
    const res = await fetch("/config.json", { cache: "no-store" });
    if (!res.ok) return;
    const c = await res.json();
    SERVER.host = c.host || location.hostname;
    SERVER.gamePort = c.gamePort ?? SERVER.gamePort;
    SERVER.powPort = c.powPort ?? SERVER.powPort;
    SERVER.powPath = c.powPath ?? null;
    SERVER.certHashes = c.certHashes ?? [];
  } catch {
    // No config: development defaults.
  }
}

/** A host name for a URL (IPv6 addresses go in brackets). */
const urlHost = (h) => (h.includes(":") && !h.startsWith("[") ? `[${h}]` : h);

/** Messages that must arrive go on the reliable stream; the rest are
 * datagrams. */
const DATAGRAM_OPCODES = new Set([C.MOVE, C.AIM]);

/** Reads [length: u32 BE][payload] frames from a stream. */
async function* readFrames(stream) {
  const reader = stream.getReader();
  let buf = new Uint8Array(0);
  const need = async (n) => {
    while (buf.length < n) {
      const { value, done } = await reader.read();
      if (done) return false;
      const merged = new Uint8Array(buf.length + value.length);
      merged.set(buf);
      merged.set(value, buf.length);
      buf = merged;
    }
    return true;
  };
  while (await need(4)) {
    const len = new DataView(buf.buffer, buf.byteOffset, 4).getUint32(0, false);
    if (!(await need(4 + len))) return;
    yield buf.slice(4, 4 + len);
    buf = buf.slice(4 + len);
  }
}

export class Game {
  constructor() {
    this.renderer = new Render(this);
    this.hud = null;
    this.dom = new Dom(this);

    this.players = new Map();
    this.animals = new Map();
    this.objects = new Map();
    this.me = null;
    this.myId = null;
    this.connected = false;

    this.loadout = null;
    this.resources = { wood: 0, stone: 0, food: 0, gold: 0, ammo: 0 };
    this.shop = null;
    this.clanState = null;
    this.clanNames = new Map();
    this.arenaState = null;
    this.account = null;
    this.lastCoins = 0;
    this.selectedItem = null;
    this.leaderIcons = { crown: null, skull: null, star: null };

    this.moveInput = { up: false, down: false, left: false, right: false };
    this.lastMoveDir = null;
    this.lastAimDir = 0;
    this._lastAimSend = 0;
    this.cameraMouseMode = false;
    this._mouseX = window.innerWidth / 2;
    this._mouseY = window.innerHeight / 2;
  }

  /** The local player (older UI code calls it this). */
  get my_player() {
    return this.me;
  }

  get inArena() {
    return !!(this.me && this.me.x > 16384);
  }

  async init() {
    this.dom.showLoading("Loading...");
    await loadTexturePack();
    await initDecoder();
    await this.renderer.init();
    this.hud = new HUD(this);

    window.addEventListener("keydown", this.handleKey);
    window.addEventListener("keyup", this.handleKey);
    window.addEventListener("mousemove", this.handleMouseMove);
    window.addEventListener("mousedown", this.handleMouse);
    window.addEventListener("mouseup", this.handleMouse);
    window.addEventListener("blur", () => this.releaseInput());

    try {
      this.dom.showLoading("Connecting...");
      await this.connect();
      this.dom.showMainMenu();
      this.renderer.app.ticker.add(() => {
        this.dom.update();
        this.frameChecks();
      });
    } catch (e) {
      console.error(e);
      this.dom.showLoading("Couldn't connect to the server. Is it running?");
    }
  }

  // ------------------------------------------------------------ network --

  async connect() {
    await loadServerConfig();
    const host = urlHost(SERVER.host);
    const challengeUrl = SERVER.powPath
      ? new URL(SERVER.powPath, location.href).href
      : `http://${host}:${SERVER.powPort}/challenge`;
    const challenge = await fetch(challengeUrl, { cache: "no-store" }).then((r) => r.text());
    const nonce = solve_pow(challenge, 4);
    // A self-signed certificate is accepted by its hash (it must be ECDSA and
    // valid for under two weeks; release/roamer.sh makes and renews it).
    const options = SERVER.certHashes.length
      ? {
          serverCertificateHashes: SERVER.certHashes.map((h) => ({
            algorithm: "sha-256",
            value: Uint8Array.from(atob(h), (ch) => ch.charCodeAt(0)),
          })),
        }
      : {};
    this.wt = new WebTransport(`https://${host}:${SERVER.gamePort}/?pow=${nonce}&challenge=${challenge}`, options);
    await this.wt.ready;
    this.wt.closed
      .catch(() => {})
      .finally(() => {
        this.connected = false;
        this.dom.showLoading("Disconnected. Refresh to play again.");
      });

    this.datagramWriter = this.wt.datagrams.writable.getWriter();
    this.datagramReader = this.wt.datagrams.readable.getReader();

    // Handshake on a bidirectional stream.
    const handshake = HandshakeState.create_client_hello();
    const hello = handshake.message();
    const framed = new Uint8Array(4 + hello.byteLength);
    new DataView(framed.buffer).setUint32(0, hello.byteLength, false);
    framed.set(hello, 4);
    const stream = await this.wt.createBidirectionalStream();
    const writer = stream.writable.getWriter();
    await writer.write(framed);
    const frames = readFrames(stream.readable);
    const { value: serverHello } = await frames.next();
    if (!serverHello) throw new Error("connection closed during handshake");
    this.crypto = handshake.complete_handshake(serverHello);
    writer.close().catch(() => {});

    // Our reliable stream to the server.
    const uni = await this.wt.createUnidirectionalStream();
    this.reliableWriter = uni.getWriter();
    this.connected = true;

    this.readDatagrams().catch((e) => console.error("datagram loop died:", e));
    this.readReliable().catch((e) => console.error("reliable loop died:", e));
  }

  /** Encodes, encrypts and sends a message. */
  async send(opcode, data) {
    if (!this.connected) return;
    const packet = this.crypto.encrypt(encode_into_bytes(data, opcode));
    if (DATAGRAM_OPCODES.has(opcode)) {
      await this.datagramWriter.write(packet).catch(() => {});
      return;
    }
    const framed = new Uint8Array(4 + packet.length);
    new DataView(framed.buffer).setUint32(0, packet.length, false);
    framed.set(packet, 4);
    await this.reliableWriter.write(framed).catch(() => {});
  }

  // Kept for the chat widget.
  sendEncrypted(data, opcode) {
    return this.send(opcode, data);
  }

  async readDatagrams() {
    while (true) {
      const { value, done } = await this.datagramReader.read();
      if (done) return;
      this.receive(value);
    }
  }

  async readReliable() {
    const streams = this.wt.incomingUnidirectionalStreams.getReader();
    while (true) {
      const { value: stream, done } = await streams.read();
      if (done) return;
      (async () => {
        for await (const frame of readFrames(stream)) this.receive(frame);
      })().catch((e) => console.error("reliable stream error:", e));
    }
  }

  receive(bytes) {
    if (!bytes || bytes.length < 9) return;
    try {
      this.handlePacket(decodePacket(this.crypto.decrypt(bytes)));
    } catch (e) {
      console.error("decode error:", e);
    }
  }

  // -------------------------------------------------------------- input --

  canAim() {
    return !this.hud?.chat?.isOpen;
  }

  releaseInput() {
    this.moveInput = { up: false, down: false, left: false, right: false };
    this.sendInput();
    if (this.attacking) {
      this.attacking = false;
      this.send(C.ATTACK, { attacking: false });
    }
  }

  handleKey = (e) => {
    const down = e.type === "keydown";
    const key = e.key.toLowerCase();

    if (this.hud?.chat?.isOpen) {
      if (down) this.hud.chat.key(e);
      return;
    }
    if (this.dom.deathVisible) {
      if (down && key === "enter") this.dom.playAgain();
      return;
    }
    // Typing in a DOM input (name, clan name...).
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
    if (!this.me) return;

    if (down && key === "escape") {
      if (this.dom.panelOpen) this.dom.closePanel();
      else this.selectItem(null);
      return;
    }
    if (key === "enter" && down) {
      e.preventDefault();
      this.hud?.chat.open();
      return;
    }

    switch (key) {
      case "w":
      case "arrowup":
        this.moveInput.up = down;
        break;
      case "s":
      case "arrowdown":
        this.moveInput.down = down;
        break;
      case "a":
      case "arrowleft":
        this.moveInput.left = down;
        break;
      case "d":
      case "arrowright":
        this.moveInput.right = down;
        break;
      default:
        if (down && !e.repeat) this.handleShortcut(key);
    }
    this.sendInput();
  };

  handleShortcut(key) {
    if (/^[1-9]$/.test(key)) return this.selectSlot(Number(key) - 1);
    const groupItem = (ids) => this.loadout?.items.find((i) => ids.includes(i));
    switch (key) {
      case "q":
        return this.eat();
      case "e":
        return this.useItem(groupItem([3, 4, 5]));
      case "f":
        return this.useItem(groupItem([6, 7, 8, 9]));
      case "b":
        return this.dom.togglePanel("shop");
      case "c":
        return this.dom.togglePanel("clan");
      case "x":
        return this.send(C.CAVE, {});
      case "v":
        return this.dom.togglePanel("arena");
      case "\\":
        this.cameraMouseMode = !this.cameraMouseMode;
        return this.hud?.toast(`camera: ${this.cameraMouseMode ? "mouse follow" : "centered"}`);
    }
  }

  /** Picks hotbar slot `i` (weapons first, then items). */
  selectSlot(i) {
    const entry = this.hud?.actionBar.entries()[i];
    if (!entry) return;
    if (entry.weapon != null) {
      this.selectItem(null);
      this.send(C.CHOOSE_WEAPON, { weapon: entry.weapon });
    } else {
      // Food and buildings go in your hand; click to eat or place.
      this.selectItem(this.selectedItem === entry.item ? null : entry.item);
    }
  }

  selectItem(item) {
    this.selectedItem = item ?? null;
    const texture = item == null ? null : this.renderer.textures[ITEMS[item]?.texture];
    this.me?.setHeldItem(texture || null, item == null ? null : ITEMS[item], this.renderer.overlayFilter);
  }

  /** Q: takes your food in hand (click to eat), or puts it away. */
  eat() {
    const food = this.loadout?.items.find((i) => ITEMS[i]?.food);
    if (food != null) this.selectItem(this.selectedItem === food ? null : food);
  }

  useItem(item) {
    if (item == null) return;
    this.send(C.USE_ITEM, { item });
  }

  chooseUpgrade(isItem, id) {
    this.send(C.CHOOSE_UPGRADE, { is_item: isItem, id });
  }

  handleMouse = (e) => {
    if (!this.me || this.hud?.chat?.isOpen) return;
    const down = e.type === "mousedown";
    if (down) {
      if (this.hud?.pointerOverUI || e.target !== this.renderer.app.canvas) return;
      if (this.selectedItem != null) {
        // Place the selected item. We go back to the weapon (like moomoo)
        // once the server says it was placed; if it couldn't be, the item
        // stays selected.
        this.useItem(this.selectedItem);
        this.pendingPlace = { item: this.selectedItem, at: performance.now() };
        return;
      }
      this.attacking = true;
      this.send(C.ATTACK, { attacking: true });
    } else if (this.attacking) {
      this.attacking = false;
      this.send(C.ATTACK, { attacking: false });
    }
  };

  handleMouseMove = (e) => {
    this._mouseX = e.clientX;
    this._mouseY = e.clientY;
    if (!this.canAim()) return;
    const aim = this._computeAim();
    if (aim === null || this.lastAimDir === aim) return;
    this.lastAimDir = aim;
    const now = performance.now();
    if (now - this._lastAimSend < 30) return;
    this._lastAimSend = now;
    this.send(C.AIM, { dir: aim });
  };

  /** Aim from the cursor relative to the player's on-screen position. */
  _computeAim = () => {
    const p = this.me;
    if (!p) return null;
    const r = this.renderer;
    const sx = r.app.screen.width / 2 + (p._rx - r.camera.x) * r.zoom;
    const sy = r.app.screen.height / 2 + (p._ry - r.camera.y) * r.zoom;
    return Math.atan2(this._mouseY - sy, this._mouseX - sx);
  };

  sendInput = () => {
    if (!this.me) return;
    let dir = null;
    const m = this.moveInput;
    if (m.up || m.down || m.left || m.right) {
      dir = Math.atan2((m.down ? 1 : 0) - (m.up ? 1 : 0), (m.right ? 1 : 0) - (m.left ? 1 : 0));
      if ((m.up && m.down && !m.left && !m.right) || (m.left && m.right && !m.up && !m.down)) dir = null;
    }
    if (this.lastMoveDir === dir) return;
    this.lastMoveDir = dir;
    this.sendMove(dir);
  };

  // Kept for the chat widget.
  sendMove(dir) {
    return this.send(C.MOVE, { dir });
  }

  // ----------------------------------------------------------- lifecycle --

  spawn(name) {
    this.send(C.SPAWN, { name, token: localStorage.getItem("roamer-token") || "" });
  }

  /** Our player died: show the death screen; the world stays loaded. */
  onDeath(data) {
    this.releaseInput();
    this.selectItem(null);
    if (this.me) {
      this.dom.deathAt = { x: this.me.x, y: this.me.y };
      this.removePlayer(this.myId);
    }
    this.me = null;
    this.hud.upgradeBar.setOptions({ age: 0, weapons: [], items: [], pending: 0 });
    this.dom.showDeath(data);
  }

  removePlayer(id) {
    const p = this.players.get(id);
    if (!p) return;
    // Players we can see fall apart; others just go.
    if (p._shown !== false && p.root?.renderable !== false && !p.root?.destroyed) {
      p.destroyLabels();
      p.centerRoot();
      this.renderer.fx.breakApart(p.root, p._rx, p._ry, 35, ["dust"]);
    } else {
      p.destroy();
    }
    this.players.delete(id);
    if (id === this.myId && this.me === p) this.me = null;
  }

  // ------------------------------------------------------------ packets --

  handlePacket({ code, data }) {
    switch (code) {
      case S.SPAWNED: {
        this.myId = data.id;
        localStorage.setItem("roamer-token", data.token);
        this.account = data;
        localStorage.setItem("roamer-account", JSON.stringify(data));
        this.lastCoins = data.coins_earned;
        this.dom.refresh("account");
        break;
      }
      case S.ADD_PLAYER: {
        this.removePlayer(data.data.id);
        const pd = data.data;
        const t = this.renderer.textures;
        const p = new Player(
          { id: pd.id, name: data.name, x: pd.x, y: pd.y, weapon: -1, aim: pd.aim, health: pd.health },
          PLAYER_TEXTURES.map((path) => t[path]),
          this.renderer.world,
          { game: this },
        );
        p.weapon = -1;
        p.variant = -1;
        p.hat = p.accessory = p.skin = -1;
        this.players.set(p.id, p);
        if (data.is_mine) {
          p.isLocal = true;
          this.me = p;
          this.myId = p.id;
          this.dom.deathAt = null;
          this.dom.showGame();
          this.hud.resources.show();
        }
        this.applyPlayer(p, pd);
        // Spawning (or joining) near us: pop in.
        if (data.is_mine || (this.me && Math.hypot(pd.x - this.me.x, pd.y - this.me.y) < 900)) {
          p.centerRoot();
          this.renderer.fx.popIn(p.root);
        }
        p.icons = this.iconsFor(p.id);
        p.refreshName();
        break;
      }
      case S.REMOVE_PLAYER:
        if (data.id !== this.myId) this.removePlayer(data.id);
        break;
      case S.UPDATE_PLAYERS:
        for (const pd of data.players) {
          const p = this.players.get(pd.id);
          if (p) this.applyPlayer(p, pd);
        }
        this.send(C.AIM, { dir: this.lastAimDir });
        break;
      case S.ANIMALS: {
        const now = performance.now();
        for (const ad of data.animals) {
          const existing = this.animals.get(ad.id);
          if (existing && existing.kind === ad.kind) {
            if (ad.health < existing.health - 0.001) {
              this.renderer.fx.hitCreature(existing.sprite._rx, existing.sprite._ry);
            }
            existing.apply(ad);
          }
          else {
            existing?.destroy();
            this.animals.set(
              ad.id,
              new Animal(ad, this.renderer.world, this.renderer.textures, this.renderer.animalLayer),
            );
          }
        }
        for (const [id, a] of this.animals) {
          const age = now - a.lastUpdate;
          if (age > 400) a.sprite.visible = false;
          if (age > 3000) {
            a.destroy();
            this.animals.delete(id);
          }
        }
        break;
      }
      case S.HIT_EVENT: {
        const p = this.players.get(data.entity_id);
        if (p) {
          p.attackAnim = 1;
          p.punchTime = performance.now();
          p.animateRightArm = !p.animateRightArm;
        }
        break;
      }
      case S.ADD_OBJECT:
        for (const od of data.objects) {
          if (this.objects.has(od.id)) this.renderer.removeObject(od.id);
          this.objects.set(od.id, new GameObject(od, this.myId));
          // Our building appeared: it was placed, so put the item away.
          const p = this.pendingPlace;
          if (p && od.owner === this.myId && od.type_obj === 50 + p.item && performance.now() - p.at < 1500) {
            this.pendingPlace = null;
            if (this.selectedItem === p.item) this.selectItem(null);
          }
        }
        break;
      case S.UPDATE_HEALTH: {
        const p = this.players.get(data.id);
        if (p && data.new_health < p.health - 0.5) this.renderer.fx.hitCreature(p._rx, p._ry);
        p?.setHealth(data.new_health, data.max_health);
        break;
      }
      case S.LOADOUT:
        this.loadout = data;
        if (this.selectedItem != null && !data.items.includes(this.selectedItem)) this.selectItem(null);
        this.hud.actionBar.setLoadout(data);
        this.updateAmmo();
        this.dom.refresh("arena");
        break;
      case S.OBJECT_HIT: {
        const o = this.objects.get(data.id);
        if (o) {
          o.onHit(data.dir);
          this.renderer.hitObject(o, data.dir);
        }
        break;
      }
      case S.RESOURCES: {
        // We ate the food in our hand: back to the weapon.
        const p = this.pendingPlace;
        if (p && ITEMS[p.item]?.food && data.food < (this.resources?.food ?? 0) && performance.now() - p.at < 1500) {
          this.pendingPlace = null;
          if (this.selectedItem === p.item) this.selectItem(null);
        }
        this.resources = data;
        this.me?.setResources(data.wood, data.stone, data.food);
        this.hud.resources.setValues(data.wood, data.stone, data.food, data.gold);
        this.dom.refreshChest();
        this.updateAmmo();
        this.dom.refresh("shop");
        break;
      }
      case S.DEATH:
        this.onDeath(data);
        break;
      case S.CHAT: {
        const p = this.players.get(data.id);
        p?.setChatMessage(data.message);
        this.hud.chat.push(`${p?.name || "player"}: ${data.message}`);
        break;
      }
      case S.UPGRADES:
        this.hud.upgradeBar.setOptions(data);
        break;
      case S.LEADERBOARD:
        this.hud.leaderboard.setData(data);
        this.leaderIcons = { crown: data.crown, skull: data.skull, star: data.star };
        for (const p of this.players.values()) {
          p.icons = this.iconsFor(p.id);
          p.refreshName();
        }
        break;
      case S.OBJECT_HEALTH:
        this.objects.get(data.id)?.setHealth(data.health, data.max_health);
        break;
      case S.REMOVE_OBJECT:
        this.renderer.removeObject(data.id);
        this.objects.delete(data.id);
        break;
      case S.ADD_PROJECTILE:
        this.renderer.addProjectile(data);
        break;
      case S.REMOVE_PROJECTILE:
        this.renderer.removeProjectile(data.id);
        break;
      case S.AGE:
        this.dom.setAge(data);
        break;
      case S.NOTICE:
        this.hud.toast(data.text, data.kind);
        break;
      case S.EXPLOSION:
        this.renderer.explosion(data.x, data.y, data.radius);
        break;
      case S.CLAN_STATE:
        this.clanState = data;
        this.clanNames = new Map(data.clans.map((c) => [c.id, c.name]));
        for (const p of this.players.values()) {
          p.clanTag = p.clan != null ? this.clanNames.get(p.clan) : null;
          p.refreshName();
        }
        this.dom.refresh("clan");
        break;
      case S.SHOP_STATE:
        this.shop = data;
        this.dom.refresh("shop");
        break;
      case S.QUEST:
        this.dom.setQuest(data);
        break;
      case S.WORLD_EVENT:
        this.dom.setEvent(data);
        break;
      case S.CHEST_STATE:
        this.dom.updateChest(data);
        break;
      case S.MINIMAP:
        this.dom.setMinimap(data.points);
        break;
      case S.CAVE_CELLS:
        this.renderer.caves.addCells(data);
        break;
      case S.ARENA_STATE:
        this.arenaState = data;
        this.dom.refresh("arena");
        break;
      default:
        console.warn("unknown packet", code);
    }
  }

  /** Applies a player's state from the server to their sprites. */
  applyPlayer(p, pd) {
    p.lastUpdate = performance.now();
    p.underground = pd.underground;
    p.setTargetPosition(pd.x, pd.y);
    p.aim = pd.aim;
    const t = this.renderer.textures;
    if (pd.weapon !== p.weapon || pd.variant !== p.variant) {
      p.weapon = pd.weapon;
      p.variant = pd.variant;
      p.setWeaponTexture(this.weaponTexture(pd.weapon, pd.variant), this.handTexture(pd.weapon, pd.variant));
      if (p.isLocal) this.updateAmmo();
    }
    if (pd.hat !== p.hat) p.setHat(pd.hat, t);
    if (pd.accessory !== p.accessory) p.setAccessory(pd.accessory, t);
    if (pd.skin !== p.skin) p.setSkin(pd.skin);
    if (pd.health !== p.health || pd.max_health !== p.maxHealth) {
      if (pd.health < p.health - 0.5 && p._shown !== false) this.renderer.fx.hitCreature(p._rx, p._ry);
      p.setHealth(pd.health, pd.max_health);
    }
    const clan = pd.clan ?? null;
    if (clan !== p.clan) {
      p.clan = clan;
      p.clanTag = clan != null ? this.clanNames.get(clan) : null;
      p.refreshName();
    }
  }

  /** Texture held in a player's hands for a weapon, or null for fists. */
  weaponTexture(id, variant = 0) {
    const w = WEAPONS[id];
    if (!w || id === 0) return null;
    const t = this.renderer.textures;
    if (w.isGun) return t[w.sprite] || null;
    return t[w.variants?.[variant]] || t[w.sprite] || null;
  }

  /** Ammo counter, shown while holding a gun. */
  updateAmmo() {
    const gun = !!this.me && !!WEAPONS[this.me.weapon]?.isGun;
    const ammo = this.resources?.ammo ?? 0;
    this.me?.setAmmo(ammo, gun);
    this.hud?.actionBar.setAmmo(ammo);
  }

  /** The single dagger drawn in each hand for dual weapons. */
  handTexture(id, variant = 0) {
    const w = WEAPONS[id];
    if (!w?.handSprites) return null;
    const t = this.renderer.textures;
    return t[w.handSprites[variant]] || t[w.handSprites[0]] || null;
  }

  /** Every frame: hide players we stopped hearing about (out of view, or in
   * the caves while we're on the surface), and hint at cave entrances. */
  frameChecks() {
    const now = performance.now();
    for (const p of this.players.values()) {
      p.setVisible(p.isLocal || now - (p.lastUpdate ?? 0) < 400);
    }
    const me = this.me;
    if (!me) return;
    let near = false;
    for (const o of this.objects.values()) {
      if (isEntrance(o) && Math.hypot(o.x - me.x, o.y - me.y) < o.scale + 18) {
        near = true;
        break;
      }
    }
    if (near && !this._nearEntrance) {
      this.hud?.toast(me.underground ? "Press X to climb up" : "Press X to go into the caves", 2);
    }
    this._nearEntrance = near;
  }

  iconsFor(id) {
    const i = this.leaderIcons;
    return { crown: i.crown === id, skull: i.skull === id, star: i.star === id };
  }
}

const game = new Game();
window.game = game;
await game.init();
