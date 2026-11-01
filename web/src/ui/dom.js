// The HTML side of the game, built on the original page (index.html and
// mainstyle.css): main menu cards, death screen, minimap, age bar,
// quest/event box, and the in-game panels (shop, clan, arena, settings).

import { C } from "../protocol";
import { HATS, ACCESSORIES, SKINS, SKIN_PRICE } from "../data/shop";
import { WEAPONS } from "../data/weapons";
import { saveTexturePack, clearTexturePack, texturePackSize } from "../texturePack";
import WoodIcon from "../assets/resources/wood.png";
import StoneIcon from "../assets/resources/stone.png";
import FoodIcon from "../assets/resources/food.png";
import GoldIcon from "../assets/resources/gold.png";

/** Resources a chest holds, with their icons. */
const CHEST_RESOURCES = [
  ["wood", WoodIcon],
  ["stone", StoneIcon],
  ["food", FoodIcon],
  ["gold", GoldIcon],
];
/** "Move everything" (the server moves as much as there is). */
const ALL = 0xffffffff;

const $ = (id) => document.getElementById(id);

/** Escapes text for HTML. */
const esc = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

/** "1m 05s" */
function formatTime(seconds) {
  const m = Math.floor(seconds / 60);
  const s = String(seconds % 60).padStart(2, "0");
  return m ? `${m}m ${s}s` : `${seconds}s`;
}

const hex = (color) => `#${color.toString(16).padStart(6, "0")}`;

/** Map size (must match server/src/config/config.toml). */
const MAP_SIZE = 16384;

/** Info screens, by the numbers the menu buttons use. */
const SCREENS = ["customize", "servers", "account", "help", "settings"];

/** Panel icon (Material Icons) and title. */
const PANEL_TITLES = {
  shop: ["storefront", "Shop"],
  skins: ["palette", "Skins"],
  clan: ["groups", "Clan"],
  arena: ["sports_kabaddi", "Arena"],
  customize: ["palette", "Customize"],
  servers: ["dns", "Servers"],
  account: ["person", "Account"],
  settings: ["settings", "Settings"],
  help: ["help", "How to Play"],
};

/** A Material Icons glyph. */
const iconHtml = (name) => (name ? `<i class="material-icons">${name}</i>` : "");
/** The gold coin used for prices. */
const coinHtml = () => `<img class="coin" src="${GoldIcon}" alt="gold">`;

const CONTROLS = [
  ["W A S D", "Move"],
  ["Mouse", "Aim"],
  ["Click", "Attack, or place the selected item"],
  ["1 - 9", "Pick from the hotbar"],
  ["Q", "Eat food"],
  ["E / F", "Quick wall / spikes"],
  ["X", "Go down / up a cave entrance or Tunnel"],
  ["B", "Shop"],
  ["C", "Clan"],
  ["V", "Arena"],
  ["Enter", "Chat"],
  ["\\", "Camera lean"],
  ["Esc", "Close panels"],
];

/** Shows/hides an element that the stylesheet hides or shows by default. */
const show = (el, on, display = "block") => el.style.setProperty("display", on ? display : "none", "important");

export class Dom {
  constructor(game) {
    this.game = game;
    this.panel = null;
    this.menuScreen = "help";
    this.shopTab = "hats";
    this.deathShownAt = 0;
    this.sound = localStorage.getItem("roamer-sound") !== "off";
    this.quest = null;
    this.event = null;
    this.minimapPoints = [];
    this.deathAt = null;
    this.age = null;

    try {
      this.savedAccount = JSON.parse(localStorage.getItem("roamer-account") || "null");
    } catch {
      this.savedAccount = null;
    }

    // Globals the original page calls from inline onclick handlers.
    window.showInfoScreen = (n) => this.showInfoScreen(n);
    window.showLeaderboard = () => this.toggleLeaderboard();
    window.toggleSound = () => this.toggleSound();
    window.showPanel = (name) => this.togglePanel(name);
    window.touchPrompt = (e, text) => {
      e.preventDefault();
      const name = prompt(text, $("nameInput").value);
      if (name != null) $("nameInput").value = name;
    };

    $("goButton").addEventListener("click", () => this.play());
    $("nameInput").addEventListener("keydown", (e) => {
      if (e.key === "Enter") this.play();
    });
    $("nameInput").value = localStorage.getItem("roamer-name") || "";
    $("playAgainButton").addEventListener("click", () => this.playAgain());
    $("exitButton").addEventListener("click", () => this.exitToLobby());

    this.minimap = $("minimap");
    this.minimapCtx = this.minimap.getContext("2d");
    this.xpBar = $("staminaHolder");
    this.xpCtx = this.xpBar.getContext("2d");

    this.renderSoundIcon();
    this.renderMenuCard();
    this.renderAccountCard();
  }

  // ------------------------------------------------------------ screens --

