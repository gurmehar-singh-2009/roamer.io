//! Every weapon in the game. Ids are sent to clients and must match
//! `web/src/data/weapons.js`.

/// Which hand slot a weapon goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Melee weapon. Everyone has one (fists at first).
    Primary,
    /// Ranged or utility weapon, picked at age 5.
    Secondary,
}

/// What firing a ranged weapon costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmmoCost {
    /// Wood per shot (bows).
    Wood(u32),
    /// Stone per shot (musket).
    Stone(u32),
    /// Ammo per shot (guns).
    Ammo(u32),
}

/// A swinging weapon.
#[derive(Clone, Copy, Debug)]
pub struct Melee {
    /// Reach from the player's center to the target's edge.
    pub range:         f32,
    /// Damage to players and animals.
    pub damage:        f32,
    /// Knockback speed given to hit players and animals.
    pub knockback:     f32,
    /// Resources gathered per hit on a tree, stone or bush.
    pub gather:        u32,
    /// Multiplier for damage dealt to buildings.
    pub building_mult: f32,
    /// Fraction of frontal damage that gets through (shields), 1 = none.
    pub block:         f32,
    /// Gold stolen from hit players.
    pub steal_gold:    u32,
    /// Digging strength in the caves (0 = worked out from `gather`).
    pub dig:           u32,
}

/// A weapon that fires projectiles.
#[derive(Clone, Copy, Debug)]
pub struct Ranged {
    /// Damage per projectile.
    pub damage:          f32,
    /// Damage per projectile to buildings.
    pub building_damage: f32,
    /// Knockback per projectile.
    pub knockback:       f32,
    /// Distance a projectile travels per tick.
    pub speed:           f32,
    /// Distance a projectile travels before it disappears.
    pub range:           f32,
    /// Projectiles per shot.
    pub pellets:         u8,
    /// Angle between pellets, in radians.
    pub spread:          f32,
    /// What each shot costs.
    pub cost:            AmmoCost,
}

/// How a weapon attacks.
#[derive(Clone, Copy, Debug)]
pub enum Attack {
    /// Hits everything in an arc in front of the player.
    Melee(Melee),
    /// Fires projectiles.
    Ranged(Ranged),
}

/// A weapon definition.
#[derive(Clone, Copy, Debug)]
pub struct WeaponDef {
    /// Id sent to clients.
    pub id:         u8,
    /// Display name.
    pub name:       &'static str,
    /// Hand slot.
    pub slot:       Slot,
    /// Age at which it's offered as an upgrade.
    pub age:        u32,
    /// Weapons it upgrades from (any one of them). Empty = no requirement.
    pub pre:        &'static [u8],
    /// Time between attacks, in milliseconds.
    pub reload_ms:  u32,
    /// Movement speed multiplier while held.
    pub speed_mult: f32,
    /// How it attacks.
    pub attack:     Attack,
}

/// Fists: the starting weapon.
pub const FISTS: u8 = 0;

/// A melee weapon's stats with the common defaults.
const fn melee(range: f32, damage: f32, knockback: f32, gather: u32) -> Attack {
    Attack::Melee(Melee {
        range,
        damage,
        knockback,
        gather,
        building_mult: 1.0,
        block: 1.0,
        steal_gold: 0,
        dig: 0,
    })
}

/// A gun's stats (costs one ammo per shot).
const fn gun(damage: f32, speed: f32, range: f32, pellets: u8, spread: f32, knockback: f32) -> Attack {
    Attack::Ranged(Ranged {
        damage,
        building_damage: damage * 0.5,
        knockback,
        speed,
        range,
        pellets,
        spread,
        cost: AmmoCost::Ammo(1),
    })
}

/// A bow-like weapon's stats.
const fn bow(damage: f32, speed: f32, range: f32, cost: AmmoCost) -> Attack {
    Attack::Ranged(Ranged {
        damage,
        building_damage: damage * 0.5,
        knockback: 20.0,
        speed,
        range,
        pellets: 1,
        spread: 0.0,
        cost,
    })
}

/// Shorthand for a primary weapon.
const fn primary(
    id: u8,
    name: &'static str,
    age: u32,
    pre: &'static [u8],
    reload_ms: u32,
    speed: f32,
    attack: Attack,
) -> WeaponDef {
    WeaponDef {
        id,
        name,
        slot: Slot::Primary,
        age,
        pre,
        reload_ms,
        speed_mult: speed,
        attack,
    }
}

