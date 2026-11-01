// Every weapon. Ids must match server/src/catalog/weapons.rs.
//
// Melee weapons use moomoo.io art and moomoo's drawing offsets: the sprite is
// drawn `length` x `width` px, with its top-left at
// (PLAYER_RADIUS + xOff - length / 2, yOff - width / 2) in the player's
// rotated frame (+x = where they're aiming).
// Guns use foes.io art: `sprite` has the stock at the top and is drawn
// `length` px long pointing forward.

const moomoo = import.meta.glob("../assets/moomoo/weapons/*.png", {
  eager: true,
  import: "default",
});
const guns = import.meta.glob("../assets/guns/*.png", {
  eager: true,
  import: "default",
});
const bullets = import.meta.glob("../assets/bullets/*.png", {
  eager: true,
  import: "default",
});

const mm = (name) => moomoo[`../assets/moomoo/weapons/${name}.png`];
const gunArt = (name) => guns[`../assets/guns/${name}.png`];
const bullet = (name) => bullets[`../assets/bullets/${name}.png`];

/** Variant suffixes and colors, plain first (gold, diamond, ruby, emerald). */
export const VARIANTS = [
  { name: "", suffix: "", color: 0xffffff },
  { name: "Gold", suffix: "_g", color: 0xf5d742 },
  { name: "Diamond", suffix: "_d", color: 0x6fe3ff },
  { name: "Ruby", suffix: "_r", color: 0xff5a6a },
  { name: "Emerald", suffix: "_e", color: 0x4ee08a },
];

/** A moomoo.io melee/bow weapon. */
function melee(id, name, desc, src, [length, width, xOff, yOff], extra = {}) {
  const variants = VARIANTS.map((v) => mm(src + v.suffix) || mm(src));
  return {
    id,
    name,
    desc,
    kind: "melee",
    sprite: mm(src),
    variants,
    texture: mm(src),
    length,
    width,
    xOff,
    yOff,
    ...extra,
  };
}

/** A foes.io gun. */
function gun(id, name, desc, art, length, bulletArt) {
  return {
    id,
    name,
    desc,
    kind: "gun",
    isGun: true,
    sprite: gunArt(art),
    texture: gunArt(`${art}_icon`),
    length,
    bullet: bullet(bulletArt),
  };
}

