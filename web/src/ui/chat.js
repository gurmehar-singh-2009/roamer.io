import { Container, Graphics, Text } from "pixi.js";
import { UI, uiTextStyle } from "./theme";

const MAX_MESSAGES = 6;
const MAX_LEN = 80;
const INPUT_W = 380;

export class ChatUI {
  constructor(game) {
    this.game = game;
    this.isOpen = false;
    this.buffer = "";
    this.msgs = [];
    this._blink = 0;

    this.container = new Container();
    game.renderer.ui.addChild(this.container);

    // message log (stacks up above the input)
    this.log = new Container();
    this.container.addChild(this.log);

    // input box
    this.input = new Container();
    this.input.visible = false;

    this.inputBg = new Graphics();
    this.inputBg
      .roundRect(0, 0, INPUT_W, 36, UI.radius)
      .fill({ color: UI.panel, alpha: UI.alpha.active });

    this.prompt = new Text({
      text: ">",
      style: uiTextStyle({ size: 16, weight: "bold" }),
    });
    this.prompt.x = 12;
    this.prompt.y = 9;
    this.prompt.alpha = 0.6;

    this.inputLabel = new Text({ text: "", style: uiTextStyle({ size: 16 }) });
    this.inputLabel.x = 32;
    this.inputLabel.y = 9;

    this.cursor = new Graphics().rect(0, 0, 2, 18).fill(0xffffff);
    this.cursor.y = 9;
    this.cursor.alpha = 0.9;

    this.input.addChild(
      this.inputBg,
      this.prompt,
      this.inputLabel,
      this.cursor,
    );
    this.container.addChild(this.input);
  }

  open() {
    this.isOpen = true;
    this.buffer = "";
    this._refresh();
    this.input.visible = true;
    this._blink = 0;

    // release held movement keys so nothing stays stuck while typing
    this._releaseMovement();
  }

  close() {
    this.isOpen = false;
    this.buffer = "";
    this.input.visible = false;
    this._releaseMovement();
  }

  _releaseMovement() {
    const g = this.game;
    g.moveInput.up =
      g.moveInput.down =
      g.moveInput.left =
      g.moveInput.right =
        false;
    g.lastMoveDir = null;
    g.sendMove(null).catch(() => {});
  }

  key(e) {
    if (e.type !== "keydown") return;

    if (e.key === "Enter") {
      const msg = this.buffer.trim();
      this.close();
      if (msg) this.game.sendEncrypted({ message: msg }, 12).catch(() => {});
      return;
    }
    if (e.key === "Escape") {
      this.close();
      return;
    }
    if (e.key === "Backspace") {
      e.preventDefault();
      this.buffer = this.buffer.slice(0, -1);
      this._refresh();
      return;
    }
    if (e.key.length === 1 && this.buffer.length < MAX_LEN) {
      if (e.key === " ") e.preventDefault();
      this.buffer += e.key;
      this._refresh();
    }
  }

  push(text) {
    if (!text) return;
    if (text.length > 64) text = text.slice(0, 61) + "…";

    const t = new Text({ text, style: uiTextStyle({ size: 15 }) });
    this.log.addChild(t);
    this.msgs.unshift({ obj: t, age: 0 });

    while (this.msgs.length > MAX_MESSAGES) {
      this.msgs.pop().obj.destroy();
    }
    this._relayout();
  }

  _relayout() {
    for (let i = 0; i < this.msgs.length; i++) {
      this.msgs[i].obj.y = -40 - i * 25;
    }
  }

  _refresh() {
    this.inputLabel.text = this.buffer;
  }

  update(delta) {
    if (this.isOpen) this._blink += delta;

    let changed = false;
    for (let i = this.msgs.length - 1; i >= 0; i--) {
      const m = this.msgs[i];
      m.age += delta;

      let alpha = 1;
      if (!this.isOpen && m.age > 7) {
        alpha = Math.max(0, 1 - (m.age - 7) / 2);
      }
      m.obj.alpha = alpha;

      if (alpha <= 0) {
        m.obj.destroy();
        this.msgs.splice(i, 1);
        changed = true;
      }
    }
    if (changed) this._relayout();

    this.cursor.visible = this.isOpen && Math.sin(this._blink * 9) > -0.3;
    if (this.isOpen) this.cursor.x = 32 + this.inputLabel.width + 4;
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}