/// Shorthand for a secondary weapon.
const fn secondary(
    id: u8,
    name: &'static str,
    age: u32,
    pre: &'static [u8],
    reload_ms: u32,
    speed: f32,
    attack: Attack,
) -> WeaponDef {
    WeaponDef {
        id,
        name,
        slot: Slot::Secondary,
        age,
        pre,
        reload_ms,
        speed_mult: speed,
        attack,
    }
}

/// Every weapon, indexed by id.
pub static WEAPONS: [WeaponDef; 30] = [
    // --- moomoo.io melee (primary) ---
    primary(0, "Fists", 1, &[], 300, 1.0, melee(55.0, 15.0, 45.0, 1)),
    primary(1, "Tool Hammer", 2, &[FISTS], 300, 1.0, melee(65.0, 25.0, 30.0, 1)),
    primary(2, "Hand Axe", 2, &[FISTS], 400, 1.0, melee(70.0, 30.0, 30.0, 2)),
    primary(3, "Great Axe", 8, &[2], 400, 1.0, melee(75.0, 35.0, 30.0, 4)),
    primary(4, "Short Sword", 2, &[FISTS], 300, 0.85, melee(110.0, 35.0, 30.0, 1)),
    primary(5, "Katana", 8, &[4], 300, 0.8, melee(118.0, 40.0, 30.0, 1)),
    primary(6, "Polearm", 2, &[FISTS], 700, 0.82, melee(140.0, 45.0, 40.0, 1)),
    primary(7, "Bat", 2, &[FISTS], 300, 1.0, melee(110.0, 20.0, 67.0, 1)),
    primary(8, "Daggers", 2, &[FISTS], 100, 1.13, melee(60.0, 20.0, 15.0, 1)),
    primary(9, "Stick", 2, &[FISTS], 400, 1.0, melee(70.0, 1.0, 20.0, 7)),
    // --- moomoo.io secondaries ---
    secondary(
        10,
        "Great Hammer",
        5,
        &[],
        400,
        0.88,
        Attack::Melee(Melee {
            range:         75.0,
            damage:        10.0,
            knockback:     30.0,
            gather:        1,
            building_mult: 7.5,
            block:         1.0,
            steal_gold:    0,
            dig:           0,
        }),
    ),
    secondary(
        11,
        "Wooden Shield",
        5,
        &[],
        500,
        0.7,
        Attack::Melee(Melee {
            range:         55.0,
            damage:        15.0,
            knockback:     45.0,
            gather:        0,
            building_mult: 1.0,
            block:         0.2,
            steal_gold:    0,
            dig:           0,
        }),
    ),
    secondary(
        12,
        "McGrabby",
        5,
        &[],
        700,
        1.05,
        Attack::Melee(Melee {
            range:         125.0,
            damage:        0.0,
            knockback:     10.0,
            gather:        0,
            building_mult: 0.0,
            block:         1.0,
            steal_gold:    250,
            dig:           0,
        }),
    ),
    secondary(
        13,
        "Hunting Bow",
        5,
        &[],
        600,
        0.75,
        bow(25.0, 100.0, 1000.0, AmmoCost::Wood(4)),
    ),
    secondary(
        14,
        "Crossbow",
        8,
        &[13],
        700,
        0.7,
        bow(35.0, 120.0, 1100.0, AmmoCost::Wood(5)),
    ),
    secondary(
        15,
        "Repeater Crossbow",
        9,
        &[14],
        230,
        0.7,
        bow(30.0, 120.0, 1100.0, AmmoCost::Wood(10)),
    ),
    secondary(
        16,
        "Musket",
        9,
        &[14],
        1500,
        0.6,
        bow(50.0, 160.0, 1400.0, AmmoCost::Stone(10)),
    ),
    // --- foes.io guns ---
    secondary(17, "Pistol", 5, &[], 300, 1.0, gun(20.0, 100.0, 900.0, 1, 0.0, 15.0)),
    secondary(18, "Revolver", 5, &[], 600, 1.0, gun(34.0, 110.0, 1000.0, 1, 0.0, 25.0)),
    secondary(
        19,
        "Hand Cannon",
        8,
        &[17, 18],
        700,
        0.95,
        gun(50.0, 120.0, 1000.0, 1, 0.0, 45.0),
    ),
    secondary(
        20,
        "Machine Pistol",
        8,
        &[17],
        100,
        1.0,
        gun(9.0, 100.0, 800.0, 1, 0.0, 5.0),
    ),
    secondary(21, "SMG", 5, &[], 130, 0.95, gun(11.0, 100.0, 850.0, 1, 0.0, 8.0)),
    secondary(
        22,
        "Assault Rifle",
        8,
        &[21],
        120,
        0.9,
        gun(14.0, 110.0, 1000.0, 1, 0.0, 10.0),
    ),
    secondary(23, "LMG", 9, &[22], 90, 0.8, gun(13.0, 110.0, 1100.0, 1, 0.0, 10.0)),
    secondary(
        24,
        "Pump Shotgun",
        5,
        &[],
        900,
        0.9,
        gun(11.0, 90.0, 500.0, 5, 0.12, 30.0),
    ),
    secondary(
        25,
        "Double Shotgun",
        8,
        &[24],
        1100,
        0.85,
        gun(11.0, 95.0, 550.0, 8, 0.1, 35.0),
    ),
    secondary(
        26,
        "Hunting Rifle",
        5,
        &[],
        1100,
        0.85,
        gun(50.0, 140.0, 1500.0, 1, 0.0, 40.0),
    ),
    secondary(
        27,
        "Sniper",
        8,
        &[26],
        1500,
        0.8,
        gun(70.0, 160.0, 1800.0, 1, 0.0, 50.0),
    ),
    secondary(
        28,
        "Laser Rifle",
        9,
        &[27],
        900,
        0.85,
        gun(60.0, 220.0, 2000.0, 1, 0.0, 20.0),
    ),
    // --- digging ---
    primary(
        29,
        "Shovel",
        2,
        &[FISTS],
        350,
        1.0,
        Attack::Melee(Melee {
            range:         65.0,
            damage:        20.0,
            knockback:     25.0,
            gather:        1,
            building_mult: 1.0,
            block:         1.0,
            steal_gold:    0,
            dig:           12,
        }),
    ),
];