  showLoading(text) {
    show($("loadingContainer"), true);
    $("loadingText").textContent = text;
    show($("mainMenuContainer"), false);
  }

  showMainMenu() {
    show($("loadingContainer"), false);
    show($("spectateContainer"), false);
    show($("mainMenuContainer"), true);
    show($("darkener"), true);
    show($("uiContainer"), false);
    $("settingsContainer").classList.remove("inGame");
    $("nameInput").focus();
    this.renderAccountCard();
    this.renderMenuCard();
  }

  /** Called once our player is in the world. */
  showGame() {
    show($("mainMenuContainer"), false);
    show($("darkener"), false);
    show($("spectateContainer"), false);
    show($("uiContainer"), true);
    $("settingsContainer").classList.add("inGame");
  }

  play() {
    if (!this.game.connected || this.game.my_player) return;
    const name = $("nameInput").value.trim().slice(0, 16);
    localStorage.setItem("roamer-name", name);
    this.game.spawn(name);
  }

  /** diep.io-style death screen. */
  showDeath(d) {
    this.closePanel();
    show($("chestCard"), false);
    show($("uiContainer"), false);
    $("settingsContainer").classList.remove("inGame");
    const coins = this.game.lastCoins ? `+${this.game.lastCoins}` : "0";
    const stat = (k, v) => `<div class="deathStat"><span>${k}:</span> ${esc(v)}</div>`;
    $("deathMessage").innerHTML = `
      <div class="deathTitle">You were killed by:</div>
      <div class="deathKiller">${esc(d.killer || "the wilderness")}</div>
      ${stat("Score", d.score)}
      ${stat("Age", d.age)}
      ${stat("Kills", d.kills)}
      ${stat("Time alive", formatTime(d.time_alive))}
      ${stat("Coins earned", coins)}
      <div class="deathContinue">(press enter to play again)</div>`;
    show($("spectateContainer"), true);
    this.deathShownAt = performance.now();
  }

  get deathVisible() {
    return $("spectateContainer").style.display === "block";
  }

  playAgain() {
    // Ignore an Enter that was held at the moment of death.
    if (performance.now() - this.deathShownAt < 600) return;
    show($("spectateContainer"), false);
    this.play();
  }

  exitToLobby() {
    this.showMainMenu();
  }

  // ---------------------------------------------------------- top icons --

  toggleLeaderboard() {
    const lb = this.game.hud?.leaderboard;
    if (!lb) return;
    lb.hidden = !lb.hidden;
    lb.markDirty();
  }

  toggleSound() {
    this.sound = !this.sound;
    localStorage.setItem("roamer-sound", this.sound ? "on" : "off");
    this.renderSoundIcon();
  }

  renderSoundIcon() {
    $("soundSetting").innerHTML = `<i class="material-icons" style="color:#fff;font-size:45px">${
      this.sound ? "volume_up" : "volume_off"
    }</i>`;
  }

  // ------------------------------------------------------ menu cards --

  /** Account stats on the left menu card. */
  renderAccountCard() {
    const a = this.game.account || this.savedAccount;
    const coins = this.game.shop?.coins;
    const row = (k, v) => `<div class="infoText">${k}: <span style="color:#353535">${esc(v)}</span></div>`;
    $("adCard").innerHTML = `
      <div class="infoHeader">Your Account</div>
      ${
        a
          ? row("Best score", a.best_score) +
            row("Total kills", a.total_kills) +
            row("Games played", a.games) +
            (coins != null ? row("Coins", coins) : "")
          : `<div class="infoText">Play a game to start your account. Your stats and skins are kept on this browser.</div>`
      }
      <div class="infoText" style="margin-top:10px">Score is your gold. Every age gives you an upgrade, and guns arrive at age 5.</div>`;
  }

  /** The right menu card: whichever info screen was picked. */
  renderMenuCard() {
    const card = $("infoCard");
    card.innerHTML = `<div class="infoHeader">${PANEL_TITLES[this.menuScreen][1]}</div><div id="infoBody"></div>`;
    this.renderInto($("infoBody"), this.menuScreen);
  }

  /** The menu buttons (0-3) and the settings icon (4). */
  showInfoScreen(n) {
    const name = SCREENS[n] || "help";
    if (this.game.my_player) {
      this.togglePanel(name);
      return;
    }
    this.menuScreen = name;
    this.renderMenuCard();
  }

  // ------------------------------------------------------------- panels --

  get panelOpen() {
    return this.panel !== null;
  }

  togglePanel(name) {
    if (!this.game.my_player) return;
    if (this.panel === name) this.closePanel();
    else this.openPanel(name);
  }

  openPanel(name) {
    this.panel = name;
    show($("chestCard"), false);
    this.lastRendered = null;
    show($("gamePanel"), true, "flex");
    this.renderPanel();
  }

