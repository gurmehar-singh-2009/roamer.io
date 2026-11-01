// Hats, accessories and skins. Must match server/src/catalog/hats.rs and
// server/src/systems/accounts.rs.

const hatArt = import.meta.glob("../assets/moomoo/hats/hat_*.png", {
  eager: true,
  import: "default",
});
const accArt = import.meta.glob("../assets/moomoo/accessories/access_*.png", {
  eager: true,
  import: "default",
});

const hat = (id, name, price, desc = "Cosmetic.") => ({
  id,
  name,
  price,
  desc,
  texture: hatArt[`../assets/moomoo/hats/hat_${id}.png`],
});
const acc = (id, name, price) => ({
  id,
  name,
  price,
  desc: "Cosmetic.",
  texture: accArt[`../assets/moomoo/accessories/access_${id}.png`],
});

export const HATS = [
  hat(6, "Soldier Helmet", 4000, "Take 25% less damage. Slightly slower."),
  hat(7, "Bull Helmet", 6000, "Deal 50% more damage, but lose 5 health a second."),
  hat(9, "Miner Helmet", 1000, "Gather 1 extra resource per hit."),
  hat(10, "Bush Gear", 3000, "Blend in with the bushes."),
  hat(11, "Spike Gear", 10000, "Reflect 45% of melee damage."),
  hat(12, "Booster Hat", 4000, "Move 16% faster."),
  hat(13, "Medic Gear", 5000, "Heal 3 health a second."),
  hat(14, "Windmill Hat", 2000, "Make 1 gold a second."),
  hat(15, "Winter Cap", 600, "No sliding on the snow."),
  hat(20, "Samurai Armor", 12000, "Attack 22% faster."),
  hat(21, "Plague Mask", 4000, "Melee hits poison."),
  hat(26, "Barbarian Armor", 3000, "Deal 15% more damage. Slower."),
  hat(31, "Flipper Hat", 2500, "No slowdown in water."),
  hat(40, "Tank Gear", 15000, "3.3x building damage. Very slow."),
  hat(43, "EMP Helmet", 6000, "Turrets ignore you."),
  hat(53, "Turret Gear", 10000, "Shoots nearby enemies. Slower."),
  hat(58, "Bloodthirster", 12000, "Heal 25% of damage dealt."),
  hat(1, "Marksman Cap", 500),
  hat(2, "Straw Hat", 200),
  hat(4, "Bandana", 200),
  hat(5, "Cowboy Hat", 300),
  hat(8, "Fiddler Hat", 300),
  hat(18, "Explorer Hat", 400),
  hat(22, "Party Hat", 400),
  hat(23, "Dark Hat", 500),
  hat(27, "Jester Hat", 600),
  hat(28, "Moo Head", 800),
  hat(29, "Pig Head", 800),
  hat(30, "Fluff Head", 800),
  hat(32, "Penguin", 900),
  hat(35, "Snail Shell", 700),
  hat(36, "Panda Head", 1000),
  hat(37, "Monkey Head", 1000),
  hat(38, "Bear Head", 1000),
  hat(42, "Cool Beard", 1500),
  hat(44, "Polar Head", 1200),
  hat(45, "Tribal Headdress", 2000),
  hat(46, "Bull Mask", 1500),
  hat(48, "Halo", 2500),
  hat(49, "Elf Hat", 900),
  hat(50, "Ninja Hood", 1500),
  hat(55, "Mud Hat", 300),
  hat(56, "Bat Hat", 1200),
  hat(57, "Pumpkin", 1200),
];

export const ACCESSORIES = [
  acc(1, "Snowball", 300),
  acc(2, "Tree Cape", 500),
  acc(3, "Stone Cape", 500),
  acc(4, "Cookie Cape", 700),
  acc(5, "Cow Cape", 700),
  acc(6, "Monkey Tail", 900),
  acc(7, "Apple Basket", 800),
  acc(8, "Winter Cape", 900),
  acc(9, "Skull Cape", 1500),
  acc(10, "Dash Cape", 1500),
  acc(11, "Dragon Cape", 3000),
  acc(12, "Super Cape", 2000),
  acc(13, "Troll Cape", 1000),
  acc(14, "Thorns", 1500),
  acc(15, "Blockades", 1500),
  acc(16, "Devils Tail", 2000),
  acc(17, "Sawblade", 2500),
  acc(18, "Angel Wings", 3000),
  acc(19, "Shadow Wings", 3500),
  acc(20, "Blood Wings", 4000),
  acc(21, "Corrupt X Wings", 5000),
];

/** moomoo.io skin colors. 0-2 are free, the rest cost coins. */
export const SKINS = [
  0xbf8f54, 0xcbb091, 0x896c4b, 0xfadadc, 0xececec, 0xc37373, 0x4c4c4c, 0xecaff7, 0x738cc3, 0x8bc373,
];
export const SKIN_PRICE = 50;

export const hatById = (id) => HATS.find((h) => h.id === id);
export const accessoryById = (id) => ACCESSORIES.find((a) => a.id === id);

export const SHOP_TEXTURES = [...HATS, ...ACCESSORIES].map((h) => h.texture).filter(Boolean);
