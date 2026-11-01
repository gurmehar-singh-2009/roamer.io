//! Weapons and their stats.

use bevy_ecs::component::Component;

/// Level at which players pick their first melee upgrade (age 2).
pub const MELEE_UPGRADE_LEVEL: u32 = 1;
/// Level at which players pick a gun (age 5). Ages start at 1, levels at 0.
pub const GUN_UPGRADE_LEVEL: u32 = 4;

/// A weapon a player can hold. The discriminant is the id sent to clients.
#[derive(Component, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Weapon {
    /// Bare hands, every player starts with these.
    Fists        = 0,
    /// Slow, strong against buildings.
    Hammer       = 1,
    /// Long range.
    Sword        = 2,
    /// Gathers resources fast.
    Hoe          = 3,
    /// Short range, high damage.
    Knife        = 4,
    /// Gun: balanced.
    Pistol       = 5,
    /// Gun: fast fire rate, low damage per shot.
    AssaultRifle = 6,
    /// Gun: several pellets, short range.
    Shotgun      = 7,
    /// Gun: slow, long range, high damage.
    Sniper       = 8,
}

impl Weapon {
    /// The weapon with this id, if there is one.
    pub const fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            0 => Self::Fists,
            1 => Self::Hammer,
            2 => Self::Sword,
            3 => Self::Hoe,
            4 => Self::Knife,
            5 => Self::Pistol,
            6 => Self::AssaultRifle,
            7 => Self::Shotgun,
            8 => Self::Sniper,
            _ => return None,
        })
    }

    /// Whether this weapon shoots bullets instead of swinging.
    pub const fn is_gun(self) -> bool {
        gun_stats(self).is_some()
    }
}

impl From<Weapon> for u8 {
    fn from(val: Weapon) -> Self {
        val as Self
    }
}

/// How a gun fires.
#[derive(Clone, Copy, Debug)]
pub struct GunStats {
    /// Damage per bullet to players.
    pub damage:          f32,
    /// Damage per bullet to buildings.
    pub building_damage: f32,
    /// Knockback per bullet.
    pub knockback:       f32,
    /// Distance a bullet travels per tick.
    pub speed:           f32,
    /// Distance a bullet can travel before disappearing.
    pub range:           f32,
    /// Bullets per shot.
    pub pellets:         u8,
    /// Angle between pellets, in radians.
    pub spread:          f32,
}

/// Bullet stats for guns, `None` for melee weapons.
pub const fn gun_stats(weapon: Weapon) -> Option<GunStats> {
    Some(match weapon {
        Weapon::Pistol => GunStats {
            damage:          20.,
            building_damage: 10.,
            knockback:       100.,
            speed:           100.,
            range:           900.,
            pellets:         1,
            spread:          0.,
        },
        Weapon::AssaultRifle => GunStats {
            damage:          14.,
            building_damage: 8.,
            knockback:       60.,
            speed:           110.,
            range:           1000.,
            pellets:         1,
            spread:          0.,
        },
        Weapon::Shotgun => GunStats {
            damage:          12.,
            building_damage: 10.,
            knockback:       150.,
            speed:           90.,
            range:           500.,
            pellets:         5,
            spread:          0.12,
        },
        Weapon::Sniper => GunStats {
            damage:          70.,
            building_damage: 30.,
            knockback:       300.,
            speed:           160.,
            range:           1800.,
            pellets:         1,
            spread:          0.,
        },
        Weapon::Fists | Weapon::Hammer | Weapon::Sword | Weapon::Hoe | Weapon::Knife => return None,
    })
}

/// Extra reach of a melee weapon past the player's collider.
pub const fn get_weapon_range(weapon: Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 35.,
        Weapon::Hammer => 50.,
        Weapon::Sword => 80.,
        Weapon::Hoe => 40.,
        Weapon::Knife => 20.,
        Weapon::Pistol | Weapon::AssaultRifle | Weapon::Shotgun | Weapon::Sniper => 0.,
    }
}

/// Damage a weapon does to players and animals.
pub const fn get_weapon_damage(weapon: Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 15.,
        Weapon::Hammer => 25.,
        Weapon::Sword => 40.,
        Weapon::Hoe => 5.,
        Weapon::Knife => 50.,
        Weapon::Pistol | Weapon::AssaultRifle | Weapon::Shotgun | Weapon::Sniper => match gun_stats(weapon) {
            Some(stats) => stats.damage,
            None => 0.,
        },
    }
}

/// Damage a weapon does to buildings. The hammer is the building breaker.
pub const fn get_weapon_building_damage(weapon: Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 10.,
        Weapon::Hammer => 75.,
        Weapon::Sword => 25.,
        Weapon::Hoe => 5.,
        Weapon::Knife => 15.,
        Weapon::Pistol | Weapon::AssaultRifle | Weapon::Shotgun | Weapon::Sniper => match gun_stats(weapon) {
            Some(stats) => stats.building_damage,
            None => 0.,
        },
    }
}

/// Time between swings or shots, in milliseconds.
pub const fn get_weapon_attack_speed(weapon: Weapon) -> u32 {
    match weapon {
        Weapon::Fists | Weapon::Sword => 400,
        Weapon::Hammer => 750,
        Weapon::Hoe => 100,
        Weapon::Knife => 500,
        Weapon::Pistol => 300,
        Weapon::AssaultRifle => 120,
        Weapon::Shotgun => 900,
        Weapon::Sniper => 1500,
    }
}

/// Resources gathered per hit. Guns don't gather.
pub const fn get_weapon_resource_multiplier(weapon: Weapon) -> u32 {
    match weapon {
        Weapon::Fists | Weapon::Hammer | Weapon::Sword => 1,
        Weapon::Hoe => 5,
        Weapon::Knife | Weapon::Pistol | Weapon::AssaultRifle | Weapon::Shotgun | Weapon::Sniper => 0,
    }
}

/// Knockback applied to hit players and animals.
pub const fn get_weapon_knockback(weapon: Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 600.,
        Weapon::Hammer => 300.,
        Weapon::Knife => 130.,
        Weapon::Sword => 400.,
        Weapon::Hoe => 375.,
        Weapon::Pistol | Weapon::AssaultRifle | Weapon::Shotgun | Weapon::Sniper => match gun_stats(weapon) {
            Some(stats) => stats.knockback,
            None => 0.,
        },
    }
}

/// Weapons a player can choose from when reaching `level`.
pub fn weapon_upgrade_hierachy(level: u32) -> Vec<Weapon> {
    match level {
        0 => vec![Weapon::Fists],
        MELEE_UPGRADE_LEVEL => vec![Weapon::Hammer, Weapon::Sword, Weapon::Hoe, Weapon::Knife],
        GUN_UPGRADE_LEVEL => vec![Weapon::Pistol, Weapon::AssaultRifle, Weapon::Shotgun, Weapon::Sniper],
        _ => vec![],
    }
}