  closePanel() {
    this.panel = null;
    show($("gamePanel"), false);
    if (this.chest) this.updateChest(this.chest);
  }

  /** Redraws the open panel (called when its data changes). */
  renderPanel() {
    if (!this.panel) return;
    const root = $("gamePanel");
    // Keep what the player typed or picked across re-renders.
    const kept = new Map();
    const samePanel = this.lastRendered === this.panel;
    if (samePanel) {
      root.querySelectorAll("input[id], select[id]").forEach((el) => {
        if (el.type !== "file" && el.type !== "checkbox") kept.set(el.id, el.value);
      });
    }
    const focused = samePanel ? document.activeElement?.id : null;
    const scroll = $("panelBody")?.scrollTop ?? 0;
    this.lastRendered = this.panel;

    const [icon, title] = PANEL_TITLES[this.panel] || ["", ""];
    root.innerHTML = `
      <div class="panelHeader">
        ${iconHtml(icon)}<span class="panelTitle">${title}</span>
        <span class="panelChip" id="panelChip"></span>
        <span class="iconButton" id="panelClose" title="Close (Esc)">${iconHtml("close")}</span>
      </div>
      <div id="panelTabs"></div>
      <div id="panelBody"></div>`;
    $("panelClose").onclick = () => this.closePanel();
    this.renderInto($("panelBody"), this.panel);

    for (const [id, value] of kept) {
      const el = document.getElementById(id);
      if (el && root.contains(el)) el.value = value;
    }
    if (focused) document.getElementById(focused)?.focus();
    $("panelBody").scrollTop = scroll;
  }

  renderInto(body, name) {
    const render = {
      shop: () => this.renderShop(body),
      skins: () => this.renderSkins(body),
      customize: () => this.renderCustomize(body),
      clan: () => this.renderClan(body),
      arena: () => this.renderArena(body),
      servers: () => this.renderServers(body),
      account: () => this.renderAccount(body),
      settings: () => this.renderSettings(body),
      help: () => this.renderHelp(body),
    }[name];
    render?.();
  }

  /** Re-renders whatever shows this data. */
  refresh(name) {
    const panels = { shop: ["shop", "skins", "customize", "account"], arena: ["arena", "servers"] }[name] || [name];
    if (panels.includes(this.panel)) this.renderPanel();
    if (!this.game.my_player && panels.includes(this.menuScreen)) this.renderMenuCard();
    if (name === "shop" || name === "account") this.renderAccountCard();
  }

  /** The number in the panel header (gold or coins). */
  chip(html) {
    const el = $("panelChip");
    if (el) el.innerHTML = html;
  }

  /** A button: an icon, optional label. */
  button(label, onClick, disabled = false, icon = "", kind = "") {
    const b = document.createElement("span");
    b.className = `btn${kind ? ` ${kind}` : ""}${disabled ? " disabled" : ""}`;
    b.innerHTML = `${icon ? iconHtml(icon) : ""}${label ? `<span>${esc(label)}</span>` : ""}`;
    if (label === "" && icon) b.classList.add("iconOnly");
    if (!disabled) b.onclick = onClick;
    return b;
  }

  /** Icon tabs (in the panel's tab row when there is one). */
  tabs(body, items, active, onPick) {
    const bar = document.createElement("div");
    bar.className = "tabs";
    for (const [id, icon, label] of items) {
      const t = document.createElement("span");
      t.className = `tab${id === active ? " active" : ""}`;
      t.innerHTML = `${iconHtml(icon)}<span>${label}</span>`;
      if (id !== active) t.onclick = () => onPick(id);
      bar.appendChild(t);
    }
    ($("panelTabs") && body.id === "panelBody" ? $("panelTabs") : body).appendChild(bar);
  }

  /** A titled section. */
  section(body, icon, title) {
    const s = document.createElement("div");
    s.className = "section";
    s.innerHTML = `<div class="sectionTitle">${iconHtml(icon)}<span>${esc(title)}</span></div>`;
    body.appendChild(s);
    return s;
  }

  /** A list row: content on the left, buttons on the right. Returns the
   * button holder. */
  row(parent, html) {
    const r = document.createElement("div");
    r.className = "listRow";
    r.innerHTML = `<span class="rowMain">${html}</span><span class="rowActions"></span>`;
    parent.appendChild(r);
    return r.querySelector(".rowActions");
  }

  /** A muted line of text. */
  note(parent, text) {
    const d = document.createElement("div");
    d.className = "note";
    d.textContent = text;
    parent.appendChild(d);
  }

  // ---------------------------------------------------------------- shop --

  shopTabs(body, active) {
    this.tabs(
      body,
      [
        ["hats", "face", "Hats"],
        ["accessories", "backpack", "Accessories"],
        ["skins", "palette", "Skins"],
      ],
      active,
      (tab) => {
        if (tab === "skins") return this.openPanel("skins");
        this.shopTab = tab;
        if (this.panel === "shop") this.renderPanel();
        else this.openPanel("shop");
      },
    );
  }

