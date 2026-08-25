use bevy_ecs::component::Component;

#[derive(Component, Copy, Clone, Debug)]
pub enum Weapon {
    Fists  = 0,
    Hammer = 1,
    Sword  = 2,
    Hoe    = 3,
    Knife  = 4,
}

impl From<u8> for Weapon {
    fn from(value: u8) -> Self {
        match value {
            0 => Weapon::Fists,
            1 => Weapon::Hammer,
            2 => Weapon::Sword,
            3 => Weapon::Hoe,
            4 => Weapon::Knife,
            _ => Weapon::Fists,
        }
    }
}

impl From<Weapon> for u8 {
    fn from(val: Weapon) -> Self {
        match val {
            Weapon::Fists => 0,
            Weapon::Hammer => 1,
            Weapon::Sword => 2,
            Weapon::Hoe => 3,
            Weapon::Knife => 4,
        }
    }
}

pub fn get_weapon_range(weapon: &Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 35.,
        Weapon::Hammer => 50.,
        Weapon::Sword => 80.,
        Weapon::Hoe => 40.,
        Weapon::Knife => 20.,
    }
}

#[allow(dead_code)]
pub fn get_weapon_damage(weapon: &Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 15.,
        Weapon::Hammer => 25.,
        Weapon::Sword => 40.,
        Weapon::Hoe => 5.,
        Weapon::Knife => 50.,
    }
}

#[allow(dead_code)]
pub fn get_weapon_attack_speed(weapon: &Weapon) -> u32 {
    match weapon {
        Weapon::Fists => 400,
        Weapon::Hammer => 750,
        Weapon::Sword => 400,
        Weapon::Hoe => 100,
        Weapon::Knife => 500,
    }
}

pub fn get_weapon_resource_multiplier(weapon: &Weapon) -> u32 {
    match weapon {
        Weapon::Fists => 1,
        Weapon::Hammer => 1,
        Weapon::Sword => 1,
        Weapon::Hoe => 5,
        Weapon::Knife => 0,
    }
}

pub fn get_weapon_knockback(weapon: &Weapon) -> f32 {
    match weapon {
        Weapon::Fists => 600.,
        Weapon::Hammer => 300.,
        Weapon::Knife => 130.,
        Weapon::Sword => 400.,
        Weapon::Hoe => 375.,
    }
}

pub fn weapon_upgrade_hierachy(level: u32) -> Vec<Weapon> {
    match level {
        0 => vec![Weapon::Fists],
        1 => vec![Weapon::Hammer, Weapon::Sword, Weapon::Hoe, Weapon::Knife],

        _ => vec![],
    }
}
