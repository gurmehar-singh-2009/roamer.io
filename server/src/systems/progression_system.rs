//! XP, ages, upgrades, weapon switching and weapon variants.

use bevy_ecs::{entity::Entity, world::World};

use crate::{
    catalog::{
        apply_upgrade,
        weapons::{variant_for_xp, weapon_or_fists, Attack, Slot},
        xp_for_age, MAX_UPGRADE_AGE,
    },
    structs::{
        components::{Dirty, Gear, Loadout, PlayerEntity, Progress, ReloadState, Resources, Weapon, WeaponXp},
        outbox::Outbox,
    },
    systems::social_system::{quest_progress, QuestKind},
};

/// Ammo given with the first gun.
pub const STARTING_AMMO: u32 = 60;

/// Variant names, plain first.
pub const VARIANT_NAMES: [&str; 5] = ["", "Gold", "Diamond", "Ruby", "Emerald"];

/// Gives a player XP, aging them up as needed.
pub fn grant_xp(world: &mut World, entity: Entity, amount: u32) {
    let Some(mut progress) = world.get_mut::<Progress>(entity) else {
        return;
    };
    let old_age = progress.age;
    progress.xp += amount;
    while progress.xp >= xp_for_age(progress.age) {
        progress.xp -= xp_for_age(progress.age);
        progress.age += 1;
    }
    let new_age = progress.age;
    world.resource_mut::<Dirty>().age.insert(entity);

    if new_age > old_age {
        if let Some(id) = world.get::<PlayerEntity>(entity).map(|p| p.0) {
            let text = if new_age <= MAX_UPGRADE_AGE {
                format!("Age {new_age}! Pick an upgrade.")
            } else {
                format!("Age {new_age}!")
            };
            world.resource_mut::<Outbox>().notice(id, text, 2);
        }
    }
}

/// Gives XP to the weapon a player used. Announces new variants.
pub fn grant_weapon_xp(world: &mut World, entity: Entity, weapon: u8, amount: u32) {
    if amount == 0 {
        return;
    }
    let Some(mut wxp) = world.get_mut::<WeaponXp>(entity) else {
        return;
    };
    let entry = wxp.0.entry(weapon).or_default();
    let before = variant_for_xp(*entry);
    *entry += amount;
    let after = variant_for_xp(*entry);
    world.resource_mut::<Dirty>().loadout.insert(entity);

    if after > before {
        if let Some(id) = world.get::<PlayerEntity>(entity).map(|p| p.0) {
            let name = weapon_or_fists(weapon).name;
            let variant = VARIANT_NAMES[usize::from(after)];
            world
                .resource_mut::<Outbox>()
                .notice(id, format!("{name} upgraded to {variant}!"), 2);
        }
    }
}

/// Picks one of the upgrades offered for the player's next pending age.
pub fn choose_upgrade(world: &mut World, entity: Entity, is_item: bool, id: u8) -> bool {
    let (Some(mut progress), Some(mut loadout)) = (
        world.get::<Progress>(entity).copied(),
        world.get::<Loadout>(entity).cloned(),
    ) else {
        return false;
    };
    if progress.upgrade_age > progress.age.min(MAX_UPGRADE_AGE) {
        return false;
    }
    if !apply_upgrade(progress.upgrade_age, &mut loadout.0, is_item, id) {
        return false;
    }
    progress.upgrade_age += 1;

    // Skip ages that have nothing left to offer.
    while progress.upgrade_age <= progress.age.min(MAX_UPGRADE_AGE) {
        let (w, i) = crate::catalog::upgrade_options(progress.upgrade_age, &loadout.0);
        if !w.is_empty() || !i.is_empty() {
            break;
        }
        progress.upgrade_age += 1;
    }

    let first_gun = !is_item
        && weapon_or_fists(id).slot == Slot::Secondary
        && matches!(weapon_or_fists(id).attack, Attack::Ranged(r) if matches!(r.cost, crate::catalog::weapons::AmmoCost::Ammo(_)))
        && world.get::<Loadout>(entity).is_some_and(|l| l.0.secondary.is_none());

    world.entity_mut(entity).insert((progress, loadout));
    if !is_item {
        switch_weapon(world, entity, id);
    }
    if first_gun {
        if let Some(mut res) = world.get_mut::<Resources>(entity) {
            res.ammo += STARTING_AMMO;
        }
        world.resource_mut::<Dirty>().resources.insert(entity);
    }

    let mut dirty = world.resource_mut::<Dirty>();
    dirty.loadout.insert(entity);
    dirty.age.insert(entity);
    true
}

/// Switches to an owned weapon. Keeps the remaining cooldown so switching
/// can't skip it.
pub fn switch_weapon(world: &mut World, entity: Entity, weapon: u8) -> bool {
    if !world.get::<Loadout>(entity).is_some_and(|l| l.0.has_weapon(weapon)) {
        return false;
    }
    let hat = world.get::<Gear>(entity).map_or(0, |g| g.hat);
    let reload = reload_ms(weapon, hat);
    world.entity_mut(entity).insert(Weapon(weapon));
    if let Some(mut r) = world.get_mut::<ReloadState>(entity) {
        *r = ReloadState(r.0, reload);
    }
    world.resource_mut::<Dirty>().loadout.insert(entity);
    true
}

/// Reload time of a weapon with a hat's reload multiplier.
pub fn reload_ms(weapon: u8, hat: u8) -> u32 {
    let mult = crate::catalog::hats::hat_effects(hat).reload;
    // Reload times are a few thousand ms at most.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ms = (f64::from(weapon_or_fists(weapon).reload_ms) * f64::from(mult)).round() as u32;
    ms
}

/// Records resources gathered for quests.
pub fn note_gathered(world: &mut World, entity: Entity, amount: u32) {
    quest_progress(world, entity, QuestKind::Gather, amount);
}
