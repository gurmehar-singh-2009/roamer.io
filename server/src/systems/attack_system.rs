//! Player attacks (melee swings and shots), turrets and turret gear.

use std::f32::consts::PI;

use bevy_ecs::{
    entity::Entity,
    query::{Has, With},
    world::World,
};
use shared::{to_client::ObjectHitAnimData, PacketType};

use crate::{
    catalog::{
        hats::hat_effects,
        items::{item_def, ItemKind},
        weapons::{weapon_or_fists, AmmoCost, Attack, Melee, Ranged, VARIANTS},
    },
    structs::{
        components::{
            AimDir, AnimalEntity, AttackAnims, AttackState, Building, Damage, DamageEvents, DamageSource, Dirty, Gear,
            ObjectEntity, ObjectKind, PlayerEntity, Position, ReloadState, Resources, Weapon, WeaponXp,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        cave_system::{dig, is_underground, Underground},
        progression_system::{grant_weapon_xp, grant_xp, note_gathered},
        social_system::{are_allies, gather_multiplier},
        weapon_system::{spawn_projectile, ProjectileSpec, TURRET_WEAPON},
        Collider, TICK_MS,
    },
};

/// Half-width of the melee hit arc.
const MELEE_ARC: f32 = PI / 2.6;
/// XP per resource gathered.
const XP_PER_RESOURCE: u32 = 3;
/// Turret range.
const TURRET_RANGE: f32 = 700.0;
/// Turret reload.
const TURRET_RELOAD_MS: u32 = 2200;
/// Turret gear (hat) reload.
const TURRET_GEAR_RELOAD_MS: u32 = 2500;

/// Signed difference between two angles, in (-PI, PI].
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    if d > PI {
        d - std::f32::consts::TAU
    } else {
        d
    }
}

/// A player who can attack this tick.
struct Attacker {
    /// Entity.
    entity:  Entity,
    /// Player id.
    id:      u32,
    /// Position.
    pos:     Position,
    /// Aim.
    aim:     f32,
    /// Held weapon.
    weapon:  u8,
    /// Its variant.
    variant: u8,
    /// Worn hat.
    hat:     u8,
}

/// Runs every player's attack: ticks reload timers and swings or shoots.
pub fn attack_system(world: &mut World) {
    let mut ready = Vec::new();
    for (entity, player, pos, aim, weapon, mut reload, attack, wxp, gear) in world
        .query::<(
            Entity,
            &PlayerEntity,
            &Position,
            &AimDir,
            &Weapon,
            &mut ReloadState,
            &AttackState,
            &WeaponXp,
            &Gear,
        )>()
        .iter_mut(world)
    {
        if reload.0 > 0 {
            reload.0 = reload.0.saturating_sub(TICK_MS);
            continue;
        }
        if !attack.0 {
            continue;
        }
        reload.0 = reload.1;
        ready.push(Attacker {
            entity,
            id: player.0,
            pos: *pos,
            aim: aim.0,
            weapon: weapon.0,
            variant: wxp.variant(weapon.0),
            hat: gear.hat,
        });
    }

    for a in ready {
        match weapon_or_fists(a.weapon).attack {
            Attack::Melee(m) => swing(world, &a, &m),
            Attack::Ranged(r) => shoot(world, &a, &r),
        }
        crate::systems::anticheat_system::note_attack(world, a.entity);
    }

    turrets(world);
}

