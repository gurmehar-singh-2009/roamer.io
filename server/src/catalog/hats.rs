//! Hats (bought with gold, most give stats) and accessories (cosmetic). Ids
//! are moomoo.io's image ids and must match `web/src/data/shop.js`.

/// Stat changes from a hat. Multipliers are 1 and additions 0 when unused.
// A table of independent flags, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug)]
pub struct HatEffects {
    /// Damage taken multiplier.
    pub damage_taken:    f32,
    /// Weapon damage dealt multiplier.
    pub damage_dealt:    f32,
    /// Movement speed multiplier.
    pub speed:           f32,
    /// Reload time multiplier.
    pub reload:          f32,
    /// Building damage multiplier.
    pub building_damage: f32,
    /// Health regenerated per second.
    pub regen:           f32,
    /// Health lost per second (bull helmet).
    pub drain:           f32,
    /// Fraction of melee damage reflected to the attacker.
    pub reflect:         f32,
    /// Extra resources per gather hit.
    pub gather_bonus:    u32,
    /// Melee hits poison.
    pub poison:          bool,
    /// Fraction of damage dealt healed back.
    pub lifesteal:       f32,
    /// Turrets ignore the wearer.
    pub turret_immune:   bool,
    /// No slowdown on snow.
    pub snow_immune:     bool,
    /// No slowdown in water.
    pub water_immune:    bool,
    /// Gold per second.
    pub gold_per_sec:    f32,
    /// Shoots at nearby enemies like a small turret.
    pub turret:          bool,
}

/// No effects.
const NONE: HatEffects = HatEffects {
    damage_taken:    1.0,
    damage_dealt:    1.0,
    speed:           1.0,
    reload:          1.0,
    building_damage: 1.0,
    regen:           0.0,
    drain:           0.0,
    reflect:         0.0,
    gather_bonus:    0,
    poison:          false,
    lifesteal:       0.0,
    turret_immune:   false,
    snow_immune:     false,
    water_immune:    false,
    gold_per_sec:    0.0,
    turret:          false,
};

/// A hat or accessory definition.
#[derive(Clone, Copy, Debug)]
pub struct HatDef {
    /// Image id.
    pub id:      u8,
    /// Display name.
    pub name:    &'static str,
    /// Gold price.
    pub price:   u32,
    /// Stat changes.
    pub effects: HatEffects,
}

/// Shorthand for a cosmetic hat.
const fn cosmetic(id: u8, name: &'static str, price: u32) -> HatDef {
    HatDef {
        id,
        name,
        price,
        effects: NONE,
    }
}

