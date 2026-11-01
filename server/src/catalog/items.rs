//! Every placeable/usable item. Ids 0-22 follow moomoo.io's item order (and the
//! SVGs in `web/src/assets/s/`). Ids are sent to clients and must match
//! `web/src/data/items.js`.

/// What an item does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemKind {
    /// Eaten to heal.
    Food {
        /// Health restored.
        heal: f32,
    },
    /// Blocks movement.
    Wall,
    /// Hurts enemies that run into it.
    Spike {
        /// Damage at full speed.
        damage: f32,
        /// Also poisons.
        poison: bool,
    },
    /// Produces gold (score) for its owner.
    Windmill {
        /// Gold per second.
        gold_per_sec: f32,
    },
    /// Produces stone for its owner.
    Mine,
    /// Produces wood for its owner.
    Sapling,
    /// Hidden from enemies; traps whoever walks onto it.
    PitTrap,
    /// Launches players in its direction.
    BoostPad,
    /// Shoots at nearby enemies.
    Turret,
    /// Stand on it to shoot over buildings.
    Platform,
    /// Heals players standing on it.
    HealPad,
    /// Its owner respawns on it.
    SpawnPad,
    /// Enemies can't build near it.
    Blocker,
    /// Teleports whoever touches it to a random spot.
    Teleporter,
    /// Stores resources that survive death.
    Chest,
    /// Explodes a few seconds after being placed.
    Tnt,
    /// A way down into the caves.
    Tunnel,
}

/// Item groups. A player owns at most one item per group; upgrades replace the
/// item in the same group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Apple, cookie, cheese.
    Food,
    /// Walls.
    Wall,
    /// Spikes.
    Spike,
    /// Windmills.
    Mill,
    /// Mine or sapling.
    Resource,
    /// Pit trap or boost pad.
    Trap,
    /// Turret, platform, healing pad, blocker or teleporter.
    Special,
    /// Spawn pad.
    Spawn,
    /// Chest.
    Storage,
    /// TNT.
    Explosive,
    /// Tunnel.
    Digging,
}

/// An item definition.
#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    /// Id sent to clients.
    pub id:     u8,
    /// Display name.
    pub name:   &'static str,
    /// Group.
    pub group:  Group,
    /// Age at which it's offered (1 = everyone has it).
    pub age:    u32,
    /// Item it upgrades from.
    pub pre:    Option<u8>,
    /// Wood cost.
    pub wood:   u32,
    /// Stone cost.
    pub stone:  u32,
    /// Food cost.
    pub food:   u32,
    /// Building health (unused for food).
    pub health: f32,
    /// Collider radius.
    pub radius: f32,
    /// How many one player can have placed.
    pub limit:  usize,
    /// What it does.
    pub kind:   ItemKind,
}

impl ItemDef {
    /// Whether players walk over it instead of bumping into it.
    pub const fn walkable(&self) -> bool {
        matches!(
            self.kind,
            ItemKind::PitTrap
                | ItemKind::BoostPad
                | ItemKind::Platform
                | ItemKind::HealPad
                | ItemKind::SpawnPad
                | ItemKind::Teleporter
                | ItemKind::Tunnel
        )
    }
}

/// Shorthand for an item definition.
#[allow(clippy::too_many_arguments)]
const fn item(
    id: u8,
    name: &'static str,
    group: Group,
    age: u32,
    pre: Option<u8>,
    (wood, stone, food): (u32, u32, u32),
    (health, radius, limit): (f32, f32, usize),
    kind: ItemKind,
) -> ItemDef {
    ItemDef {
        id,
        name,
        group,
        age,
        pre,
        wood,
        stone,
        food,
        health,
        radius,
        limit,
        kind,
    }
}

