// Every item. Ids must match server/src/catalog/items.rs.
// `size` is the image size relative to the collider radius (from the moomoo
// SVG viewBoxes), so sprites line up with what the server collides with.

const art = import.meta.glob("../assets/items/*.png", {
  eager: true,
  import: "default",
});
const icon = (id) => art[`../assets/items/item_${id}.png`];

/** Collider radius of each building (server/src/catalog/items.rs). */
const RADIUS = { 3: 36, 4: 36, 5: 38, 6: 34, 7: 36, 8: 36, 9: 36, 10: 36, 11: 38, 12: 38, 13: 52, 14: 44, 15: 40, 16: 36, 17: 34, 18: 34, 19: 36, 20: 36, 21: 24, 22: 28, 23: 32, 24: 24, 25: 32 };

/** item(id, name, description, [wood, stone, food], size, extras) */
function item(id, name, desc, [wood, stone, food], size, extra = {}) {
  return { id, name, desc, wood, stone, food, size, radius: RADIUS[id] ?? 0, texture: icon(id), ...extra };
}

export const ITEMS = [
  item(0, "Apple", "Heals 20 health.", [0, 0, 10], 2.44, { food: true }),
  item(1, "Cookie", "Heals 40 health.", [0, 0, 15], 2.18, { food: true }),
  item(2, "Cheese", "Heals 30 health.", [0, 0, 25], 2.18, { food: true }),
  item(3, "Wood Wall", "Blocks the way.", [10, 0, 0], 2.388),
  item(4, "Stone Wall", "Sturdier wall.", [0, 25, 0], 2.388),
  item(5, "Castle Wall", "The sturdiest wall.", [0, 35, 0], 2.388),
  item(6, "Spikes", "Hurts enemies that run into it.", [20, 5, 0], 2.108),
  item(7, "Greater Spikes", "Hurts more.", [30, 10, 0], 2.108),
  item(8, "Poison Spikes", "Hurts and poisons.", [35, 15, 0], 2.108),
  item(9, "Spinning Spikes", "Hurts a lot.", [30, 20, 0], 2.108, { spin: 2.5 }),
  item(10, "Windmill", "Makes 1 gold a second.", [50, 10, 0], 3.18, { spin: 1.2 }),
  item(11, "Faster Windmill", "Makes 1.5 gold a second.", [60, 20, 0], 3.18, { spin: 1.8 }),
  item(12, "Power Mill", "Makes 2 gold a second.", [100, 50, 0], 3.18, { spin: 2.4 }),
  item(13, "Mine", "Makes 1 stone a second.", [20, 100, 0], 2.18),
  item(14, "Sapling", "Makes 1 wood a second.", [150, 0, 0], 2.18),
  item(15, "Pit Trap", "Hidden. Traps enemies.", [30, 30, 0], 2.38),
  item(16, "Boost Pad", "Launches players forward.", [5, 20, 0], 2.18),
  item(17, "Turret", "Shoots nearby enemies.", [200, 150, 0], 2.52),
  item(18, "Platform", "Shoot over buildings from it.", [20, 0, 0], 2.18),
  item(19, "Healing Pad", "Heals you and your clan.", [30, 0, 10], 2.18),
  item(20, "Spawn Pad", "You respawn here.", [100, 100, 0], 2.18),
  item(21, "Blocker", "Enemies can't build nearby.", [30, 25, 0], 2.18),
  item(22, "Teleporter", "Teleports you somewhere random.", [60, 60, 0], 2.18),
  item(23, "Chest", "Keeps resources safe when you die.", [50, 20, 0], 2.4),
  item(24, "TNT", "Explodes 3 seconds after placing.", [20, 40, 0], 2.6),
  item(25, "Tunnel", "Digs a way into the caves. Stand on it and press X.", [30, 30, 0], 2.4),
];

/** Text like "10 wood, 5 stone". */
export function costText(item) {
  return [
    item.wood && `${item.wood} wood`,
    item.stone && `${item.stone} stone`,
    item.food && `${item.food} food`,
  ]
    .filter(Boolean)
    .join(", ");
}

export const ITEM_TEXTURES = ITEMS.map((i) => i.texture);
