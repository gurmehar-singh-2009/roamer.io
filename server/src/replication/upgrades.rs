//! Per-player state messages: resources, loadout, age, upgrades, shop.

use bevy_ecs::{entity::Entity, world::World};
use shared::{
    to_client::{AgeData, LoadoutData, SetResourceData, ShopData, UpgradesData, WeaponProgressTO},
    PacketType,
};

use crate::{
    catalog::{upgrade_options, weapons::next_variant_xp, xp_for_age, MAX_UPGRADE_AGE},
    structs::{
        components::{Account, Dirty, Gear, Loadout, PlayerEntity, Progress, Resources, Weapon, WeaponXp},
        outbox::{Outbox, Target},
    },
    systems::accounts::Accounts,
};

/// The player id of a player entity.
fn id_of(world: &World, entity: Entity) -> Option<u32> {
    world.get::<PlayerEntity>(entity).map(|p| p.0)
}

/// Sends a player their resources.
pub fn send_resources(world: &mut World, entity: Entity) {
    let (Some(id), Some(res)) = (id_of(world, entity), world.get::<Resources>(entity).copied()) else {
        return;
    };
    world.resource_mut::<Outbox>().send(
        Target::Player(id),
        PacketType::SetResource,
        &SetResourceData {
            wood:  res.wood,
            stone: res.stone,
            food:  res.food,
            gold:  res.gold,
            ammo:  res.ammo,
        },
    );
}

/// Sends a player their weapons, items and weapon variant progress.
pub fn send_loadout(world: &mut World, entity: Entity) {
    let Some(id) = id_of(world, entity) else {
        return;
    };
    let (Some(loadout), Some(weapon), Some(wxp)) = (
        world.get::<Loadout>(entity).cloned(),
        world.get::<Weapon>(entity).copied(),
        world.get::<WeaponXp>(entity).cloned(),
    ) else {
        return;
    };
    let progress = loadout
        .0
        .weapons()
        .into_iter()
        .map(|w| {
            let xp = wxp.0.get(&w).copied().unwrap_or(0);
            WeaponProgressTO {
                weapon: w,
                variant: wxp.variant(w),
                xp,
                next: next_variant_xp(xp).unwrap_or(0),
            }
        })
        .collect();
    world.resource_mut::<Outbox>().send_reliable(
        Target::Player(id),
        PacketType::Loadout,
        &LoadoutData {
            primary: loadout.0.primary,
            secondary: loadout.0.secondary,
            items: loadout.0.items,
            selected: weapon.0,
            progress,
        },
    );
}

/// Sends a player their age and XP.
pub fn send_age(world: &mut World, entity: Entity) {
    let (Some(id), Some(p)) = (id_of(world, entity), world.get::<Progress>(entity).copied()) else {
        return;
    };
    world.resource_mut::<Outbox>().send(
        Target::Player(id),
        PacketType::Age,
        &AgeData {
            age:    p.age,
            xp:     p.xp,
            max_xp: xp_for_age(p.age),
        },
    );
}

/// Sends a player the upgrades they can pick now (or an empty list).
pub fn send_upgrades(world: &mut World, entity: Entity) {
    let (Some(id), Some(p), Some(loadout)) = (
        id_of(world, entity),
        world.get::<Progress>(entity).copied(),
        world.get::<Loadout>(entity).cloned(),
    ) else {
        return;
    };
    let reached = p.age.min(MAX_UPGRADE_AGE);
    let data = if p.upgrade_age <= reached {
        let (weapons, items) = upgrade_options(p.upgrade_age, &loadout.0);
        UpgradesData {
            age: p.upgrade_age,
            weapons,
            items,
            pending: reached - p.upgrade_age,
        }
    } else {
        UpgradesData::default()
    };
    world
        .resource_mut::<Outbox>()
        .send_reliable(Target::Player(id), PacketType::Upgrades, &data);
}

/// Sends a player what they own in the shop.
pub fn send_shop(world: &mut World, entity: Entity) {
    let (Some(id), Some(gear)) = (id_of(world, entity), world.get::<Gear>(entity).cloned()) else {
        return;
    };
    let account = world
        .get::<Account>(entity)
        .and_then(|a| world.resource::<Accounts>().get(&a.0).cloned())
        .unwrap_or_default();
    world.resource_mut::<Outbox>().send_reliable(
        Target::Player(id),
        PacketType::ShopState,
        &ShopData {
            hats:        gear.hats,
            accessories: gear.accessories,
            hat:         gear.hat,
            accessory:   gear.accessory,
            skins:       account.owned_skins(),
            skin:        gear.skin,
            coins:       account.coins,
        },
    );
}

/// Sends a player all of their state.
pub fn send_all(world: &mut World, entity: Entity) {
    send_resources(world, entity);
    send_loadout(world, entity);
    send_age(world, entity);
    send_upgrades(world, entity);
    send_shop(world, entity);
}

/// Sends fresh state to players whose state changed this tick.
pub fn flush_dirty(world: &mut World) {
    let dirty = std::mem::take(&mut *world.resource_mut::<Dirty>());
    for e in dirty.resources {
        send_resources(world, e);
    }
    for e in dirty.loadout {
        send_loadout(world, e);
    }
    for e in dirty.age {
        send_age(world, e);
        send_upgrades(world, e);
    }
}