/// Every hat.
pub static HATS: [HatDef; 44] = [
    HatDef {
        id:      6,
        name:    "Soldier Helmet",
        price:   4000,
        effects: HatEffects {
            damage_taken: 0.75,
            speed: 0.94,
            ..NONE
        },
    },
    HatDef {
        id:      7,
        name:    "Bull Helmet",
        price:   6000,
        effects: HatEffects {
            damage_dealt: 1.5,
            drain: 5.0,
            speed: 0.96,
            ..NONE
        },
    },
    HatDef {
        id:      9,
        name:    "Miner Helmet",
        price:   1000,
        effects: HatEffects {
            gather_bonus: 1,
            ..NONE
        },
    },
    HatDef {
        id:      10,
        name:    "Bush Gear",
        price:   3000,
        effects: NONE,
    },
    HatDef {
        id:      11,
        name:    "Spike Gear",
        price:   10000,
        effects: HatEffects { reflect: 0.45, ..NONE },
    },
    HatDef {
        id:      12,
        name:    "Booster Hat",
        price:   4000,
        effects: HatEffects { speed: 1.16, ..NONE },
    },
    HatDef {
        id:      13,
        name:    "Medic Gear",
        price:   5000,
        effects: HatEffects { regen: 3.0, ..NONE },
    },
    HatDef {
        id:      14,
        name:    "Windmill Hat",
        price:   2000,
        effects: HatEffects {
            gold_per_sec: 1.0,
            ..NONE
        },
    },
    HatDef {
        id:      15,
        name:    "Winter Cap",
        price:   600,
        effects: HatEffects {
            snow_immune: true,
            ..NONE
        },
    },
    HatDef {
        id:      20,
        name:    "Samurai Armor",
        price:   12000,
        effects: HatEffects { reload: 0.78, ..NONE },
    },
    HatDef {
        id:      21,
        name:    "Plague Mask",
        price:   4000,
        effects: HatEffects { poison: true, ..NONE },
    },
    HatDef {
        id:      26,
        name:    "Barbarian Armor",
        price:   3000,
        effects: HatEffects {
            damage_dealt: 1.15,
            speed: 0.9,
            ..NONE
        },
    },
    HatDef {
        id:      31,
        name:    "Flipper Hat",
        price:   2500,
        effects: HatEffects {
            water_immune: true,
            ..NONE
        },
    },
    HatDef {
        id:      40,
        name:    "Tank Gear",
        price:   15000,
        effects: HatEffects {
            building_damage: 3.3,
            speed: 0.3,
            ..NONE
        },
    },
    HatDef {
        id:      43,
        name:    "EMP Helmet",
        price:   6000,
        effects: HatEffects {
            turret_immune: true,
            ..NONE
        },
    },
    HatDef {
        id:      53,
        name:    "Turret Gear",
        price:   10000,
        effects: HatEffects {
            turret: true,
            speed: 0.7,
            ..NONE
        },
    },
    HatDef {
        id:      58,
        name:    "Bloodthirster",
        price:   12000,
        effects: HatEffects {
            lifesteal: 0.25,
            damage_dealt: 1.12,
            ..NONE
        },
    },
    cosmetic(1, "Marksman Cap", 500),
    cosmetic(2, "Straw Hat", 200),
    cosmetic(4, "Bandana", 200),
    cosmetic(5, "Cowboy Hat", 300),
    cosmetic(8, "Fiddler Hat", 300),
    cosmetic(18, "Explorer Hat", 400),
    cosmetic(22, "Party Hat", 400),
    cosmetic(23, "Dark Hat", 500),
    cosmetic(27, "Jester Hat", 600),
    cosmetic(28, "Moo Head", 800),
    cosmetic(29, "Pig Head", 800),
    cosmetic(30, "Fluff Head", 800),
    cosmetic(32, "Penguin", 900),
    cosmetic(35, "Snail Shell", 700),
    cosmetic(36, "Panda Head", 1000),
    cosmetic(37, "Monkey Head", 1000),
    cosmetic(38, "Bear Head", 1000),
    cosmetic(42, "Cool Beard", 1500),
    cosmetic(44, "Polar Head", 1200),
    cosmetic(45, "Tribal Headdress", 2000),
    cosmetic(46, "Bull Mask", 1500),
    cosmetic(48, "Halo", 2500),
    cosmetic(49, "Elf Hat", 900),
    cosmetic(50, "Ninja Hood", 1500),
    cosmetic(55, "Mud Hat", 300),
    cosmetic(56, "Bat Hat", 1200),
    cosmetic(57, "Pumpkin", 1200),
];

/// Every accessory (all cosmetic).
pub static ACCESSORIES: [HatDef; 21] = [
    cosmetic(1, "Snowball", 300),
    cosmetic(2, "Tree Cape", 500),
    cosmetic(3, "Stone Cape", 500),
    cosmetic(4, "Cookie Cape", 700),
    cosmetic(5, "Cow Cape", 700),
    cosmetic(6, "Monkey Tail", 900),
    cosmetic(7, "Apple Basket", 800),
    cosmetic(8, "Winter Cape", 900),
    cosmetic(9, "Skull Cape", 1500),
    cosmetic(10, "Dash Cape", 1500),
    cosmetic(11, "Dragon Cape", 3000),
    cosmetic(12, "Super Cape", 2000),
    cosmetic(13, "Troll Cape", 1000),
    cosmetic(14, "Thorns", 1500),
    cosmetic(15, "Blockades", 1500),
    cosmetic(16, "Devils Tail", 2000),
    cosmetic(17, "Sawblade", 2500),
    cosmetic(18, "Angel Wings", 3000),
    cosmetic(19, "Shadow Wings", 3500),
    cosmetic(20, "Blood Wings", 4000),
    cosmetic(21, "Corrupt X Wings", 5000),
];

/// Looks up a hat by id.
pub fn hat(id: u8) -> Option<&'static HatDef> {
    HATS.iter().find(|h| h.id == id)
}

/// Looks up an accessory by id.
pub fn accessory(id: u8) -> Option<&'static HatDef> {
    ACCESSORIES.iter().find(|a| a.id == id)
}

/// The effects of the hat with this id (none for 0 or unknown ids).
pub fn hat_effects(id: u8) -> HatEffects {
    hat(id).map_or(NONE, |h| h.effects)
}
