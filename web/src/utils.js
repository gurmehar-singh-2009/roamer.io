export default function utils() {
  this.findPlayerByID = (id) => {
    return this.game.players.find((player) => player.id === id);
  };

  this.findAnimalByID = (id) => {
    return this.game.animals.find((animal) => animal.id === id);
  };

  this.findObjectByID = (id) => {
    return this.game.objects.find((object) => object.id === id);
  };

  this.normalize = (x, minIn, maxIn, minOut, maxOut) =>
    ((x - minIn) * (maxOut - minOut)) / (maxIn - minIn) + minOut;

  return this;
}

export function format(number) {
  if (number >= 1_000_000) return (number / 1_000_000).toFixed(1) + "m";
  if (number >= 1_000) return (number / 1_000).toFixed(1) + "k";
  return String(number);
}

/**
 * Critically-damped smoothing (Unity SmoothDamp). Returns [value, velocity].
 * Carries velocity, so it tracks a moving target without pulsing,
 * never overshoots, and is frame-rate independent.
 */
export function smoothDamp(current, target, velocity, smoothTime, dt) {
  if (dt <= 0) return [current, velocity];

  smoothTime = Math.max(1e-4, smoothTime);
  const omega = 2 / smoothTime;
  const x = omega * dt;
  const exp = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);

  const change = current - target;
  const temp = (velocity + omega * change) * dt;
  let newVelocity = (velocity - omega * temp) * exp;
  let output = target + (change + temp) * exp;

  // Never overshoot.
  if (target - current > 0 === output > target) {
    output = target;
    newVelocity = 0;
  }
  return [output, newVelocity];
}
