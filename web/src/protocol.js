// Packet opcodes. Must match `PacketType` in shared/src/lib.rs.

/** Client -> server. */
export const C = {
  SPAWN: 1,
  MOVE: 2,
  AIM: 5,
  ATTACK: 6,
  CHAT: 12,
  CHOOSE_WEAPON: 15,
  USE_ITEM: 17,
  CHOOSE_UPGRADE: 31,
  SHOP: 32,
  CLAN: 33,
  CHEST: 34,
  TRADE: 35,
  ARENA: 36,
  CAVE: 37,
};

/** Server -> client. */
export const S = {
  ADD_PLAYER: 101,
  REMOVE_PLAYER: 102,
  UPDATE_PLAYERS: 103,
  ANIMALS: 104,
  HIT_EVENT: 106,
  ADD_OBJECT: 107,
  UPDATE_HEALTH: 108,
  LOADOUT: 109,
  OBJECT_HIT: 110,
  RESOURCES: 111,
  DEATH: 112,
  CHAT: 113,
  UPGRADES: 114,
  LEADERBOARD: 116,
  OBJECT_HEALTH: 119,
  REMOVE_OBJECT: 120,
  ADD_PROJECTILE: 121,
  REMOVE_PROJECTILE: 122,
  AGE: 123,
  NOTICE: 124,
  EXPLOSION: 125,
  CLAN_STATE: 126,
  SHOP_STATE: 127,
  QUEST: 128,
  WORLD_EVENT: 129,
  CHEST_STATE: 130,
  MINIMAP: 131,
  SPAWNED: 132,
  ARENA_STATE: 133,
  CAVE_CELLS: 134,
};

/** Object type ids (see `ObjectKind::type_id` on the server). */
export const OBJ = {
  TREE: 0,
  STONE: 1,
  BUSH: 2,
  GOLD: 3,
  CAVE: 4, // cave entrance
  CRATE: 10, // + tier (0-4)
  ITEM: 50, // + item id
};
