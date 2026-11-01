//! Player items: eating, placing buildings, and everything buildings do each
//! tick (regeneration, resource generators, pads, traps, turrets' timers,
//! TNT, spikes and chests).

use std::collections::HashMap;

use bevy_ecs::{entity::Entity, query::Without, world::World};
use nanorand::Rng;
use shared::{
    to_client::{ChestData, ExplosionData, RemoveObjectData, UpdateObjectHealthData},
    to_server::ChestMessage,
    PacketType,
};

use crate::{
    catalog::items::{item_def, ItemDef, ItemKind, BLOCKER_RADIUS},
    structs::{
        components::{
            AimDir, AnimalEntity, Building, ChestStore, Damage, DamageEvents, DamageSource, Dirty, Health, LastEatTick,
            Loadout, ObjectBundle, ObjectEntity, ObjectKind, PadCooldown, PlayerEntity, Position, Resources,
            SolidObjectBundle, TickCount, Trapped, Velocity,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        cave_system::{in_rock, is_underground, open_area_at, Underground},
        damage_system::player_entity,
        health_system::heal,
        player_system::{random_land_spot, PLAYER_RADIUS},
        progression_system::grant_xp,
        rng,
        social_system::{are_allies, quest_progress, QuestKind},
        Collider, NonReactiveCollider, PLAYER_MAX_SPEED, TICK_MS,
    },
    CONFIG,
};

/// Time after taking damage before a building starts to regenerate.
pub const REGEN_DELAY_MS: u32 = 5000;
/// Time between regeneration steps.
const REGEN_INTERVAL_MS: u32 = 200;
/// Fraction of max health restored per regeneration step.
const REGEN_FRACTION: f32 = 0.01;
/// Gap between the player and a building placed in front of them.
const PLACE_GAP: f32 = 5.0;
/// How far a blocked placement may be pushed aside.
const MAX_PUSH: f32 = 60.0;
/// Extra distance past touching at which a spike still hits.
const SPIKE_REACH: f32 = 5.0;
/// Impact speeds below this do no spike damage, so standing still next to a
/// spike (for example in a pit trap) is safe.
const SPIKE_MIN_IMPACT: f32 = 5.0;
/// Spike damage cap, as a multiple of the spike's damage.
const SPIKE_MAX_SCALE: f32 = 2.0;
/// Minimum time between two spike hits on the same target.
const SPIKE_COOLDOWN_MS: u32 = 400;
/// TNT fuse.
const TNT_FUSE_MS: u32 = 3000;
/// TNT blast radius.
const TNT_RADIUS: f32 = 260.0;
/// TNT damage at the center (players and animals).
const TNT_DAMAGE: f32 = 70.0;
/// TNT damage at the center (buildings).
const TNT_BUILDING_DAMAGE: f32 = 600.0;
/// Boost pad launch speed.
const BOOST_SPEED: f32 = 90.0;
/// Healing pad health per second.
const HEAL_PAD_PER_SEC: f32 = 10.0;
/// How close to their chest a player must be to use it.
pub const CHEST_REACH: f32 = 160.0;

/// Uses an item: eats food or places a building.
pub fn use_item(world: &mut World, entity: Entity, item: u8, tick: u64) -> bool {
    if !world.get::<Loadout>(entity).is_some_and(|l| l.0.has_item(item)) {
        return false;
    }
    let Some(def) = item_def(item) else {
        return false;
    };
    let done = match def.kind {
        ItemKind::Food { heal: amount } => eat(world, entity, def, amount, tick),
        _ => place(world, entity, def),
    };
    if done {
        world.resource_mut::<Dirty>().resources.insert(entity);
    }
    done
}

/// Whether the player can pay for an item.
const fn can_afford(res: Resources, def: &ItemDef) -> bool {
    res.wood >= def.wood && res.stone >= def.stone && res.food >= def.food
}

/// Pays for an item.
fn pay(world: &mut World, entity: Entity, def: &ItemDef) {
    if let Some(mut r) = world.get_mut::<Resources>(entity) {
        r.wood -= def.wood;
        r.stone -= def.stone;
        r.food -= def.food;
    }
}

/// Eats food. Only one eat per player per tick, which stops scripts from
/// eating many times in one tick to out-heal damage.
fn eat(world: &mut World, entity: Entity, def: &ItemDef, amount: f32, tick: u64) -> bool {
    if world.get::<LastEatTick>(entity).is_none_or(|t| t.0 >= tick) {
        return false;
    }
    let Some(health) = world.get::<Health>(entity).copied() else {
        return false;
    };
    let res = world.get::<Resources>(entity).copied().unwrap_or_default();
    if health.0 >= health.1 || !can_afford(res, def) {
        return false;
    }
    pay(world, entity, def);
    if let Some(mut t) = world.get_mut::<LastEatTick>(entity) {
        t.0 = tick;
    }
    let id = world.get::<PlayerEntity>(entity).map_or(0, |p| p.0);
    let pos = world.get::<Position>(entity).map_or((0.0, 0.0), |p| (p.0, p.1));
    heal(world, entity, id, pos, amount);
    true
}

/// Where a building would overlap something, and with what: (center, radius).
fn blockers_at(world: &mut World, x: f32, y: f32, r: f32) -> Vec<(f32, f32, f32)> {
    world
        .query::<(&Position, &Collider, Option<&ObjectEntity>, Option<&PlayerEntity>)>()
        .iter(world)
        .filter(|(_, _, o, p)| o.is_some() || p.is_some())
        .filter(|(p, c, ..)| {
            let (dx, dy) = (p.0 - x, p.1 - y);
            dx.mul_add(dx, dy * dy) < (c.rad + r) * (c.rad + r)
        })
        .map(|(p, c, ..)| (p.0, p.1, c.rad))
        .collect()
}

/// Finds a free spot near (x, y) by pushing out of whatever is in the way.
fn free_spot(world: &mut World, (mut x, mut y): (f32, f32), r: f32) -> Option<(f32, f32)> {
    let start = (x, y);
    for _ in 0..6 {
        let blocking = blockers_at(world, x, y, r);
        let Some(&(bx, by, br)) = blocking.first() else {
            return Some((x, y));
        };
        // Push straight away from the blocker's center until just touching.
        let (dx, dy) = (x - bx, y - by);
        let len = dx.hypot(dy).max(0.001);
        let need = br + r + 1.0;
        x = f32::mul_add(dx / len, need, bx);
        y = f32::mul_add(dy / len, need, by);
        if (x - start.0).hypot(y - start.1) > MAX_PUSH {
            return None;
        }
    }
    None
}

/// Places a building in front of the player.
fn place(world: &mut World, entity: Entity, def: &ItemDef) -> bool {
    let Some(id) = world.get::<PlayerEntity>(entity).map(|p| p.0) else {
        return false;
    };
    // In the caves, only Tunnels can be built: they dig up to the surface.
    let underground = is_underground(world, entity);
    if underground && def.kind != ItemKind::Tunnel {
        world
            .resource_mut::<Outbox>()
            .notice(id, "Only Tunnels can be built in the caves", 1);
        return false;
    }
    let res = world.get::<Resources>(entity).copied().unwrap_or_default();
    if !can_afford(res, def) {
        world
            .resource_mut::<Outbox>()
            .notice(id, format!("Not enough resources for {}", def.name), 1);
        return false;
    }

    let owned = world
        .query::<&Building>()
        .iter(world)
        .filter(|b| b.owner == id && b.item == def.id)
        .count();
    if owned >= def.limit {
        world
            .resource_mut::<Outbox>()
            .notice(id, format!("You can't place more than {} {}", def.limit, def.name), 1);
        return false;
    }

    let Some(pos) = world.get::<Position>(entity).copied() else {
        return false;
    };
    let aim = world
        .get::<crate::structs::components::AimDir>(entity)
        .map_or(0.0, |a| a.0);
    let dist = PLAYER_RADIUS + def.radius + PLACE_GAP;
    let wanted = (aim.cos().mul_add(dist, pos.0), aim.sin().mul_add(dist, pos.1));

    let Some((x, y)) = free_spot(world, wanted, def.radius) else {
        if underground {
            world
                .resource_mut::<Outbox>()
                .notice(id, "Something on the surface is in the way", 1);
        }
        return false;
    };
    if underground && in_rock(world, x, y) {
        world.resource_mut::<Outbox>().notice(id, "Dig out more room first", 1);
        return false;
    }

    let map = f32::from(CONFIG.map.size);
    if x < def.radius || y < def.radius || x > map - def.radius || y > map - def.radius {
        return false;
    }

    // Enemy blockers stop building nearby.
    let blocked = world.query::<(&Building, &Position)>().iter(world).any(|(b, p)| {
        item_def(b.item).is_some_and(|i| i.kind == ItemKind::Blocker)
            && b.owner != id
            && !are_allies(world, b.owner, id)
            && (p.0 - x).hypot(p.1 - y) < BLOCKER_RADIUS
    });
    if blocked {
        world
            .resource_mut::<Outbox>()
            .notice(id, "An enemy blocker stops you building here", 1);
        return false;
    }

    pay(world, entity, def);
    spawn_building(world, id, def, (x, y), aim);
    if def.kind == ItemKind::Tunnel {
        open_area_at(world, x, y);
    }
    quest_progress(world, entity, QuestKind::PlaceBuildings, 1);
    true
}

/// Spawns a building entity.
pub fn spawn_building(world: &mut World, owner: u32, def: &ItemDef, (x, y): (f32, f32), dir: f32) -> Entity {
    let object = ObjectBundle {
        object:   ObjectEntity,
        kind:     ObjectKind::Building(def.id),
        pos:      Position(x, y),
        dir:      AimDir(dir),
        health:   Health(def.health, def.health),
        collider: Collider::circle(def.radius),
    };
    let building = Building {
        owner,
        item: def.id,
        regen_wait_ms: 0,
        timer_ms: if def.kind == ItemKind::Tnt { TNT_FUSE_MS } else { 1000 },
    };
    let mut entity = if def.walkable() {
        world.spawn((object, building))
    } else {
        world.spawn((
            SolidObjectBundle {
                object,
                solid: NonReactiveCollider,
            },
            building,
        ))
    };
    if def.kind == ItemKind::Chest {
        entity.insert(ChestStore::default());
    }
    entity.id()
}

/// Despawns every building owned by a player.
pub fn remove_player_buildings(world: &mut World, owner: u32) {
    let owned: Vec<Entity> = world
        .query::<(Entity, &Building)>()
        .iter(world)
        .filter(|(_, b)| b.owner == owner)
        .map(|(e, _)| e)
        .collect();
    for e in owned {
        remove_object(world, e);
    }
}

/// Despawns an object and tells clients that know it.
pub fn remove_object(world: &mut World, entity: Entity) {
    let index = entity.index();
    world.resource_mut::<Outbox>().send_reliable(
        Target::ForgetObject(index),
        PacketType::RemoveObject,
        &RemoveObjectData { id: index },
    );
    world.despawn(entity);
}

/// Blows up TNT at `pos`.
pub fn explode(world: &mut World, (x, y): (f32, f32), owner: u32) {
    world.resource_mut::<Outbox>().send(
        Target::Near(x, y),
        PacketType::Explosion,
        &ExplosionData {
            x,
            y,
            radius: TNT_RADIUS,
        },
    );

    let hits: Vec<(Entity, f32, bool)> = world
        .query::<(
            Entity,
            &Position,
            Option<&PlayerEntity>,
            Option<&AnimalEntity>,
            Option<&ObjectKind>,
        )>()
        .iter(world)
        .filter(|(e, ..)| !is_underground(world, *e))
        .filter_map(|(e, p, player, animal, kind)| {
            let dist = (p.0 - x).hypot(p.1 - y);
            if dist > TNT_RADIUS {
                return None;
            }
            let falloff = 1.0 - dist / TNT_RADIUS;
            let is_object = matches!(kind, Some(ObjectKind::Building(_) | ObjectKind::Crate(_)));
            (player.is_some() || animal.is_some() || is_object).then_some((e, falloff, is_object))
        })
        .collect();

    let source = DamageSource {
        player: Some(owner),
        pos: (x, y),
        // Explosions open any crate.
        variant: u8::MAX,
        ..DamageSource::default()
    };
    let events = hits.into_iter().map(|(target, falloff, is_object)| Damage {
        target,
        amount: if is_object { 0.0 } else { TNT_DAMAGE * falloff },
        building_amount: if is_object { TNT_BUILDING_DAMAGE * falloff } else { 0.0 },
        knockback: 60.0 * falloff,
        source: source.clone(),
        ..Damage::default()
    });
    world.resource_mut::<DamageEvents>().0.extend(events);
}

/// Everything buildings do each tick.
pub fn building_tick_system(world: &mut World) {
    let tick = world.resource::<TickCount>().0;
    let second = tick.is_multiple_of(u64::from(1000 / TICK_MS));

    regenerate(world);
    tick_tnt(world);
    if second {
        generate_resources(world);
        heal_pads(world);
        update_chests(world);
    }
    pads_and_traps(world);
}

/// Slowly heals buildings that haven't been damaged for a while.
fn regenerate(world: &mut World) {
    let mut updates = Vec::new();
    for (entity, mut health, mut building) in world.query::<(Entity, &mut Health, &mut Building)>().iter_mut(world) {
        building.regen_wait_ms = building.regen_wait_ms.saturating_sub(TICK_MS);
        if building.regen_wait_ms > 0 || health.0 <= 0.0 || health.0 >= health.1 {
            continue;
        }
        health.0 = health.1.mul_add(REGEN_FRACTION, health.0).min(health.1);
        building.regen_wait_ms = REGEN_INTERVAL_MS;
        updates.push((entity.index(), health.0, health.1));
    }
    let mut out = world.resource_mut::<Outbox>();
    for (id, health, max_health) in updates {
        out.send(
            Target::KnowsObject(id),
            PacketType::UpdateObjectHealth,
            &UpdateObjectHealthData { id, health, max_health },
        );
    }
}

/// Counts down TNT fuses.
fn tick_tnt(world: &mut World) {
    let mut blasts = Vec::new();
    for (entity, mut building, pos) in world.query::<(Entity, &mut Building, &Position)>().iter_mut(world) {
        if item_def(building.item).is_some_and(|i| i.kind == ItemKind::Tnt) {
            building.timer_ms = building.timer_ms.saturating_sub(TICK_MS);
            if building.timer_ms == 0 {
                blasts.push((entity, (pos.0, pos.1), building.owner));
            }
        }
    }
    for (entity, pos, owner) in blasts {
        remove_object(world, entity);
        explode(world, pos, owner);
    }
}

/// Fractional windmill gold carried over between seconds, by owner.
#[derive(bevy_ecs::resource::Resource, Default)]
pub struct GoldRemainder(pub HashMap<u32, f32>);

/// Windmills make gold, mines stone and saplings wood for their owners.
fn generate_resources(world: &mut World) {
    let mut income: HashMap<u32, (Resources, f32)> = HashMap::new();
    for building in world.query::<&Building>().iter(world) {
        let Some(def) = item_def(building.item) else {
            continue;
        };
        let entry = income.entry(building.owner).or_default();
        match def.kind {
            ItemKind::Windmill { gold_per_sec } => entry.1 += gold_per_sec,
            ItemKind::Mine => entry.0.stone += 1,
            ItemKind::Sapling => entry.0.wood += 1,
            _ => {}
        }
    }
    for (owner, (mut gain, gold)) in income {
        let carried = world.resource::<GoldRemainder>().0.get(&owner).copied().unwrap_or(0.0) + gold;
        let whole = carried.floor();
        world.resource_mut::<GoldRemainder>().0.insert(owner, carried - whole);
        // A few windmills make a few gold per second.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = whole as u32;
        gain.gold = whole;
        if gain == Resources::default() {
            continue;
        }
        let Some(entity) = player_entity(world, owner) else {
            continue;
        };
        if let Some(mut r) = world.get_mut::<Resources>(entity) {
            r.gold += gain.gold;
            r.stone += gain.stone;
            r.wood += gain.wood;
        }
        world.resource_mut::<Dirty>().resources.insert(entity);
        grant_xp(world, entity, gain.gold);
    }
}

/// Players overlapping a building of this kind.
fn players_on(world: &mut World, kind: ItemKind) -> Vec<(Entity, u32, Entity, Building, Position, f32)> {
    let pads: Vec<(Entity, Building, Position, f32)> = world
        .query::<(Entity, &Building, &Position, &Collider)>()
        .iter(world)
        .filter(|(_, b, ..)| item_def(b.item).is_some_and(|i| i.kind == kind))
        .map(|(e, b, p, c)| (e, *b, *p, c.rad))
        .collect();
    if pads.is_empty() {
        return Vec::new();
    }
    let players: Vec<(Entity, u32, Position)> = world
        .query_filtered::<(Entity, &PlayerEntity, &Position), Without<Underground>>()
        .iter(world)
        .map(|(e, p, pos)| (e, p.0, *pos))
        .collect();
    let mut hits = Vec::new();
    for (pe, id, ppos) in players {
        for (be, b, bpos, r) in &pads {
            if (ppos.0 - bpos.0).hypot(ppos.1 - bpos.1) < r + PLAYER_RADIUS * 0.6 {
                hits.push((pe, id, *be, *b, ppos, *r));
                break;
            }
        }
    }
    hits
}

/// Healing pads heal friendly players standing on them.
fn heal_pads(world: &mut World) {
    for (entity, id, _, b, pos, _) in players_on(world, ItemKind::HealPad) {
        if b.owner == id || are_allies(world, b.owner, id) {
            heal(world, entity, id, (pos.0, pos.1), HEAL_PAD_PER_SEC);
        }
    }
}

/// Boost pads, teleporters and pit traps.
fn pads_and_traps(world: &mut World) {
    for (_, mut cd) in world.query::<(Entity, &mut PadCooldown)>().iter_mut(world) {
        cd.0 = cd.0.saturating_sub(TICK_MS);
    }
    let ready = |world: &World, e: Entity| world.get::<PadCooldown>(e).is_some_and(|c| c.0 == 0);

    for (entity, _, pad, _, _, _) in players_on(world, ItemKind::BoostPad) {
        if !ready(world, entity) {
            continue;
        }
        let dir = world.get::<AimDir>(pad).map_or(0.0, |d| d.0);
        if let Some(mut vel) = world.get_mut::<Velocity>(entity) {
            *vel = Velocity(dir.cos() * BOOST_SPEED, dir.sin() * BOOST_SPEED);
        }
        world.entity_mut(entity).insert(PadCooldown(300));
    }

    for (entity, _, _, _, _, _) in players_on(world, ItemKind::Teleporter) {
        if !ready(world, entity) {
            continue;
        }
        let spot = random_land_spot(world, 100.0);
        if let Some(mut p) = world.get_mut::<Position>(entity) {
            *p = Position(spot.0, spot.1);
        }
        world.entity_mut(entity).insert(PadCooldown(2000));
    }

    for (entity, id, trap, b, _, _) in players_on(world, ItemKind::PitTrap) {
        if b.owner != id && !are_allies(world, b.owner, id) && world.get::<Trapped>(entity).is_none() {
            world.entity_mut(entity).insert(Trapped(trap));
            world
                .resource_mut::<Outbox>()
                .notice(id, "You fell into a trap! Break it to escape.", 1);
        }
    }
}

/// Sends chest contents to players who have a chest.
fn update_chests(world: &mut World) {
    let chests: Vec<(u32, Position, Resources)> = world
        .query::<(&Building, &Position, &ChestStore)>()
        .iter(world)
        .map(|(b, p, s)| (b.owner, *p, s.0))
        .collect();
    for (owner, cpos, store) in chests {
        let Some(entity) = player_entity(world, owner) else {
            continue;
        };
        let near = world
            .get::<Position>(entity)
            .is_some_and(|p| (p.0 - cpos.0).hypot(p.1 - cpos.1) < CHEST_REACH);
        world.resource_mut::<Outbox>().send(
            Target::Player(owner),
            PacketType::ChestState,
            &ChestData {
                near,
                has_chest: true,
                wood: store.wood,
                stone: store.stone,
                food: store.food,
                gold: store.gold,
            },
        );
    }
}

/// Moves resources into or out of the player's chest (as much of each amount
/// as there is). They must be standing next to it.
pub fn chest_action(world: &mut World, entity: Entity, msg: &ChestMessage) -> bool {
    let (Some(id), Some(pos)) = (
        world.get::<PlayerEntity>(entity).map(|p| p.0),
        world.get::<Position>(entity).copied(),
    ) else {
        return false;
    };
    let chest = world
        .query::<(Entity, &Building, &Position)>()
        .iter(world)
        .find(|(_, b, p)| {
            b.owner == id
                && (p.0 - pos.0).hypot(p.1 - pos.1) < CHEST_REACH
                && item_def(b.item).is_some_and(|i| i.kind == ItemKind::Chest)
        })
        .map(|(e, ..)| e);
    let Some(chest) = chest else {
        world
            .resource_mut::<Outbox>()
            .notice(id, "Stand next to your chest to use it", 1);
        return false;
    };
    let (Some(mut res), Some(mut store)) = (
        world.get::<Resources>(entity).copied(),
        world.get::<ChestStore>(chest).map(|s| s.0),
    ) else {
        return false;
    };
    let (from, to) = if msg.deposit {
        (&mut res, &mut store)
    } else {
        (&mut store, &mut res)
    };
    let mut moved = 0;
    for (have, get, want) in [
        (&mut from.wood, &mut to.wood, msg.wood),
        (&mut from.stone, &mut to.stone, msg.stone),
        (&mut from.food, &mut to.food, msg.food),
        (&mut from.gold, &mut to.gold, msg.gold),
    ] {
        let n = want.min(*have);
        *have -= n;
        *get += n;
        moved += n;
    }
    if moved == 0 {
        return false;
    }
    world.entity_mut(entity).insert(res);
    world.entity_mut(chest).insert(ChestStore(store));
    world.resource_mut::<Dirty>().resources.insert(entity);
    update_chests(world);
    true
}

/// Damages players and animals that run into spikes.
///
/// Damage scales with how fast the target moves into the spike, so standing
/// still next to one (for example, trapped in a pit) does nothing, while
/// being knocked into one does extra. Targets bounce off.
pub fn spike_system(world: &mut World) {
    let spikes: Vec<(Position, f32, u32, f32, bool)> = world
        .query::<(&Building, &Position, &Collider)>()
        .iter(world)
        .filter_map(|(b, p, c)| match item_def(b.item)?.kind {
            ItemKind::Spike { damage, poison } => Some((*p, c.rad, b.owner, damage, poison)),
            _ => None,
        })
        .collect();

    let mut cooldowns = std::mem::take(&mut world.resource_mut::<SpikeCooldowns>().0);
    cooldowns.retain(|_, ms| {
        *ms = ms.saturating_sub(TICK_MS);
        *ms > 0
    });
    if spikes.is_empty() {
        world.resource_mut::<SpikeCooldowns>().0 = cooldowns;
        return;
    }

    let targets: Vec<(Entity, Option<u32>, Position, Velocity, f32)> = world
        .query::<(
            Entity,
            &Position,
            &Velocity,
            &Collider,
            Option<&PlayerEntity>,
            Option<&AnimalEntity>,
        )>()
        .iter(world)
        .filter(|(e, .., p, a)| (p.is_some() || a.is_some()) && !is_underground(world, *e))
        .map(|(e, pos, vel, col, p, _)| (e, p.map(|p| p.0), *pos, *vel, col.rad))
        .collect();

    let mut events = Vec::new();
    for (entity, player, pos, vel, rad) in targets {
        if cooldowns.contains_key(&entity) {
            continue;
        }
        for &(spos, srad, owner, damage, poison) in &spikes {
            if let Some(id) = player {
                if id == owner || are_allies(world, owner, id) {
                    continue;
                }
            }
            let (dx, dy) = (spos.0 - pos.0, spos.1 - pos.1);
            let reach = rad + srad + SPIKE_REACH;
            let dist_sq = dx.mul_add(dx, dy * dy);
            if dist_sq >= reach * reach || dist_sq <= f32::EPSILON {
                continue;
            }
            let dist = dist_sq.sqrt();
            let (nx, ny) = (dx / dist, dy / dist);
            let impact = vel.0.mul_add(nx, vel.1 * ny);
            if impact < SPIKE_MIN_IMPACT {
                continue;
            }
            let scale = (impact / PLAYER_MAX_SPEED).min(SPIKE_MAX_SCALE);

            // Bounce off: reflect the velocity along the contact normal.
            if let Some(mut v) = world.get_mut::<Velocity>(entity) {
                v.0 = f32::mul_add(2.0 * impact, -nx, v.0);
                v.1 = f32::mul_add(2.0 * impact, -ny, v.1);
            }
            events.push(Damage {
                target: entity,
                amount: damage * scale,
                poison,
                source: DamageSource {
                    player: Some(owner),
                    pos: (spos.0, spos.1),
                    ..DamageSource::default()
                },
                ..Damage::default()
            });
            cooldowns.insert(entity, SPIKE_COOLDOWN_MS);
            break;
        }
    }
    world.resource_mut::<SpikeCooldowns>().0 = cooldowns;
    world.resource_mut::<DamageEvents>().0.extend(events);
}

/// Per-target spike cooldowns.
#[derive(bevy_ecs::resource::Resource, Default)]
pub struct SpikeCooldowns(pub HashMap<Entity, u32>);

/// Frees players whose trap was destroyed.
pub fn release_trapped(world: &mut World) {
    let freed: Vec<Entity> = world
        .query::<(Entity, &Trapped)>()
        .iter(world)
        .filter(|(_, t)| world.get_entity(t.0).is_err())
        .map(|(e, _)| e)
        .collect();
    for e in freed {
        world.entity_mut(e).remove::<Trapped>();
    }
}

/// Random rotation for new objects.
pub fn random_dir() -> f32 {
    rng().generate::<f32>() * std::f32::consts::TAU
}
