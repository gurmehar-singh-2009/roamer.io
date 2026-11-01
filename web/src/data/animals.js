// Every animal. Ids must match server/src/catalog/animals.rs.
// `size` is the drawn size in px (about 2.6x the collision radius).

// The original Roamer.io wolf and fish.
import WolfSprite from "../assets/wolf.png";
import FishSprite from "../assets/fish_1.png";

const art = import.meta.glob("../assets/moomoo/animals/*.png", {
  eager: true,
  import: "default",
});
const img = (name) => art[`../assets/moomoo/animals/${name}.png`];

export const ANIMALS = [
  { id: 0, name: "Fish", texture: FishSprite, size: 104 },
  { id: 1, name: "Wolf", texture: WolfSprite, size: 156 },
  { id: 2, name: "Cow", texture: img("cow_1"), size: 122 },
  { id: 3, name: "Pig", texture: img("pig_1"), size: 142 },
  { id: 4, name: "Bull", texture: img("bull_1"), size: 186 },
  { id: 5, name: "Bully", texture: img("bull_2"), size: 186 },
  { id: 6, name: "Sheep", texture: img("sheep_1"), size: 130 },
  { id: 7, name: "Chicken", texture: img("chicken_1"), size: 92 },
  { id: 8, name: "Duck", texture: img("duck_1"), size: 92 },
  { id: 9, name: "Goldfish", texture: FishSprite, size: 104, tint: 0xffd23f },
  { id: 10, name: "Sid", texture: img("sid"), size: 200, boss: true },
  { id: 11, name: "Vince", texture: img("vince"), size: 200, boss: true },
  { id: 12, name: "Raider", texture: img("enemy"), size: 191, boss: true },
  { id: 13, name: "Alpha Wolf", texture: WolfSprite, size: 218, tint: 0x8a8a8a, boss: true },
];

export const ANIMAL_TEXTURES = [...new Set(ANIMALS.map((a) => a.texture))];
