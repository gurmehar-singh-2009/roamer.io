/**
 * World draw order, back to front (world.sortableChildren = true).
 *
 * Flat things sit on the ground; rocks, bushes and buildings stand on it;
 * animals and players walk among them; tree canopies hang over everyone;
 * shots and explosions fly above; bars, names and chat are always readable.
 * Underground, the cave layer covers the surface (which is also hidden).
 */
export const Z = {
  BOUNDS: 0, // biome colors
  GROUND: 1, // ground decoration
  FOOTPRINTS: 2,
  FLAT_OBJECTS: 3, // cave entrances
  FLAT_BUILDINGS: 4, // pads, traps, platforms, tunnels
  CAVE: 5, // cave rock and floor (only while underground)
  OBJECTS: 10, // stones, bushes, gold, crates
  BUILDINGS: 11, // walls, spikes, mills, turrets, chests...
  ANIMALS: 20,
  PLAYERS: 22, // one container per player (backpack, arms, weapon, head)
  TREES: 25, // canopies cover players, like moomoo.io
  LEAVES: 26, // leaf particles
  PROJECTILES: 27,
  EFFECTS: 28, // explosions
  HEALTHBAR: 40,
  NAME: 41,
  CHAT: 50,
};

/** Global UI layer order (ui.sortableChildren = true). */
export const UI_Z = {
  OVERLAY: 0, // fullscreen tint
  LEADERBOARD: 10,
  RESOURCES: 11,
  CHAT: 20, // chat log + input
  TOAST: 25, // transient messages
  ACTIONBAR: 30, // weapon hotbar
  UPGRADE: 40, // upgrade selection
  TOOLTIP: 50, // upgrade tooltips
};
