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