export const WEAPONS = [
  {
    id: 0,
    name: "Fists",
    desc: "Punch trees, rocks and enemies.",
    kind: "fists",
  },
  melee(1, "Tool Hammer", "Reliable tool for gathering.", "hammer_1", [140, 140, -3, 18]),
  melee(2, "Hand Axe", "Gathers 2 resources per hit.", "axe_1", [140, 140, 3, 24]),
  melee(3, "Great Axe", "Gathers 4 resources per hit.", "great_axe_1", [140, 140, -8, 25]),
  melee(4, "Short Sword", "Long reach, good damage.", "sword_1", [130, 210, -8, 46]),
  melee(5, "Katana", "Faster and deadlier sword.", "samurai_1", [130, 210, -8, 59]),
  melee(6, "Polearm", "Huge reach, slow swing.", "spear_1", [130, 210, -8, 53]),
  melee(7, "Bat", "Knocks enemies far back.", "bat_1", [110, 180, -8, 53]),
  melee(8, "Daggers", "Very fast stabs, move faster.", "dagger_1", [110, 110, 18, 0], {
    dual: true,
    // One dagger per hand.
    handSprites: VARIANTS.map((v) => mm(`dagger_single${v.suffix}`)),
  }),
  melee(9, "Stick", "Gathers 7 resources per hit.", "stick_1", [140, 140, 3, 24]),
  melee(10, "Great Hammer", "Smashes buildings.", "great_hammer_1", [140, 140, -9, 25]),
  melee(11, "Wooden Shield", "Blocks most damage from the front.", "shield_1", [120, 120, 6, 0], { shield: true }),
  melee(12, "McGrabby", "Steals gold from enemies.", "grab_1", [130, 210, -8, 53]),
  melee(13, "Hunting Bow", "Fires arrows. Costs 4 wood a shot.", "bow_1", [120, 120, -6, 0], {
    ranged: true,
    bullet: mm("arrow_1"),
  }),
  melee(14, "Crossbow", "Stronger arrows. Costs 5 wood a shot.", "crossbow_1", [120, 120, -4, 0], {
    ranged: true,
    bullet: mm("arrow_1"),
  }),
  melee(15, "Repeater Crossbow", "Rapid arrows. Costs 10 wood a shot.", "crossbow_2", [120, 120, -4, 0], {
    ranged: true,
    bullet: mm("arrow_1"),
  }),
  melee(16, "Musket", "Huge damage. Costs 10 stone a shot.", "musket_1", [205, 205, 25, 0], {
    ranged: true,
    bullet: mm("bullet_1"),
  }),
  gun(17, "Pistol", "Balanced sidearm.", "pistol", 52, "bullet_small"),
  gun(18, "Revolver", "Slow but hard-hitting.", "revolver", 56, "bullet_small"),
  gun(19, "Hand Cannon", "Huge pistol with big knockback.", "hand_cannon", 60, "tracer_yellow"),
  gun(20, "Machine Pistol", "Sprays bullets very fast.", "machine_pistol", 58, "bullet_small"),
  gun(21, "SMG", "Fast fire, light damage.", "smg", 84, "bullet_small"),
  gun(22, "Assault Rifle", "Fast fire, solid damage.", "assault_rifle", 100, "tracer_yellow"),
  gun(23, "LMG", "Endless spray. Slows you down.", "lmg", 115, "tracer_yellow"),
  gun(24, "Pump Shotgun", "Fires 5 pellets. Short range.", "pump_shotgun", 105, "tracer_yellow"),
  gun(25, "Double Shotgun", "Fires 8 pellets. Short range.", "double_shotgun", 105, "tracer_yellow"),
  gun(26, "Hunting Rifle", "Long range, heavy hits.", "hunting_rifle", 115, "tracer_white"),
  gun(27, "Sniper", "Extreme range and damage.", "sniper", 120, "tracer_white"),
  gun(28, "Laser Rifle", "Fires fast laser bolts.", "laser_rifle", 118, "laser"),
  melee(29, "Shovel", "Digs big holes in the caves, fast.", "shovel_1", [140, 140, -3, 18]),
];

/** Time between attacks in ms (server/src/catalog/weapons.rs), used to time
 * the swing so back-to-back swings don't snap. */
const RELOAD_MS = { 0: 300, 1: 300, 2: 400, 3: 400, 4: 300, 5: 300, 6: 700, 7: 300, 8: 100, 9: 400, 10: 400, 11: 500, 12: 700, 13: 600, 14: 700, 15: 230, 16: 1500, 29: 350 };

/**
 * Where the two hands hold each weapon, in moomoo.io's frame (player radius
 * 35, +x = aim, +y = right-hand side). moomoo's grip (both hands on the handle
 * across the front, centered on the handle line) fits every swung weapon; crossbows and the
 * musket are held by the stock instead.
 */
const DEFAULT_GRIP = [
  [28, -27],
  [28, 27],
];
const GRIPS = {
  14: [
    [26, -6],
    [44, 6],
  ],
  15: [
    [26, -6],
    [44, 6],
  ],
  16: [
    [22, -6],
    [62, 6],
  ],
};

for (const w of WEAPONS) {
  w.reload = RELOAD_MS[w.id] ?? 300;
  w.grip = GRIPS[w.id] ?? DEFAULT_GRIP;
}

/** Bullet sprite for turret shots (weapon id 255). */
export const TURRET_BULLET = mm("bullet_1");

/** Every image needed to draw weapons and their projectiles. */
export const WEAPON_TEXTURES = [
  ...new Set(
    WEAPONS.flatMap((w) => [w.sprite, w.texture, w.bullet, ...(w.variants || []), ...(w.handSprites || [])]).concat(
      TURRET_BULLET,
    ),
  ),
].filter(Boolean);

/** How a weapon's cost per shot is described. */
export function weaponCost(id) {
  if (id >= 17) return "1 ammo per shot";
  return { 13: "4 wood per shot", 14: "5 wood per shot", 15: "10 wood per shot", 16: "10 stone per shot" }[id] || "";
}
