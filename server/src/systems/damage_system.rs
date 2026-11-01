//! Applies queued damage to players, animals, crates and buildings.

use std::collections::HashSet;

use bevy_ecs::{entity::Entity, world::World};
use shared::{
    to_client::{RemoveObjectData, UpdateObjectHealthData},
    PacketType,
};

use crate::{
    catalog::{
        animals::{animal, Behavior},
        hats::hat_effects,
        items::{item_def, ItemKind},
        weapons::{weapon_or_fists, Attack},
    },
    replication::health::send_health,
    structs::{
        bevy::PlayerMap,
        components::{
            AiState, AimDir, AnimalEntity, Building, Damage, DamageEvents, DamageSource, Deaths, Dirty, Gear, Health,
            Name, ObjectKind, PlayerEntity, PlayerStats, Poisoned, Position, Resources, Stagger, Velocity, Weapon,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        attack_system::angle_diff,
        building_system::{explode, REGEN_DELAY_MS},
        progression_system::{grant_weapon_xp, grant_xp, VARIANT_NAMES},
        social_system::{are_allies, quest_progress, QuestKind},
    },
};

/// Poison damage per second.
const POISON_DPS: f32 = 5.0;
/// Poison duration.
const POISON_MS: u32 = 5000;
/// Rounds of follow-up damage (reflection, chain explosions) per tick.
const MAX_ROUNDS: usize = 4;

/// The entity of a living player.
pub fn player_entity(world: &World, id: u32) -> Option<Entity> {
    world.resource::<PlayerMap>().map.get(&id).copied()
}

/// Applies every queued damage event.
pub fn damage_system(world: &mut World) {
    let mut dead = HashSet::new();
    for _ in 0..MAX_ROUNDS {
        let events = std::mem::take(&mut world.resource_mut::<DamageEvents>().0);
        if events.is_empty() {
            break;
        }
        for mut d in events {
            if dead.contains(&d.target) || world.get_entity(d.target).is_err() {
                continue;
            }
            // Arena rules.
            if let Some((object_damage, bounciness)) = d
                .source
                .player
                .and_then(|p| crate::systems::arena_system::arena_rules(world, p))
            {
                d.building_amount *= object_damage;
                d.knockback *= bounciness;
            }
            if let Some(id) = world.get::<PlayerEntity>(d.target).map(|p| p.0) {
                damage_player(world, id, &d, &mut dead);
            } else if let Some(kind) = world.get::<AnimalEntity>(d.target).map(|a| a.0) {
                damage_animal(world, kind, &d, &mut dead);
            } else if let Some(kind) = world.get::<ObjectKind>(d.target).copied() {
                damage_object(world, kind, &d, &mut dead);
            }
        }
    }
}

/// Unit vector from the source to the target (or zero).
fn push_dir(world: &World, target: Entity, from: (f32, f32)) -> (f32, f32) {
    let Some(pos) = world.get::<Position>(target) else {
        return (0.0, 0.0);
    };
    let (dx, dy) = (pos.0 - from.0, pos.1 - from.1);
    let len = dx.hypot(dy);
    if len > 0.0 {
        (dx / len, dy / len)
    } else {
        (0.0, 0.0)
    }
}

/// Knocks the target away from the source.
fn knock_back(world: &mut World, target: Entity, d: &Damage) {
    if d.knockback <= 0.0 {
        return;
    }
    let dir = push_dir(world, target, d.source.pos);
    if let Some(mut vel) = world.get_mut::<Velocity>(target) {
        vel.0 = dir.0.mul_add(d.knockback, vel.0);
        vel.1 = dir.1.mul_add(d.knockback, vel.1);
    }
    // Players hit by animals lose control for a moment, so they fly back.
    let by_animal = d.source.entity.is_some_and(|e| world.get::<AnimalEntity>(e).is_some());
    if by_animal && world.get::<PlayerEntity>(target).is_some() {
        world.entity_mut(target).insert(Stagger(STAGGER_MS));
    }
}

/// How long a player hit by an animal can't steer.
const STAGGER_MS: u32 = 250;

/// Weapon XP and lifesteal for whoever dealt the damage.
fn reward_attacker(world: &mut World, d: &Damage, dealt: f32) {
    let Some(attacker) = d.source.player.and_then(|id| player_entity(world, id)) else {
        return;
    };
    if let Some(weapon) = d.weapon {
        // Damage is at most a few hundred per hit.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let xp = (dealt / 4.0).round() as u32;
        grant_weapon_xp(world, attacker, weapon, xp);
    }
    if d.lifesteal > 0.0 && dealt > 0.0 {
        if let Some(mut health) = world.get_mut::<Health>(attacker) {
            health.0 = d.lifesteal.mul_add(dealt, health.0).min(health.1);
        }
    }
}

/// Damage to a player.
fn damage_player(world: &mut World, id: u32, d: &Damage, dead: &mut HashSet<Entity>) {
    let target = d.target;
    if let Some(src) = d.source.player {
        if src == id || are_allies(world, src, id) {
            return;
        }
    }

    let mut amount = d.amount;

    // Shields block hits from the front.
    let held = world.get::<Weapon>(target).map_or(0, |w| w.0);
    if let Attack::Melee(m) = weapon_or_fists(held).attack {
        if m.block < 1.0 {
            let pos = world.get::<Position>(target).copied().unwrap_or(Position(0.0, 0.0));
            let aim = world.get::<AimDir>(target).map_or(0.0, |a| a.0);
            let to_source = (d.source.pos.1 - pos.1).atan2(d.source.pos.0 - pos.0);
            if angle_diff(aim, to_source).abs() < std::f32::consts::FRAC_PI_2 {
                amount *= m.block;
            }
        }
    }

    let hat = hat_effects(world.get::<Gear>(target).map_or(0, |g| g.hat));
    amount *= hat.damage_taken;

    // Who gets the kill.
    let attacker_name = d
        .source
        .player
        .and_then(|src| player_entity(world, src))
        .and_then(|e| world.get::<Name>(e).map(|n| n.0.clone()));
    if let (Some(src), Some(name)) = (d.source.player, attacker_name) {
        if let Some(mut stats) = world.get_mut::<PlayerStats>(target) {
            stats.last_hit = Some((src, name));
        }
    }

    let Some(mut health) = world.get_mut::<Health>(target) else {
        return;
    };
    health.0 = (health.0 - amount).max(0.0);
    let (hp, max) = (health.0, health.1);

    knock_back(world, target, d);

    if d.poison {
        world.entity_mut(target).insert(Poisoned {
            dps:          POISON_DPS,
            remaining_ms: POISON_MS,
            source:       d.source.player,
        });
    }

    // McGrabby steals gold.
    if d.steal_gold > 0 {
        let stolen = world.get::<Resources>(target).map_or(0, |r| r.gold.min(d.steal_gold));
        if stolen > 0 {
            if let Some(mut r) = world.get_mut::<Resources>(target) {
                r.gold -= stolen;
            }
            world.resource_mut::<Dirty>().resources.insert(target);
            if let Some(thief) = d.source.player.and_then(|src| player_entity(world, src)) {
                if let Some(mut r) = world.get_mut::<Resources>(thief) {
                    r.gold += stolen;
                }
                world.resource_mut::<Dirty>().resources.insert(thief);
            }
        }
    }

    // Spike gear reflects melee damage.
    if hat.reflect > 0.0 && d.source.melee && amount > 0.0 {
        if let Some(attacker) = d.source.entity {
            let pos = world.get::<Position>(target).map_or((0.0, 0.0), |p| (p.0, p.1));
            world.resource_mut::<DamageEvents>().0.push(Damage {
                target: attacker,
                amount: amount * hat.reflect,
                source: DamageSource {
                    entity: Some(target),
                    player: Some(id),
                    pos,
                    ..DamageSource::default()
                },
                ..Damage::default()
            });
        }
    }

    reward_attacker(world, d, amount);

    let pos = world.get::<Position>(target).map_or((0.0, 0.0), |p| (p.0, p.1));
    send_health(&mut world.resource_mut::<Outbox>(), id, pos, hp, max);

    if hp <= 0.0 {
        dead.insert(target);
        let killer = world.get::<PlayerStats>(target).and_then(|s| s.last_hit.clone());
        world.resource_mut::<Deaths>().0.push((id, killer));
    }
}

/// Damage to an animal.
fn damage_animal(world: &mut World, kind: u8, d: &Damage, dead: &mut HashSet<Entity>) {
    let target = d.target;
    let Some(def) = animal(kind) else {
        return;
    };
    let Some(mut health) = world.get_mut::<Health>(target) else {
        return;
    };
    health.0 = (health.0 - d.amount).max(0.0);
    let hp = health.0;

    knock_back(world, target, d);

    // React: passive animals run, others fight back.
    let reaction = match (def.behavior, d.source.entity) {
        (Behavior::Passive, _) => Some(AiState::Flee {
            from: d.source.pos,
            ms:   3000,
        }),
        (Behavior::Neutral | Behavior::Hostile, Some(attacker)) if d.source.player.is_some() => {
            Some(AiState::Chase(attacker))
        }
        _ => None,
    };
    if let Some(state) = reaction {
        world.entity_mut(target).insert(state);
    }

    reward_attacker(world, d, d.amount);

    if hp > 0.0 {
        return;
    }
    dead.insert(target);
    let pos = world.get::<Position>(target).map_or((0.0, 0.0), |p| (p.0, p.1));
    world.despawn(target);

    let Some(src) = d.source.player else {
        return;
    };
    let Some(killer) = player_entity(world, src) else {
        return;
    };
    if let Some(mut r) = world.get_mut::<Resources>(killer) {
        r.food += def.food;
        r.gold += def.gold;
    }
    world.resource_mut::<Dirty>().resources.insert(killer);
    grant_xp(world, killer, def.xp);
    quest_progress(world, killer, QuestKind::KillAnimals, 1);

    if def.boss {
        let name = world.get::<Name>(killer).map_or_default(|n| n.0.clone());
        let mut out = world.resource_mut::<Outbox>();
        out.send(
            Target::All,
            PacketType::Notice,
            &shared::to_client::NoticeData {
                text: format!("{} was defeated by {name}!", def.name),
                kind: 2,
            },
        );
        let _ = pos;
    }
}

/// Crate loot for each tier: (ammo, wood/stone/food, gold).
const fn crate_loot(tier: u8) -> (u32, u32, u32) {
    let t = tier as u32;
    (15 + 10 * t, 20 * (t + 1), 25 * t * t)
}

/// Damage to a crate or building.
fn damage_object(world: &mut World, kind: ObjectKind, d: &Damage, dead: &mut HashSet<Entity>) {
    let target = d.target;
    match kind {
        ObjectKind::Crate(tier) => {
            if d.source.variant < tier {
                if let Some(src) = d.source.player {
                    let needed = VARIANT_NAMES[usize::from(tier)];
                    world.resource_mut::<Outbox>().notice(
                        src,
                        format!("This crate needs a {needed} weapon (or better)"),
                        1,
                    );
                }
                return;
            }
        }
        ObjectKind::Building(_) => {
            let owner = world.get::<Building>(target).map(|b| b.owner);
            if let (Some(owner), Some(src)) = (owner, d.source.player) {
                if owner != src && are_allies(world, owner, src) {
                    return;
                }
            }
        }
        ObjectKind::Tree | ObjectKind::Stone | ObjectKind::Bush | ObjectKind::Gold | ObjectKind::CaveEntrance => {
            return;
        }
    }

    let Some(mut health) = world.get_mut::<Health>(target) else {
        return;
    };
    if health.0 <= 0.0 || d.building_amount <= 0.0 {
        return;
    }
    health.0 = (health.0 - d.building_amount).max(0.0);
    let (hp, max) = (health.0, health.1);
    if let Some(mut b) = world.get_mut::<Building>(target) {
        b.regen_wait_ms = REGEN_DELAY_MS;
    }

    // Building damage trains the weapon less than damage to players.
    if let (Some(weapon), Some(attacker)) = (d.weapon, d.source.player.and_then(|s| player_entity(world, s))) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let xp = (d.building_amount / 8.0).round() as u32;
        grant_weapon_xp(world, attacker, weapon, xp);
    }

    let index = target.index();
    if hp > 0.0 {
        world.resource_mut::<Outbox>().send(
            Target::KnowsObject(index),
            PacketType::UpdateObjectHealth,
            &UpdateObjectHealthData {
                id:         index,
                health:     hp,
                max_health: max,
            },
        );
        return;
    }

    dead.insert(target);
    let pos = world.get::<Position>(target).map_or((0.0, 0.0), |p| (p.0, p.1));
    let building = world.get::<Building>(target).copied();
    world.resource_mut::<Outbox>().send_reliable(
        Target::ForgetObject(index),
        PacketType::RemoveObject,
        &RemoveObjectData { id: index },
    );
    world.despawn(target);

    if let ObjectKind::Crate(tier) = kind {
        open_crate(world, tier, d.source.player);
    }
    if let Some(b) = building {
        if item_def(b.item).is_some_and(|i| i.kind == ItemKind::Tnt) {
            explode(world, pos, b.owner);
        }
    }
}

/// Gives crate loot to whoever broke it.
fn open_crate(world: &mut World, tier: u8, opener: Option<u32>) {
    let Some(entity) = opener.and_then(|id| player_entity(world, id)) else {
        return;
    };
    let (ammo, res, gold) = crate_loot(tier);
    if let Some(mut r) = world.get_mut::<Resources>(entity) {
        r.ammo += ammo;
        r.wood += res;
        r.stone += res;
        r.food += res;
        r.gold += gold;
    }
    world.resource_mut::<Dirty>().resources.insert(entity);
    grant_xp(world, entity, 40 + 40 * u32::from(tier));
    quest_progress(world, entity, QuestKind::OpenCrates, 1);
    if let Some(id) = opener {
        world.resource_mut::<Outbox>().notice(
            id,
            format!("Crate opened: +{ammo} ammo, +{res} wood/stone/food, +{gold} gold"),
            2,
        );
    }
}
