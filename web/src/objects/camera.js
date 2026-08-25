export default class Camera {
  constructor(x, y) {
    this.x = x;
    this.y = y;
    this.zoom = 1;
  }

  update(delta, myPlayer) {
    const camDx = myPlayer.x - this.x;
    const camDy = myPlayer.y - this.y;
    const camDist = Math.hypot(camDy, camDx);

    this.x += camDx * Math.min(0.05, camDist * 0.05 * delta);
    this.y += camDy * Math.min(0.05, camDist * 0.05 * delta);
  }
}
