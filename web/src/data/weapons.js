import Arm1 from "../assets/arm_1.png";
import Arm2 from "../assets/arm_2.png";
import Hammer from "../assets/weapons/hammer_1.png";
import Sword from "../assets/weapons/sword_1.png";
import Hoe from "../assets/weapons/bat_1.png";
import Knife from "../assets/weapons/spear_1.png";

export const WEAPONS = [
  {
    id: 0,
    name: "Fists",
    textures: [Arm1, Arm2],
    hoverDetails: ["Fists", "Used to punch natural objects and other players."],
  },
  {
    id: 1,
    name: "Hammer",
    textures: [Hammer],
    texture: Hammer,
    hoverDetails: [
      "Hammer",
      "Used to smash natural objects and other players.",
    ],
    xOff: -8,
    yOff: 42,
    length: 96,
    width: 72
  },
  {
    id: 2,
    name: "Sword",
    textures: [Sword],
    texture: Sword,
    hoverDetails: ["Sword", "Aggressive weapon for fighting other players."],
  },
  {
    id: 3,
    name: "Hoe",
    textures: [Hoe],
    texture: Hoe,
    hoverDetails: ["Hoe", "Ideal for gathering resources."],
  },
  {
    id: 4,
    name: "Knife",
    textures: [Knife],
    texture: Knife,
    hoverDetails: [
      "Knife",
      "Short range melee weapon that does a lot of damage.",
    ],
  },
];

// for (const weapon of WEAPONS) {
//     weapon.textures?.map(async (x) => await Assets.load(x));
// }