  renderShop(body) {
    const shop = this.game.shop || { hats: [], accessories: [], hat: 0, accessory: 0 };
    const gold = this.game.resources?.gold ?? 0;
    this.chip(`${coinHtml()}${gold}`);
    this.shopTabs(body, this.shopTab);
    const isHats = this.shopTab === "hats";
    const list = isHats ? HATS : ACCESSORIES;
    const owned = isHats ? shop.hats : shop.accessories;
    const worn = isHats ? shop.hat : shop.accessory;
    const kind = isHats ? 0 : 1;
    const grid = document.createElement("div");
    grid.className = "cardGrid";
    for (const h of list) {
      const has = owned.includes(h.id);
      const wearing = worn === h.id;
      const card = document.createElement("div");
      card.className = `card${wearing ? " selected" : ""}`;
      card.title = h.desc || "";
      card.innerHTML = `
        <img src="${h.texture}" alt="">
        <div class="cardName">${esc(h.name)}</div>
        ${h.desc ? `<div class="cardDesc">${esc(h.desc)}</div>` : ""}
        <div class="cardPrice">${has ? iconHtml(wearing ? "check_circle" : "inventory_2") : `${coinHtml()}${h.price}`}</div>`;
      if (!has) {
        card.appendChild(
          this.button("Buy", () => this.game.send(C.SHOP, { buy: true, kind, id: h.id }), gold < h.price, "shopping_cart", "primary"),
        );
      } else if (wearing) {
        card.appendChild(this.button("Remove", () => this.game.send(C.SHOP, { buy: false, kind, id: 0 }), false, "remove_circle"));
      } else {
        card.appendChild(this.button("Wear", () => this.game.send(C.SHOP, { buy: false, kind, id: h.id }), false, "checkroom", "primary"));
      }
      grid.appendChild(card);
    }
    body.appendChild(grid);
  }

  /** Skin swatches. Skins cost coins and stay on your account. */
  skinGrid(body) {
    const shop = this.game.shop || { skins: [0, 1, 2], skin: 0, coins: 0 };
    const inGame = !!this.game.my_player;
    const grid = document.createElement("div");
    grid.className = "cardGrid small";
    SKINS.forEach((color, id) => {
      const has = shop.skins.includes(id);
      const selected = shop.skin === id;
      const card = document.createElement("div");
      card.className = `card${selected ? " selected" : ""}`;
      card.innerHTML = `<div class="swatch" style="background:${hex(color)}"></div>
        <div class="cardPrice">${has ? iconHtml(selected ? "check_circle" : "inventory_2") : `${iconHtml("toll")}${SKIN_PRICE}`}</div>`;
      card.appendChild(
        this.button(
          has ? "Use" : "Buy",
          () => this.game.send(C.SHOP, { buy: !has, kind: 2, id }),
          !inGame || selected || (!has && shop.coins < SKIN_PRICE),
          has ? "check" : "shopping_cart",
          "primary",
        ),
      );
      grid.appendChild(card);
    });
    body.appendChild(grid);
    if (!inGame) this.note(body, "Join a game to change your skin.");
  }

  renderSkins(body) {
    this.chip(`${iconHtml("toll")}${this.game.shop?.coins ?? 0}`);
    this.shopTabs(body, "skins");
    this.note(body, `Skins cost coins (you earn them every game) and stay on your account.`);
    this.skinGrid(body);
  }

  renderCustomize(body) {
    const coins = this.game.shop?.coins;
    if (coins != null) this.note(body, `You have ${coins} coins. Skins cost ${SKIN_PRICE}.`);
    this.skinGrid(body);
    this.texturePackSection(body);
  }

  // ---------------------------------------------------------------- clan --