/// Every item, indexed by id.
pub static ITEMS: [ItemDef; 26] = [
    item(
        0,
        "Apple",
        Group::Food,
        1,
        None,
        (0, 0, 10),
        (0.0, 0.0, 0),
        ItemKind::Food { heal: 20.0 },
    ),
    item(
        1,
        "Cookie",
        Group::Food,
        3,
        Some(0),
        (0, 0, 15),
        (0.0, 0.0, 0),
        ItemKind::Food { heal: 40.0 },
    ),
    item(
        2,
        "Cheese",
        Group::Food,
        7,
        Some(1),
        (0, 0, 25),
        (0.0, 0.0, 0),
        ItemKind::Food { heal: 30.0 },
    ),
    item(
        3,
        "Wood Wall",
        Group::Wall,
        1,
        None,
        (10, 0, 0),
        (380.0, 36.0, 30),
        ItemKind::Wall,
    ),
    item(
        4,
        "Stone Wall",
        Group::Wall,
        3,
        Some(3),
        (0, 25, 0),
        (900.0, 36.0, 30),
        ItemKind::Wall,
    ),
    item(
        5,
        "Castle Wall",
        Group::Wall,
        7,
        Some(4),
        (0, 35, 0),
        (1500.0, 38.0, 30),
        ItemKind::Wall,
    ),
    item(
        6,
        "Spikes",
        Group::Spike,
        1,
        None,
        (20, 5, 0),
        (400.0, 34.0, 15),
        ItemKind::Spike {
            damage: 20.0,
            poison: false,
        },
    ),
    item(
        7,
        "Greater Spikes",
        Group::Spike,
        6,
        Some(6),
        (30, 10, 0),
        (500.0, 36.0, 15),
        ItemKind::Spike {
            damage: 35.0,
            poison: false,
        },
    ),
    item(
        8,
        "Poison Spikes",
        Group::Spike,
        9,
        Some(7),
        (35, 15, 0),
        (600.0, 36.0, 15),
        ItemKind::Spike {
            damage: 30.0,
            poison: true,
        },
    ),
    item(
        9,
        "Spinning Spikes",
        Group::Spike,
        9,
        Some(7),
        (30, 20, 0),
        (500.0, 36.0, 15),
        ItemKind::Spike {
            damage: 45.0,
            poison: false,
        },
    ),
    item(
        10,
        "Windmill",
        Group::Mill,
        1,
        None,
        (50, 10, 0),
        (400.0, 36.0, 7),
        ItemKind::Windmill { gold_per_sec: 1.0 },
    ),
    item(
        11,
        "Faster Windmill",
        Group::Mill,
        6,
        Some(10),
        (60, 20, 0),
        (500.0, 38.0, 7),
        ItemKind::Windmill { gold_per_sec: 1.5 },
    ),
    item(
        12,
        "Power Mill",
        Group::Mill,
        8,
        Some(11),
        (100, 50, 0),
        (800.0, 38.0, 7),
        ItemKind::Windmill { gold_per_sec: 2.0 },
    ),
    item(
        13,
        "Mine",
        Group::Resource,
        6,
        None,
        (20, 100, 0),
        (4000.0, 52.0, 1),
        ItemKind::Mine,
    ),
    item(
        14,
        "Sapling",
        Group::Resource,
        6,
        None,
        (150, 0, 0),
        (4000.0, 44.0, 2),
        ItemKind::Sapling,
    ),
    item(
        15,
        "Pit Trap",
        Group::Trap,
        4,
        None,
        (30, 30, 0),
        (500.0, 40.0, 6),
        ItemKind::PitTrap,
    ),
    item(
        16,
        "Boost Pad",
        Group::Trap,
        4,
        None,
        (5, 20, 0),
        (150.0, 36.0, 12),
        ItemKind::BoostPad,
    ),
    item(
        17,
        "Turret",
        Group::Special,
        7,
        None,
        (200, 150, 0),
        (800.0, 34.0, 2),
        ItemKind::Turret,
    ),
    item(
        18,
        "Platform",
        Group::Special,
        7,
        None,
        (20, 0, 0),
        (300.0, 34.0, 12),
        ItemKind::Platform,
    ),
    item(
        19,
        "Healing Pad",
        Group::Special,
        7,
        None,
        (30, 0, 10),
        (400.0, 36.0, 4),
        ItemKind::HealPad,
    ),
    item(
        20,
        "Spawn Pad",
        Group::Spawn,
        9,
        None,
        (100, 100, 0),
        (400.0, 36.0, 1),
        ItemKind::SpawnPad,
    ),
    item(
        21,
        "Blocker",
        Group::Special,
        7,
        None,
        (30, 25, 0),
        (400.0, 24.0, 3),
        ItemKind::Blocker,
    ),
    item(
        22,
        "Teleporter",
        Group::Special,
        7,
        None,
        (60, 60, 0),
        (200.0, 28.0, 2),
        ItemKind::Teleporter,
    ),
    item(
        23,
        "Chest",
        Group::Storage,
        1,
        None,
        (50, 20, 0),
        (600.0, 32.0, 1),
        ItemKind::Chest,
    ),
    item(
        24,
        "TNT",
        Group::Explosive,
        10,
        None,
        (20, 40, 0),
        (50.0, 24.0, 3),
        ItemKind::Tnt,
    ),
    item(
        25,
        "Tunnel",
        Group::Digging,
        1,
        None,
        (30, 30, 0),
        (300.0, 32.0, 4),
        ItemKind::Tunnel,
    ),
];

/// Looks up an item by id.
pub fn item_def(id: u8) -> Option<&'static ItemDef> {
    ITEMS.get(usize::from(id))
}

/// Items every player starts with.
pub fn starting_items() -> Vec<u8> {
    ITEMS.iter().filter(|i| i.age == 1).map(|i| i.id).collect()
}

/// Radius around a blocker where enemies can't build.
pub const BLOCKER_RADIUS: f32 = 300.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_positions() {
        for (i, item) in ITEMS.iter().enumerate() {
            assert_eq!(usize::from(item.id), i, "{} has the wrong id", item.name);
        }
    }

    #[test]
    fn upgrades_stay_in_their_group() {
        for item in &ITEMS {
            if let Some(pre) = item.pre {
                let pre = item_def(pre).unwrap();
                assert_eq!(pre.group, item.group, "{}", item.name);
                assert!(pre.age < item.age, "{}", item.name);
            }
        }
    }
}