/// A melee swing: gathers from natural objects and damages everything else
/// in the arc.
#[allow(
    clippy::too_many_lines,
    reason = "one swing, step by step: victims, objects, digging, gathering"
)]
fn swing(world: &mut World, a: &Attacker, m: &Melee) {
    world.resource_mut::<AttackAnims>().0.push(a.id);
    let hat = hat_effects(a.hat);
    let variant = VARIANTS[usize::from(a.variant)];
    let damage = m.damage * variant.damage_mult * hat.damage_dealt;
    let underground = is_underground(world, a.entity);

    let in_arc = |pos: &Position, radius: f32| {
        let (dx, dy) = (pos.0 - a.pos.0, pos.1 - a.pos.1);
        let dist = dx.hypot(dy);
        dist - radius <= m.range && angle_diff(a.aim, dy.atan2(dx)).abs() <= MELEE_ARC
    };

    // Players and animals.
    let victims: Vec<Entity> = world
        .query::<(
            Entity,
            &Position,
            &Collider,
            Option<&PlayerEntity>,
            Option<&AnimalEntity>,
            Has<Underground>,
        )>()
        .iter(world)
        .filter(|(e, pos, col, p, an, under)| {
            // Only things on the same layer (animals live on the surface).
            *e != a.entity && (p.is_some() || an.is_some()) && *under == underground && in_arc(pos, col.rad)
        })
        .map(|(e, ..)| e)
        .collect();

    // Objects are on the surface; underground, the swing digs instead.
    let objects: Vec<(Entity, ObjectKind, Position)> = if underground {
        dig(world, a.entity, a.pos, a.aim, m, a.weapon);
        Vec::new()
    } else {
        world
            .query_filtered::<(Entity, &ObjectKind, &Position, &Collider), With<ObjectEntity>>()
            .iter(world)
            .filter(|(_, kind, pos, col)| **kind != ObjectKind::CaveEntrance && in_arc(pos, col.rad))
            .map(|(e, k, p, _)| (e, *k, *p))
            .collect()
    };

    let source = DamageSource {
        entity:  Some(a.entity),
        player:  Some(a.id),
        pos:     (a.pos.0, a.pos.1),
        variant: a.variant,
        melee:   true,
    };

    let mut events = Vec::new();
    for target in victims {
        events.push(Damage {
            target,
            amount: damage,
            building_amount: 0.0,
            knockback: m.knockback,
            poison: variant.poison || hat.poison,
            lifesteal: variant.lifesteal + hat.lifesteal,
            weapon: Some(a.weapon),
            steal_gold: m.steal_gold,
            source: source.clone(),
        });
    }

    // Each object hit gives its own resource (a bush and a stone in one
    // swing give food and stone).
    let mut gain = Resources::default();
    for (entity, kind, pos) in objects {
        let hit_dir = (a.pos.1 - pos.1).atan2(a.pos.0 - pos.0);
        world.resource_mut::<Outbox>().send(
            Target::Near(pos.0, pos.1),
            PacketType::ObjectHitAnim,
            &ObjectHitAnimData {
                id:  entity.index(),
                dir: hit_dir,
            },
        );
        match kind {
            ObjectKind::Tree | ObjectKind::Stone | ObjectKind::Bush | ObjectKind::Gold => {
                if m.gather > 0 {
                    let amount = (m.gather + hat.gather_bonus) * gather_multiplier(world);
                    match kind {
                        ObjectKind::Tree => gain.wood += amount,
                        ObjectKind::Stone => gain.stone += amount,
                        ObjectKind::Bush => gain.food += amount,
                        _ => gain.gold += amount,
                    }
                }
            }
            ObjectKind::CaveEntrance => {}
            ObjectKind::Crate(_) | ObjectKind::Building(_) => events.push(Damage {
                target: entity,
                amount: 0.0,
                building_amount: damage * m.building_mult * hat.building_damage,
                weapon: Some(a.weapon),
                source: source.clone(),
                ..Damage::default()
            }),
        }
    }

    world.resource_mut::<DamageEvents>().0.extend(events);

    let gathered = gain.wood + gain.stone + gain.food + gain.gold;
    if gathered > 0 {
        if let Some(mut res) = world.get_mut::<Resources>(a.entity) {
            res.wood += gain.wood;
            res.stone += gain.stone;
            res.food += gain.food;
            res.gold += gain.gold;
        }
        world.resource_mut::<Dirty>().resources.insert(a.entity);
        grant_xp(world, a.entity, gathered * XP_PER_RESOURCE);
        grant_weapon_xp(world, a.entity, a.weapon, gathered);
        note_gathered(world, a.entity, gathered);
    }
}

/// Pays for a shot. Returns false (and tells the player) if they can't.
fn pay_for_shot(world: &mut World, a: &Attacker, cost: AmmoCost) -> bool {
    let Some(mut res) = world.get_mut::<Resources>(a.entity) else {
        return false;
    };
    let (have, need, what) = match cost {
        AmmoCost::Wood(n) => (&mut res.wood, n, "wood"),
        AmmoCost::Stone(n) => (&mut res.stone, n, "stone"),
        AmmoCost::Ammo(n) => (&mut res.ammo, n, "ammo"),
    };
    if *have < need {
        let text = if what == "ammo" {
            "Out of ammo! Open crates to find more.".to_string()
        } else {
            format!("Not enough {what} to shoot")
        };
        world.resource_mut::<Outbox>().notice(a.id, text, 1);
        return false;
    }
    *have -= need;
    world.resource_mut::<Dirty>().resources.insert(a.entity);
    true
}

/// Whether a position is on top of a platform.
fn on_platform(world: &mut World, pos: Position) -> bool {
    world
        .query::<(&Building, &Position, &Collider)>()
        .iter(world)
        .any(|(b, p, c)| {
            item_def(b.item).is_some_and(|i| i.kind == ItemKind::Platform)
                && (p.0 - pos.0).hypot(p.1 - pos.1) < c.rad + 20.0
        })
}