  renderClan(body) {
    const state = this.game.clanState || { clans: [], mine: null, requests: [] };
    const myId = this.game.myId;
    const mine = state.clans.find((c) => c.id === state.mine);
    const send = (action, extra = {}) => this.game.send(C.CLAN, { action, name: "", target: 0, ...extra });

    if (!mine) {
      const create = this.section(body, "add_circle", "Create a clan");
      const form = document.createElement("div");
      form.className = "form";
      form.innerHTML = `<input id="clanName" class="input" maxlength="8" placeholder="Clan name">`;
      form.appendChild(this.button("Create", () => send(0, { name: $("clanName").value }), false, "add", "primary"));
      create.appendChild(form);
      const join = this.section(body, "groups", "Join a clan");
      if (!state.clans.length) this.note(join, "No clans yet. Make the first one!");
      for (const c of state.clans) {
        this.row(join, `<b>[${esc(c.name)}]</b><span class="muted">${iconHtml("person")}${c.members.length}</span>`).appendChild(
          this.button("Ask to join", () => send(1, { target: c.id }), false, "person_add"),
        );
      }
      return;
    }

    const me = mine.members.find((m) => m.id === myId);
    const myRole = me?.role ?? 0;
    const roleIcon = ["person", "shield", "star"];
    const members = this.section(body, "groups", `[${mine.name}]`);
    for (const m of mine.members) {
      const actions = this.row(
        members,
        `${iconHtml(roleIcon[m.role])}<span>${esc(m.name)}${m.id === myId ? ' <span class="muted">(you)</span>' : ""}</span>`,
      );
      if (m.id === myId) continue;
      if (myRole === 2 && m.role === 0) actions.appendChild(this.button("", () => send(6, { target: m.id }), false, "arrow_upward"));
      if (myRole === 2 && m.role === 1) actions.appendChild(this.button("", () => send(7, { target: m.id }), false, "arrow_downward"));
      if ((myRole === 2 && m.role < 2) || (myRole === 1 && m.role === 0)) {
        actions.appendChild(this.button("", () => send(5, { target: m.id }), false, "person_remove", "danger"));
      }
    }
    if (myRole > 0) {
      const reqs = this.section(body, "mail", "Join requests");
      if (!state.requests.length) this.note(reqs, "No requests.");
      for (const r of state.requests) {
        const actions = this.row(reqs, `${iconHtml("person")}<span>${esc(r.name)}</span>`);
        actions.appendChild(this.button("", () => send(3, { target: r.id }), false, "check", "primary"));
        actions.appendChild(this.button("", () => send(4, { target: r.id }), false, "close", "danger"));
      }
    }

    const trade = this.section(body, "swap_horiz", "Send resources");
    const others = mine.members.filter((m) => m.id !== myId);
    const form = document.createElement("div");
    form.className = "form";
    form.innerHTML = `
      <select id="tradeTo" class="input">${others.map((m) => `<option value="${m.id}">${esc(m.name)}</option>`).join("")}</select>
      ${CHEST_RESOURCES.map(
        ([k, img]) => `<label class="amount"><img src="${img}" alt="${k}"><input id="t_${k}" class="input" type="number" min="0" value="0"></label>`,
      ).join("")}`;
    const num = (id) => Math.max(0, Math.floor(Number($(id).value) || 0));
    form.appendChild(
      this.button(
        "Send",
        () =>
          this.game.send(C.TRADE, {
            to: Number($("tradeTo").value),
            wood: num("t_wood"),
            stone: num("t_stone"),
            food: num("t_food"),
            gold: num("t_gold"),
          }),
        !others.length,
        "send",
        "primary",
      ),
    );
    trade.appendChild(form);
    if (!others.length) this.note(trade, "Nobody else in your clan yet.");
    const leave = document.createElement("div");
    leave.className = "form end";
    leave.appendChild(this.button("Leave clan", () => send(2), false, "logout", "danger"));
    body.appendChild(leave);
  }

  // --------------------------------------------------------------- arena --

