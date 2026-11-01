import { Container, Graphics, Text } from "pixi.js";
import { Leaderboard, LEADERBOARD_W } from "./leaderboard";
import { ActionBar } from "./actionBar";
import { UpgradeBar } from "./upgradeBar";
import { ResourceDisplay, ROW_W as RES_ROW_W, RESOURCES_H } from "./resourceDisplay";
import { ChatUI } from "./chat";
import { UI, uiTextStyle } from "./theme";
import { UI_Z } from "../render/layers";

/** Toast colors by notice kind: info, warning, good news. */
const TOAST_COLORS = ["#ffffff", "#ff9a8a", "#ffffff"];

export class HUD {
  constructor(game) {
    this.game = game;
    this.uiHover = 0;

    const ui = game.renderer.ui;
    ui.sortableChildren = true; // global UI layer order is now explicit

    this.leaderboard = new Leaderboard(game);
    this.resources = new ResourceDisplay(game);
    this.actionBar = new ActionBar(game);
    this.upgradeBar = new UpgradeBar(game);
    this.chat = new ChatUI(game);

    // --- explicit global UI layer order (see render/layers.js) ------------
    this.leaderboard.container.zIndex = UI_Z.LEADERBOARD;
    this.resources.container.zIndex = UI_Z.RESOURCES;
    this.chat.container.zIndex = UI_Z.CHAT;
    this.actionBar.container.zIndex = UI_Z.ACTIONBAR;
    this.upgradeBar.container.zIndex = UI_Z.UPGRADE;
    if (this.upgradeBar.tooltip) this.upgradeBar.tooltip.zIndex = UI_Z.TOOLTIP;

    this.components = [
      this.leaderboard,
      this.resources,
      this.actionBar,
      this.upgradeBar,
      this.chat,
    ];

    // Short messages in a pill above the hotbar.
    this.toastText = new Text({ text: "", style: uiTextStyle({ size: 15, weight: "bold" }) });
    this.toastText.anchor.set(0.5);
    this.toastBg = new Graphics();
    this.toastBox = new Container();
    this.toastBox.addChild(this.toastBg, this.toastText);
    this.toastBox.alpha = 0;
    this.toastBox.zIndex = UI_Z.TOAST;
    ui.addChild(this.toastBox);
    this._toastT = 0;


    // hide the leftover DOM chatbox if one exists in the HTML
    const legacy = document.getElementById("chatbox");
    if (legacy) legacy.style.display = "none";

    this._w = 0;
    this._h = 0;

    // The HUD drives itself from the app ticker — it does not depend on
    // the render loop calling update().
    this._tick = (ticker) => this.update(ticker.deltaMS * 0.001);
    game.renderer.app.ticker.add(this._tick);

    this.layout();
  }

  uiHoverIn() {
    this.uiHover++;
  }
  uiHoverOut() {
    this.uiHover = Math.max(0, this.uiHover - 1);
  }
  get pointerOverUI() {
    return this.uiHover > 0;
  }

  toast(msg, kind = 0) {
    this.toastText.text = msg;
    this.toastText.style.fill = TOAST_COLORS[kind] || "#ffffff";
    const w = this.toastText.width + 28;
    const h = this.toastText.height + 12;
    this.toastBg.clear().roundRect(-w / 2, -h / 2, w, h, h / 2).fill({ color: UI.panel, alpha: UI.alpha.active });
    this.toastBox.alpha = 1;
    this._toastT = 2.2;
  }

  update(delta) {
    // auto re-layout when the canvas size changes
    const screen = this.game.renderer.app.screen;
    if (screen.width !== this._w || screen.height !== this._h) {
      this._w = screen.width;
      this._h = screen.height;
      this.layout();
    }

    for (const c of this.components) c.update(delta);

    // Only the leaderboard and chat stay up while dead or in the menu.
    const alive = !!this.game.my_player;
    this.actionBar.container.visible = alive && this.actionBar.slots.length > 0;
    this.resources.container.visible = alive;
    if (!alive) {
      this.upgradeBar.toggleVisibility(false);
      this.uiHover = 0;
    }

    if (this._toastT > 0) {
      this._toastT -= delta;
      this.toastBox.alpha = Math.min(1, this._toastT / 0.6);
    } else {
      this.toastBox.alpha = 0;
    }
  }

  layout() {
    const { width: W, height: H } = this.game.renderer.app.screen;

    const m = UI.margin;
    this.leaderboard.container.position.set(W - m - LEADERBOARD_W, m);
    this.resources.container.position.set(W - m - RES_ROW_W, H - m - RESOURCES_H);
    this.actionBar.layout(W, H);
    this.upgradeBar.layout(W, H);
    this.chat.container.position.set(m, H - m - 44);
    this.toastBox.position.set(W / 2, H - 200);
  }

  destroy() {
    this.game.renderer.app.ticker.remove(this._tick);

    for (const c of this.components) c.destroy?.();
    this.toastBox.destroy({ children: true });
  }
}