/// Fires a ranged weapon.
fn shoot(world: &mut World, a: &Attacker, r: &Ranged) {
    if !pay_for_shot(world, a, r.cost) {
        return;
    }
    world.resource_mut::<AttackAnims>().0.push(a.id);
    let hat = hat_effects(a.hat);
    let variant = VARIANTS[usize::from(a.variant)];
    let underground = is_underground(world, a.entity);
    let over_buildings = !underground && on_platform(world, a.pos);
    let middle = f32::from(r.pellets - 1) / 2.0;

    for i in 0..r.pellets {
        let angle = (f32::from(i) - middle).mul_add(r.spread, a.aim);
        spawn_projectile(
            world,
            ProjectileSpec {
                owner: Some(a.entity),
                owner_player: Some(a.id),
                from: (a.pos.0, a.pos.1),
                muzzle: 35.0 + 20.0,
                angle,
                weapon: a.weapon,
                variant: a.variant,
                speed: r.speed,
                range: r.range,
                damage: r.damage * variant.damage_mult * hat.damage_dealt,
                building_damage: r.building_damage * hat.building_damage,
                knockback: r.knockback,
                over_buildings,
                underground,
            },
        );
    }
    // Using a ranged weapon trains it a little even when missing.
    grant_weapon_xp(world, a.entity, a.weapon, 1);
}

/// Turrets and turret gear shoot at the nearest enemy player in range.
fn turrets(world: &mut World) {
    // (entity, owner player, position, is hat, ready)
    let mut shooters: Vec<(Entity, u32, Position, bool)> = Vec::new();

    for (entity, mut building, pos) in world.query::<(Entity, &mut Building, &Position)>().iter_mut(world) {
        if !item_def(building.item).is_some_and(|i| i.kind == ItemKind::Turret) {
            continue;
        }
        building.timer_ms = building.timer_ms.saturating_sub(TICK_MS);
        if building.timer_ms == 0 {
            shooters.push((entity, building.owner, *pos, false));
        }
    }
    let tick = world.resource::<crate::structs::components::TickCount>().0;
    let gear_ticks = u64::from(TURRET_GEAR_RELOAD_MS / TICK_MS);
    for (entity, player, pos, gear) in world.query::<(Entity, &PlayerEntity, &Position, &Gear)>().iter(world) {
        if hat_effects(gear.hat).turret && (tick + u64::from(player.0)).is_multiple_of(gear_ticks) {
            shooters.push((entity, player.0, *pos, true));
        }
    }

    let targets: Vec<(Entity, u32, Position, u8, bool)> = world
        .query::<(Entity, &PlayerEntity, &Position, &Gear, Has<Underground>)>()
        .iter(world)
        .map(|(e, p, pos, g, under)| (e, p.0, *pos, g.hat, under))
        .collect();

    for (shooter, owner, pos, is_hat) in shooters {
        // Turrets are on the surface; turret gear shoots on its wearer's layer.
        let underground = is_hat && is_underground(world, shooter);
        let target = targets
            .iter()
            .filter(|(e, id, _, hat, under)| {
                *e != shooter
                    && *id != owner
                    && *under == underground
                    && !are_allies(world, owner, *id)
                    && !hat_effects(*hat).turret_immune
            })
            .map(|(_, _, p, ..)| (p, (p.0 - pos.0).hypot(p.1 - pos.1)))
            .filter(|(_, d)| *d <= TURRET_RANGE)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((target_pos, _)) = target else {
            continue;
        };
        let angle = (target_pos.1 - pos.1).atan2(target_pos.0 - pos.0);

        if !is_hat {
            if let Some(mut b) = world.get_mut::<Building>(shooter) {
                b.timer_ms = TURRET_RELOAD_MS;
            }
            if let Some(mut dir) = world.get_mut::<AimDir>(shooter) {
                dir.0 = angle;
            }
        }
        spawn_projectile(
            world,
            ProjectileSpec {
                owner: Some(shooter),
                owner_player: Some(owner),
                from: (pos.0, pos.1),
                muzzle: 45.0,
                angle,
                weapon: TURRET_WEAPON,
                variant: 0,
                speed: 120.0,
                range: TURRET_RANGE + 100.0,
                damage: if is_hat { 15.0 } else { 25.0 },
                building_damage: 10.0,
                knockback: 20.0,
                over_buildings: true,
                underground,
            },
        );
    }
}