/// Looks up a weapon by id.
pub fn weapon(id: u8) -> Option<&'static WeaponDef> {
    WEAPONS.get(usize::from(id))
}

/// Looks up a weapon by id, falling back to fists for unknown ids.
pub fn weapon_or_fists(id: u8) -> &'static WeaponDef {
    weapon(id).unwrap_or(&WEAPONS[0])
}

/// Weapon variant: gold, diamond, ruby and emerald, earned by using a weapon.
/// Index 0 is the plain weapon.
#[derive(Clone, Copy, Debug)]
pub struct VariantDef {
    /// Weapon XP needed.
    pub xp:          u32,
    /// Damage multiplier.
    pub damage_mult: f32,
    /// Hits poison the target.
    pub poison:      bool,
    /// Fraction of damage dealt healed back.
    pub lifesteal:   f32,
}

/// All variants, plain first.
pub static VARIANTS: [VariantDef; 5] = [
    VariantDef {
        xp:          0,
        damage_mult: 1.0,
        poison:      false,
        lifesteal:   0.0,
    },
    VariantDef {
        xp:          300,
        damage_mult: 1.1,
        poison:      false,
        lifesteal:   0.0,
    },
    VariantDef {
        xp:          800,
        damage_mult: 1.18,
        poison:      false,
        lifesteal:   0.0,
    },
    VariantDef {
        xp:          1600,
        damage_mult: 1.18,
        poison:      true,
        lifesteal:   0.0,
    },
    VariantDef {
        xp:          3200,
        damage_mult: 1.25,
        poison:      true,
        lifesteal:   0.2,
    },
];

/// The variant index reached with this much weapon XP.
pub fn variant_for_xp(xp: u32) -> u8 {
    let reached = VARIANTS.iter().rposition(|v| xp >= v.xp).unwrap_or(0);
    u8::try_from(reached).unwrap_or(0)
}

/// XP needed for the next variant, or `None` at the last one.
pub fn next_variant_xp(xp: u32) -> Option<u32> {
    VARIANTS.iter().map(|v| v.xp).find(|&need| need > xp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_positions() {
        for (i, w) in WEAPONS.iter().enumerate() {
            assert_eq!(usize::from(w.id), i, "{} has the wrong id", w.name);
        }
    }

    #[test]
    fn prerequisites_exist_and_come_earlier() {
        for w in &WEAPONS {
            for &p in w.pre {
                let pre = weapon(p).unwrap();
                assert!(pre.age < w.age, "{} upgrades from a later weapon", w.name);
                assert_eq!(pre.slot, w.slot, "{} upgrades across slots", w.name);
            }
        }
    }

    #[test]
    fn variants() {
        assert_eq!(variant_for_xp(0), 0);
        assert_eq!(variant_for_xp(299), 0);
        assert_eq!(variant_for_xp(300), 1);
        assert_eq!(variant_for_xp(10_000), 4);
        assert_eq!(next_variant_xp(300), Some(800));
        assert_eq!(next_variant_xp(5000), None);
    }
}