  renderArena(body) {
    const state = this.game.arenaState || { rooms: [], mine: null, result: "" };
    const room = state.rooms.find((r) => r.id === state.mine);
    const maps = ["Open field", "Pillars", "Maze"];
    const send = (action, extra = {}) =>
      this.game.send(C.ARENA, {
        action,
        room: 0,
        team_size: 1,
        map: 0,
        max_health: 100,
        object_damage: 1,
        bounciness: 1,
        primary: 0,
        secondary: 255,
        ...extra,
      });
    if (state.result) this.note(body, `Last fight: ${state.result}`);

    if (!room) {
      const create = this.section(body, "add_circle", "Create a room");
      const form = document.createElement("div");
      form.className = "form";
      form.innerHTML = `
        <select id="aSize" class="input">${[1, 2, 3, 4].map((n) => `<option value="${n}">${n}v${n}</option>`).join("")}</select>
        <select id="aMap" class="input">${maps.map((m, i) => `<option value="${i}">${m}</option>`).join("")}</select>
        <label class="amount" title="Health">${iconHtml("favorite")}<input id="aHp" class="input" type="number" value="100" min="50" max="500"></label>
        <label class="amount" title="Building damage">${iconHtml("home")}<input id="aObj" class="input" type="number" value="1" min="0" max="5" step="0.5"></label>
        <label class="amount" title="Knockback">${iconHtml("open_with")}<input id="aBounce" class="input" type="number" value="1" min="0" max="3" step="0.5"></label>`;
      form.appendChild(
        this.button(
          "Create",
          () =>
            send(0, {
              team_size: Number($("aSize").value),
              map: Number($("aMap").value),
              max_health: Number($("aHp").value) || 100,
              object_damage: Number($("aObj").value) || 0,
              bounciness: Number($("aBounce").value) || 0,
            }),
          false,
          "add",
          "primary",
        ),
      );
      create.appendChild(form);
      const open = this.section(body, "meeting_room", "Open rooms");
      if (!state.rooms.length) this.note(open, "No rooms yet. Create one!");
      for (const r of state.rooms) {
        const full = r.players.length >= r.team_size * 2;
        const actions = this.row(
          open,
          `<b>${r.team_size}v${r.team_size}</b><span>${maps[r.map]}</span><span class="muted">${iconHtml("person")}${r.players.length}/${r.team_size * 2}</span><span class="muted">${esc(r.host)}</span>${r.started ? `<span class="muted">${iconHtml("sports_kabaddi")}</span>` : ""}`,
        );
        actions.appendChild(this.button("Join", () => send(1, { room: r.id }), r.started || full, "login", "primary"));
      }
      return;
    }

    const isHost = room.players[0]?.id === this.game.myId;
    const teams = this.section(body, "sports_kabaddi", `${room.team_size}v${room.team_size} ${maps[room.map]}`);
    const team = (t) =>
      room.players
        .filter((p) => p.role === t)
        .map((p) => esc(p.name))
        .join(", ") || '<span class="muted">empty</span>';
    for (const t of [0, 1]) this.row(teams, `<b>Team ${t + 1}</b><span>${team(t)}</span>`);
    this.row(
      teams,
      `<span title="Health">${iconHtml("favorite")}${room.max_health}</span><span title="Building damage">${iconHtml("home")}x${room.object_damage}</span><span title="Knockback">${iconHtml("open_with")}x${room.bounciness}</span>`,
    );

    const loadout = this.section(body, "construction", "Your loadout");
    const option = (w) => `<option value="${w.id}">${esc(w.name)}</option>`;
    const form = document.createElement("div");
    form.className = "form";
    form.innerHTML = `
      <select id="aPrimary" class="input">${WEAPONS.filter((w) => w.id <= 9 || w.id === 29).map(option).join("")}</select>
      <select id="aSecondary" class="input"><option value="255">No secondary</option>${WEAPONS.filter((w) => w.id >= 10 && w.id !== 29)
        .map(option)
        .join("")}</select>`;
    form.appendChild(
      this.button(
        "Save",
        () => {
          this.arenaLoadout = { primary: Number($("aPrimary").value), secondary: Number($("aSecondary").value) };
          send(4, this.arenaLoadout);
          this.game.hud?.toast("Loadout saved", 2);
        },
        false,
        "save",
        "primary",
      ),
    );
    loadout.appendChild(form);
    if (this.arenaLoadout) {
      $("aPrimary").value = this.arenaLoadout.primary;
      $("aSecondary").value = this.arenaLoadout.secondary;
    }
    const buttons = document.createElement("div");
    buttons.className = "form end";
    if (!room.started) buttons.appendChild(this.button("Start fight", () => send(3), !isHost, "play_arrow", "primary"));
    buttons.appendChild(this.button("Leave room", () => send(2), false, "logout", "danger"));
    body.appendChild(buttons);
  }

  // ------------------------------------------------- servers / account --

  renderServers(body) {
    const online = this.game.hud?.leaderboard?.online;
    const rooms = this.game.arenaState?.rooms?.length ?? 0;
    const list = document.createElement("div");
    body.appendChild(list);
    this.row(
      list,
      `${iconHtml("dns")}<b>Local server</b><span class="muted">${this.game.connected ? `${iconHtml("person")}${online ?? "?"}` : "offline"}</span>`,
    );
    this.row(list, `${iconHtml("sports_kabaddi")}<span>Arena rooms</span><span class="muted">${rooms}</span>`);
  }

  renderAccount(body) {
    const a = this.game.account || this.savedAccount;
    const coins = this.game.shop?.coins;
    const list = document.createElement("div");
    body.appendChild(list);
    if (a) {
      this.row(list, `${iconHtml("emoji_events")}<span>Best score</span><b>${esc(a.best_score)}</b>`);
      this.row(list, `${iconHtml("whatshot")}<span>Total kills</span><b>${esc(a.total_kills)}</b>`);
      this.row(list, `${iconHtml("sports_esports")}<span>Games</span><b>${esc(a.games)}</b>`);
      if (coins != null) this.row(list, `${iconHtml("toll")}<span>Coins</span><b>${esc(coins)}</b>`);
    } else {
      this.note(list, "Play a game to start your account.");
    }
    const reset = document.createElement("div");
    reset.className = "form";
    reset.appendChild(
      this.button(
        "New account",
        () => {
          if (!confirm("Start a new account? Your stats, coins and skins on this browser are lost.")) return;
          localStorage.removeItem("roamer-token");
          localStorage.removeItem("roamer-account");
          this.savedAccount = null;
          this.game.account = null;
          this.refresh("account");
        },
        false,
        "restart_alt",
        "danger",
      ),
    );
    body.appendChild(reset);
  }

  // -------------------------------------------------- settings / help --

