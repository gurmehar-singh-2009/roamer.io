import { smoothDamp } from "../utils.js";

export default class Camera {
  constructor(x = 0, y = 0) {
    this.x = x;
    this.y = y;
    this.zoom = 1;
    // Approx. time to settle on the target. 0.06–0.12 = tight,
    // 0.25+ = floaty. Was 0.1 (felt too rapid) — bumped up.
    // Keep tuning this one number to taste; nothing else needs to change.
    this.smoothTime = 0.2;
    this._vx = 0;
    this._vy = 0;
  }

  update(dt, tx, ty) {
    // Teleport/respawn → jump. MUST zero velocity, or the spring
    // slingshots after the snap. Keep this threshold well under the
    // player's SNAP_DIST so they always snap together.
    if (Math.hypot(tx - this.x, ty - this.y) > 400) {
      this.x = tx;
      this.y = ty;
      this._vx = 0;
      this._vy = 0;
      return;
    }

    // One hitched frame (GC, tab switch) can't fling the camera.
    dt = Math.min(dt, 0.05);

    [this.x, this._vx] = smoothDamp(this.x, tx, this._vx, this.smoothTime, dt);
    [this.y, this._vy] = smoothDamp(this.y, ty, this._vy, this.smoothTime, dt);
  }
}
