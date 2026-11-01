import { Container, Graphics, Text } from "pixi.js";
import { UI, uiTextStyle } from "./theme";

const ROW_W = 240;
const ROW_H = 24;
const GAP = 3;
/** Width of the whole panel. */
export const LEADERBOARD_W = ROW_W + UI.pad * 2;
const MAX_ROWS = 10;
// Score bars: white at a low alpha (yours a bit brighter).
const BAR = 0xffffff;

/**
 * Scoreboard styled after the reference: flat dark rows with a green fill
 * bar behind the text, name on the left, score on the right. "You" gets a
 * brighter fill + subtle border. Fill widths animate smoothly.
 *  - Remote: server packet opcode 16 → { leaderboard: [{ id, name, score }] }
 *  - Local fallback: player.score (hit events).
 */
export class Leaderboard {
  constructor(game) {
    this.game = game;
    this.remoteEntries = null;
    this._dirty = true;
    this._timer = 1;

    this.container = new Container();
    this.container.visible = false;
    game.renderer.ui.addChild(this.container);

    this.bg = new Graphics();
    this.container.addChild(this.bg);


    this.rows = [];
    for (let i = 0; i < MAX_ROWS; i++) this.rows.push(this._createRow());
  }

  _createRow() {
    const row = new Container();
    row.visible = false;

    const bg = new Graphics();
    const fill = new Graphics();

    const name = new Text({ text: "", style: uiTextStyle({ size: 14 }) });
    name.anchor.set(0, 0.5);
    name.x = 8;
    name.y = ROW_H / 2;

    const score = new Text({
      text: "",
      style: uiTextStyle({ size: 14, weight: "bold" }),
    });
    score.anchor.set(1, 0.5);
    score.x = ROW_W - 8;
    score.y = ROW_H / 2;

    row.addChild(bg, fill, name, score);
    this.container.addChild(row);

    return {
      row,
      bg,
      fill,
      name,
      score,
      w: 0,
      targetW: 0,
      fillAlpha: 0.8,
      isMe: null,
      needsRedraw: false,
    };
  }

  markDirty() {
    this._dirty = true;
  }

  setRemoteEntries(entries) {
    this.remoteEntries = Array.isArray(entries) ? entries : null;
    this.markDirty();
  }

  /** Data from the server's leaderboard packet (top 10 by gold). */
  setData(data) {
    this.online = data.online;
    const icons = (id) =>
      `${data.crown === id ? "♛ " : ""}${data.skull === id ? "☠ " : ""}${data.star === id ? "★ " : ""}`;
    const entries = data.entries.map((e) => ({ id: e.id, name: icons(e.id) + e.name, score: e.score }));
    // Show "you" in the last row if you're not in the top 10.
    const me = this.game.my_player;
    if (me && !entries.some((e) => e.id === me.id)) {
      entries.push({ id: me.id, name: icons(me.id) + me.name, score: this.game.resources?.gold ?? 0 });
    }
    this.setRemoteEntries(entries);
  }

  collectEntries() {
    let list;
    if (this.remoteEntries) {
      const myId = this.game.my_player?.id;
      list = this.remoteEntries.map((e) => ({
        name: e.name,
        score: e.score || 0,
        isMe: e.id === myId,
      }));
    } else {
      const my = this.game.my_player;
      list = [...this.game.players.values()].map((p) => ({
        name: p.name,
        score: p.score || 0,
        isMe: p === my,
      }));
    }

    list.sort((a, b) => b.score - a.score);

    if (list.length <= this.maxEntriesSafe()) return list;

    // If "you" fell out of the top rows, pin them in the last slot.
    const meIdx = list.findIndex((e) => e.isMe);
    if (meIdx === -1 || meIdx < MAX_ROWS) {
      return list.slice(0, MAX_ROWS);
    }
    return [...list.slice(0, MAX_ROWS - 1), list[meIdx]];
  }

  maxEntriesSafe() {
    return MAX_ROWS;
  }

  rebuild() {
    const entries = this.collectEntries();
    this.container.visible = entries.length > 0 && !this.hidden;
    const h = UI.pad * 2 + entries.length * (ROW_H + GAP) - GAP;
    this.bg.clear().roundRect(0, 0, LEADERBOARD_W, h, UI.radius).fill({ color: UI.panel, alpha: UI.alpha.panel });


    const maxScore = Math.max(1, ...entries.map((e) => e.score || 0));

    for (let i = 0; i < this.rows.length; i++) {
      const r = this.rows[i];

      if (i >= entries.length) {
        r.row.visible = false;
        r.targetW = 0;
        continue;
      }

      const e = entries[i];
      r.row.visible = true;
      r.row.x = UI.pad;
      r.row.y = UI.pad + i * (ROW_H + GAP);

      const me = !!e.isMe;
      if (r.isMe !== me) {
        r.isMe = me;
        r.bg.clear();
        r.bg
          .roundRect(0, 0, ROW_W, ROW_H, UI.radius)
          .fill({ color: 0x000000, alpha: me ? 0.3 : 0.2 });
      }

      r.name.text = e.name || "???";
      fitText(r.name, ROW_W - 90);
      r.score.text = formatScore(e.score || 0);

      r.fillAlpha = me ? 0.3 : 0.15;
      r.targetW = Math.max(8, (ROW_W * (e.score || 0)) / maxScore);
      if (r.w === 0) r.w = Math.min(r.targetW, 14);
      r.needsRedraw = true;
    }
  }

  update(delta) {
    this._timer += delta;
    if (this._dirty && this._timer >= 0.25) {
      this._dirty = false;
      this._timer = 0;
      this.rebuild();
    }

    if (!this.container.visible) return;

    for (const r of this.rows) {
      if (!r.row.visible) continue;

      const dw = r.targetW - r.w;
      if (r.needsRedraw || Math.abs(dw) > 0.5) {
        r.w += dw * Math.min(1, delta * 8);
        this._drawFill(r);
        r.needsRedraw = false;
      }
    }
  }

  _drawFill(r) {
    const w = Math.max(6, r.w);
    r.fill.clear();
    r.fill
      .roundRect(0, 0, w, ROW_H, UI.radius)
      .fill({ color: BAR, alpha: r.fillAlpha });
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}

function formatScore(n) {
  if (n >= 1e6) return (n / 1e6).toFixed(1) + "m";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
  return String(Math.floor(n));
}

function fitText(t, maxW) {
  if (t.width <= maxW) return;
  let s = t.text;
  while (s.length > 1 && t.width > maxW) {
    s = s.slice(0, -1);
    t.text = s + "…";
  }
}