  texturePackSection(body) {
    const s = this.section(body, "image", "Texture pack");
    this.note(s, `Pick images named like the game's own (tree_1.png, hat_7.png...). ${texturePackSize()} loaded.`);
    const form = document.createElement("div");
    form.className = "form";
    form.innerHTML = `<input id="packFiles" type="file" accept="image/*" multiple style="display:none">`;
    form.appendChild(this.button("Choose images", () => $("packFiles").click(), false, "upload", "primary"));
    form.appendChild(
      this.button(
        "Remove",
        async () => {
          await clearTexturePack();
          location.reload();
        },
        false,
        "delete",
        "danger",
      ),
    );
    s.appendChild(form);
    $("packFiles").onchange = async (e) => {
      const count = await saveTexturePack(e.target.files);
      this.game.hud?.toast(`Saved ${count} images. Reloading...`, 2);
      setTimeout(() => location.reload(), 800);
    };
  }

  /** An on/off switch row. */
  toggleRow(parent, icon, label, on, onChange) {
    const actions = this.row(parent, `${iconHtml(icon)}<span>${label}</span>`);
    const t = document.createElement("span");
    t.className = `switch${on ? " on" : ""}`;
    t.innerHTML = "<span></span>";
    t.onclick = () => onChange(!on);
    actions.appendChild(t);
  }

  renderSettings(body) {
    const s = this.section(body, "tune", "Game");
    this.toggleRow(s, "videocam", "Lean camera to mouse", this.game.cameraMouseMode, (on) => {
      this.game.cameraMouseMode = on;
      this.refresh("settings");
    });
    this.toggleRow(s, "leaderboard", "Show leaderboard", !this.game.hud?.leaderboard?.hidden, () => {
      this.toggleLeaderboard();
      this.refresh("settings");
    });
    const zoomActions = this.row(s, `${iconHtml("zoom_in")}<span>Zoom</span>`);
    const r = this.game.renderer;
    const zoom = (step) => {
      r.zoomFactor = Math.min(1.3, Math.max(0.7, Math.round((r.zoomFactor + step) * 20) / 20));
      try {
        localStorage.setItem("roamer-zoom", String(r.zoomFactor));
      } catch {
        // Not saved; still applies now.
      }
      $("zoomValue").textContent = `${Math.round(r.zoomFactor * 100)}%`;
    };
    zoomActions.appendChild(this.button("", () => zoom(-0.05), false, "remove"));
    const value = document.createElement("span");
    value.id = "zoomValue";
    value.className = "value";
    value.textContent = `${Math.round(r.zoomFactor * 100)}%`;
    zoomActions.appendChild(value);
    zoomActions.appendChild(this.button("", () => zoom(0.05), false, "add"));
    this.texturePackSection(body);
  }

  renderHelp(body) {
    const keys = document.createElement("div");
    keys.className = "keys";
    keys.innerHTML = CONTROLS.map(([k, v]) => `<kbd>${k}</kbd><span>${v}</span>`).join("");
    body.appendChild(keys);
    const how = this.section(body, "lightbulb", "How it works");
    this.note(
      how,
      `Hit trees, rocks, bushes and gold to gather. Every age gives you an upgrade: weapons, buildings, and guns from
      age 5. Use a weapon enough and it turns Gold, Diamond, Ruby, then Emerald. Crates hold ammo and resources; their
      color shows the weapon variant that opens them. Stand on a cave entrance or Tunnel and press X to go underground,
      where you can dig, and build Tunnels back up.`,
    );
  }

  // -------------------------------------------------------------- chest --

  updateChest(c) {
    this.chest = c;
    const card = $("chestCard");
    // Hidden behind open panels so they don't overlap.
    const visible = c.has_chest && c.near && !!this.game.my_player && !this.panel;
    show(card, visible);
    if (!visible) return;
    if (!this.chestBuilt) this.buildChest(card);
    this.refreshChest();
  }

  /** Builds the chest card once, so typed amounts survive updates. */
  buildChest(card) {
    this.chestBuilt = true;
    card.innerHTML = `
      <div class="panelHeader">${iconHtml("inventory_2")}<span class="panelTitle">Chest</span></div>
      <div class="chestRow chestHead">
        <span></span>
        <span title="You">${iconHtml("person")}</span>
        <span title="In the chest">${iconHtml("inventory_2")}</span>
        <span title="Amount to move">${iconHtml("edit")}</span>
      </div>
      ${CHEST_RESOURCES.map(
        ([k, icon]) => `<div class="chestRow">
          <img src="${icon}" alt="${k}" title="${k}">
          <span id="chestYou_${k}">0</span>
          <span id="chestIn_${k}">0</span>
          <input id="chestAmt_${k}" class="input" type="number" min="0" placeholder="0">
        </div>`,
      ).join("")}
      <div id="chestButtons" class="form"></div>`;
    const amounts = () =>
      Object.fromEntries(CHEST_RESOURCES.map(([k]) => [k, Math.max(0, Math.floor(Number($(`chestAmt_${k}`).value) || 0))]));
    const everything = Object.fromEntries(CHEST_RESOURCES.map(([k]) => [k, ALL]));
    const send = (deposit, what) => this.game.send(C.CHEST, { deposit, ...what });
    const buttons = $("chestButtons");
    buttons.appendChild(this.button("Put in", () => send(true, amounts()), false, "move_to_inbox", "primary"));
    buttons.appendChild(this.button("Take out", () => send(false, amounts()), false, "outbox", "primary"));
    buttons.appendChild(this.button("All in", () => send(true, everything), false, "keyboard_double_arrow_down"));
    buttons.appendChild(this.button("All out", () => send(false, everything), false, "keyboard_double_arrow_up"));
  }

