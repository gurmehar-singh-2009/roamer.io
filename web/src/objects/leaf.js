import { Sprite } from "pixi.js";

export class Leaf {
    constructor(x, y, dir, texture, world) {
        this.x = x;
        this.y = y;
        this.dir = dir;
        this.sprite = new Sprite(texture);
        this.size = 67;

        this.sprite.anchor.set(0.5, 0.5);
        this.sprite.width = this.size;
        this.sprite.height = this.size;
        this.sprite.x = this.x;
        this.sprite.y = this.y;

        world.addChild(this.sprite);
    }
}

export class HitLeaf extends Leaf {
    constructor(x, y, dir, texture, world) {
        super(x, y, dir, texture, world);

        this.life = 1000;
        this.dir = dir + (Math.random() * Math.PI) - Math.PI / 2;
        this.renderAim = Math.random() * Math.PI * 2;
        this.opacity = 0;
        this.sprite.alpha = this.opacity;
    }

    update(dt) {
        this.life -= dt * 1000;
        this.renderAim += Math.PI * dt;

        this.x += 300 * dt * Math.cos(this.dir);
        this.y += 300 * dt * Math.sin(this.dir);

        this.sprite.x = this.x;
        this.sprite.y = this.y;
        this.sprite.rotation = this.renderAim;

        if (this.life < 670) {
            this.opacity -= dt * 5;
            this.size *= 0.96;
        } else {
            this.opacity = Math.min(1, this.opacity + dt * 6);
            this.size *= 1.005;
        }

        this.size *= 0.999;
        this.sprite.alpha = Math.max(0, this.opacity);
        this.sprite.width = this.size;
        this.sprite.height = this.size;
    }
}

export class MapLeaf extends Leaf {}