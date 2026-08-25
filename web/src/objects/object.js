export default class GameObject {
  constructor({ id, x, y, dir, scale, type_obj }) {
    this.id = id;
    this.x = x;
    this.y = y;
    this.dir = dir;
    this.scale = scale;
    this.type_obj = type_obj;
    this.wiggleX = 0;
    this.wiggleY = 0;
    this.wiggleAway = true;
    this.hitDir;
    this.hit = false;
  }

  onHit(dir) {
    this.hit = true;
    this.hitDir = dir;
    this.wiggleProgress = 0;
  }

  updateWiggle(dt) {
    if (!this.hit) return;

    const DURATION = 0.5;
    const AMPLITUDE = 6.7;

    this.wiggleProgress += dt / DURATION;

    if (this.wiggleProgress >= 1) {
      this.wiggleProgress = 0;
      this.wiggleX = 0;
      this.wiggleY = 0;
      this.hit = false;
      return;
    }

    const t = this.wiggleProgress;
    const displacement = AMPLITUDE * Math.sin(t * Math.PI * 2.5 + Math.PI) * Math.pow(1 - t, 1.5);

    this.wiggleX = displacement * Math.cos(this.hitDir);
    this.wiggleY = displacement * Math.sin(this.hitDir);
  }
}