  /** Updates the counts on the chest card. */
  refreshChest() {
    if (!this.chestBuilt || !this.chest) return;
    const res = this.game.resources || {};
    for (const [k] of CHEST_RESOURCES) {
      $(`chestYou_${k}`).textContent = res[k] ?? 0;
      $(`chestIn_${k}`).textContent = this.chest[k] ?? 0;
    }
  }

  // --------------------------------------------------------- game info --

  /** Age and XP bar above the hotbar. */
  setAge({ age, xp, max_xp }) {
    this.age = { age, xp, max_xp };
    $("effectsList").textContent = `AGE ${age}`;
    show($("staminaDisplay"), true);
    this.drawXp();
  }

  drawXp() {
    const c = this.xpBar;
    const w = c.clientWidth || 400;
    const h = c.clientHeight || 16;
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
    }
    const ctx = this.xpCtx;
    ctx.clearRect(0, 0, w, h);
    if (!this.age) return;
    const ratio = this.age.max_xp ? Math.min(1, this.age.xp / this.age.max_xp) : 1;
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, w * ratio, h);
  }

  setQuest(q) {
    this.quest = q.text ? q : null;
    this.renderInfo();
  }

  setEvent(e) {
    const started = e.name && e.name !== this.event?.name;
    this.event = e.name ? e : null;
    this.renderInfo();
    if (started) this.announce(e.name, e.description);
  }

  /** Quest and world event, next to the minimap. */
  renderInfo() {
    const el = $("infoDisplay");
    const parts = [];
    if (this.event) {
      parts.push(
        `<div class="infoTitle"><span>EVENT</span><span>${formatTime(this.event.seconds_left)}</span></div>
         <div>${esc(this.event.name)}</div>
         <div class="infoDSub">${esc(this.event.description)}</div>`,
      );
    }
    if (this.quest) {
      const q = this.quest;
      const done = Math.min(q.progress, q.goal);
      parts.push(
        `<div class="infoTitle"><span>QUEST</span><span>${q.reward} GOLD</span></div>
         <div>${esc(q.text)}</div>
         <div class="infoBar"><div style="width:${Math.round((done / Math.max(1, q.goal)) * 100)}%"></div></div>
         <div class="infoDSub">${done} / ${q.goal}</div>`,
      );
    }
    el.innerHTML = parts.join(`<div style="height:10px"></div>`);
    show(el, parts.length > 0 && !!this.game.my_player);
  }

  /** Big text at the top of the screen for a few seconds. */
  announce(title, info = "") {
    $("killText").textContent = title;
    $("killInfo").textContent = info;
    const cont = $("killTextCont");
    cont.style.opacity = "1";
    cont.style.transition = "none";
    clearTimeout(this._announceTimer);
    this._announceTimer = setTimeout(() => {
      cont.style.transition = "opacity 0.8s";
      cont.style.opacity = "0";
    }, 3500);
  }



  setMinimap(points) {
    this.minimapPoints = points;
  }

  /** Called every frame. */
  update() {
    this.drawMinimap();
  }

  drawMinimap() {
    const c = this.minimap;
    const size = c.clientWidth || 120;
    if (c.width !== size) c.width = c.height = size;
    const ctx = this.minimapCtx;
    ctx.clearRect(0, 0, size, size);
    const k = size / MAP_SIZE;
    const dot = (x, y, r, color) => {
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x * k, y * k, r, 0, Math.PI * 2);
      ctx.fill();
    };
    for (const p of this.minimapPoints) {
      dot(p.x, p.y, p.kind === 1 ? 4 : 3, ["#8ecc51", "#ff5a4a", "#f5d742"][p.kind] || "#fff");
    }
    if (this.deathAt) {
      ctx.fillStyle = "#ff5a4a";
      ctx.font = "14px GameFont";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText("x", this.deathAt.x * k, this.deathAt.y * k);
    }
    const me = this.game.my_player;
    if (me && me.x <= MAP_SIZE) dot(me._rx, me._ry, 3.5, "#fff");
  }
}
