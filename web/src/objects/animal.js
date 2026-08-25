import { Sprite } from "pixi.js";

export default class Animal {
    constructor({ id, animal_type, x, y }, world, texture) {
        this.id = id;
        this.type = animal_type;
        this.x = x;
        this.y = y;

        this.sprite = new Sprite(texture);
        this.sprite.anchor.set(0.5);

        const size = animal_type === 1 ? 120 : 150;
        this.sprite.width = size;
        this.sprite.height = size;

        this.sprite._rx = x;
        this.sprite._ry = y;
        this.sprite._rot = 0;
        
        this.lastUpdate = performance.now();

        world.addChild(this.sprite);
    }

    update(delta, lerpAlpha) {
        if (!this.sprite.visible) return;

        const dx = this.x - this.sprite._rx;
        const dy = this.y - this.sprite._ry;

        if (Math.abs(dx) > 0.1 || Math.abs(dy) > 0.1) {
            const targetRot = Math.atan2(dy, dx) - Math.PI / 2;
            let diff = targetRot - this.sprite._rot;
            if (diff > Math.PI) diff -= Math.PI * 2;
            if (diff < -Math.PI) diff += Math.PI * 2;

            this.sprite._rot += diff * lerpAlpha;
            this.sprite.rotation = this.sprite._rot;
        }

        this.sprite._rx += (this.x - this.sprite._rx) * lerpAlpha;
        this.sprite._ry += (this.y - this.sprite._ry) * lerpAlpha;

        this.sprite.x = this.sprite._rx;
        this.sprite.y = this.sprite._ry;
    }

    destroy() {
        this.sprite.destroy();
    }
}