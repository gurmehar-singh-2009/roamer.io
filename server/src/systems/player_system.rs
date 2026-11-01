//! Player lifecycle: spawning, dying, respawning and leaving.

use std::collections::HashMap;

use bevy_ecs::{entity::Entity, query::With, resource::Resource, world::World};
use nanorand::Rng;
use shared::{
    to_client::{AddPlayerData, DeathData, SpawnedData},
    PacketType,
};

use crate::{
    catalog::{items::ItemKind, Owned},
    replication::{player_to, upgrades as state},
    structs::{
        bevy::PlayerMap,
        components::{
            Account, AimDir, AttackState, Building, Gear, Health, LastEatTick, Loadout, MoveDir, Name, ObjectEntity,
            PadCooldown, PlayerBundle, PlayerEntity, PlayerStats, Position, Progress, ReloadState, Resources,
            TickCount, Velocity, Weapon, WeaponXp,
        },
        outbox::{Outbox, Target},
        visibility::PlayerKnowledge,
    },
    systems::{
        accounts::Accounts, arena_system, building_system, rng, social_system, Collider, ReactiveCollider, TICK_MS,
    },
    CONFIG,
};

/// Player radius.
pub const PLAYER_RADIUS: f32 = 35.0;
/// Player max health.
pub const PLAYER_HEALTH: f32 = 100.0;

/// What a player keeps between lives (until they disconnect).
#[derive(Clone, Debug, Default)]
pub struct Session {
    /// Name used last.
    pub name:       String,
    /// Account token.
    pub token:      String,
    /// Hats and accessories owned and worn.
    pub gear:       Gear,
    /// Clan id.
    pub clan:       Option<u32>,
    /// Where they died last (for the minimap).
    pub last_death: Option<(f32, f32)>,
}

/// Sessions by player id.
#[derive(Resource, Default)]
pub struct Sessions(pub HashMap<u32, Session>);

/// Picks a random spot on land that isn't inside an object.
pub fn random_land_spot(world: &mut World, margin: f32) -> (f32, f32) {
    let mut rng = rng();
    let land_x = f32::from(CONFIG.map.ocean_start_x) - margin;
    let size = f32::from(CONFIG.map.size) - margin;

    let objects: Vec<(f32, f32, f32)> = world
        .query_filtered::<(&Position, &Collider), With<ObjectEntity>>()
        .iter(world)
        .map(|(p, c)| (p.0, p.1, c.rad))
        .collect();

    let mut spot = (land_x / 2.0, size / 2.0);
    for _ in 0..60 {
        let x = rng.generate::<f32>().mul_add(land_x - margin, margin);
        let y = rng.generate::<f32>().mul_add(size - margin, margin);
        spot = (x, y);
        let clear = objects.iter().all(|&(ox, oy, r)| {
            let (dx, dy) = (ox - x, oy - y);
            dx.mul_add(dx, dy * dy) > (r + margin) * (r + margin)
        });
        if clear {
            break;
        }
    }
    spot
}

/// Where a player spawns: their spawn pad if they have one, else anywhere.
fn spawn_point(world: &mut World, id: u32) -> (f32, f32) {
    let pad = world
        .query::<(&Building, &Position)>()
        .iter(world)
        .find(|(b, _)| {
            b.owner == id && crate::catalog::items::item_def(b.item).is_some_and(|i| i.kind == ItemKind::SpawnPad)
        })
        .map(|(_, p)| (p.0, p.1));
    pad.unwrap_or_else(|| random_land_spot(world, 120.0))
}

/// Spawns a player (or does nothing if they're alive). Returns their entity.
#[allow(
    clippy::too_many_lines,
    reason = "one step after another; splitting it hides the order"
)]
pub fn spawn_player(world: &mut World, id: u32, name: &str, token: &str) -> Option<Entity> {
    if world.resource::<PlayerMap>().map.contains_key(&id) {
        return None;
    }

    // Account: reuse the token if it's known, else make a new account.
    let token = world.resource_mut::<Accounts>().login(token);
    let account = world.resource::<Accounts>().get(&token).cloned().unwrap_or_default();

    let session = {
        let mut sessions = world.resource_mut::<Sessions>();
        let session = sessions.0.entry(id).or_default();
        session.name = name.to_string();
        session.token.clone_from(&token);
        session.clone()
    };

    let (x, y) = spawn_point(world, id);
    let tick = world.resource::<TickCount>().0;

    let entity = world
        .spawn((
            PlayerBundle {
                player:   PlayerEntity(id),
                name:     Name(name.to_string()),
                pos:      Position(x, y),
                vel:      Velocity(0.0, 0.0),
                move_dir: MoveDir(None),
                aim:      AimDir(0.0),
                weapon:   Weapon(crate::catalog::weapons::FISTS),
                reload:   ReloadState(0, 300),
                health:   Health(PLAYER_HEALTH, PLAYER_HEALTH),
                attack:   AttackState(false),
                res:      Resources {
                    wood: 20,
                    food: 20,
                    ..Resources::default()
                },
                progress: Progress::default(),
                loadout:  Loadout(Owned::starting()),
                wxp:      WeaponXp::default(),
                gear:     Gear {
                    skin: account.skin,
                    ..session.gear
                },
                stats:    PlayerStats {
                    spawn_tick: tick,
                    ..PlayerStats::default()
                },
                collider: Collider::circle(PLAYER_RADIUS),
                reactive: ReactiveCollider,
                eat:      LastEatTick(0),
                pads:     PadCooldown::default(),
            },
            Account(token.clone()),
        ))
        .id();

    world.resource_mut::<PlayerMap>().map.insert(id, entity);
    world.resource_mut::<PlayerKnowledge>().add_player(id);

    // Tell the new player about everyone else, and everyone about them.
    let others: Vec<(u32, String, shared::to_client::PlayerTO)> = world
        .resource::<PlayerMap>()
        .map
        .iter()
        .filter(|(&other, _)| other != id)
        .map(|(&other, &e)| (other, e))
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|(other, e)| Some((other, world.get::<Name>(e)?.0.clone(), player_to(world, e)?)))
        .collect();

    let me = player_to(world, entity)?;
    let mut out = world.resource_mut::<Outbox>();
    for (_, other_name, data) in others {
        out.send_reliable(
            Target::Player(id),
            PacketType::AddPlayer,
            &AddPlayerData {
                is_mine: false,
                name: other_name,
                data,
            },
        );
    }
    out.send_reliable(
        Target::Player(id),
        PacketType::AddPlayer,
        &AddPlayerData {
            is_mine: true,
            name:    name.to_string(),
            data:    me.clone(),
        },
    );
    out.send_reliable(
        Target::AllExcept(id),
        PacketType::AddPlayer,
        &AddPlayerData {
            is_mine: false,
            name:    name.to_string(),
            data:    me,
        },
    );
    out.send_reliable(
        Target::Player(id),
        PacketType::Spawned,
        &SpawnedData {
            id,
            token,
            best_score: account.best_score,
            total_kills: account.total_kills,
            games: account.games,
            coins_earned: 0,
        },
    );

    state::send_all(world, entity);
    crate::systems::cave_system::send_all_cells(world, id);
    social_system::send_clan_state(world, Some(id));
    social_system::give_quest(world, entity);
    Some(entity)
}

/// Handles everyone who died this tick.
pub fn process_deaths(world: &mut World) {
    let deaths = std::mem::take(&mut world.resource_mut::<crate::structs::components::Deaths>().0);
    for (victim, killer) in deaths {
        kill_player(world, victim, killer.as_ref());
    }
}

/// Removes a dead player, sends them the death screen and rewards the
/// killer.
fn kill_player(world: &mut World, victim: u32, killer: Option<&(u32, String)>) {
    let Some(&entity) = world.resource::<PlayerMap>().map.get(&victim) else {
        return;
    };
    // Arena fighters don't really die; they go back to the main world.
    if let Some(room) = world.get::<crate::structs::components::InArena>(entity).map(|a| a.0) {
        arena_system::fighter_down(world, room, victim);
        return;
    }
    world.resource_mut::<PlayerMap>().map.remove(&victim);

    let tick = world.resource::<TickCount>().0;
    let res = world.get::<Resources>(entity).copied().unwrap_or_default();
    let age = world.get::<Progress>(entity).map_or(1, |p| p.age);
    let (kills, spawn_tick) = world
        .get::<PlayerStats>(entity)
        .map_or((0, tick), |s| (s.kills, s.spawn_tick));
    let pos = world.get::<Position>(entity).map(|p| (p.0, p.1));
    let token = world.get::<Account>(entity).map(|a| a.0.clone());

    let ticks_alive = tick.saturating_sub(spawn_tick);
    let time_alive = u32::try_from(ticks_alive.saturating_mul(u64::from(TICK_MS)) / 1000).unwrap_or(u32::MAX);

    // Account stats and coins.
    let coins = res.gold / 100 + kills * 5;
    let account = token.and_then(|t| world.resource_mut::<Accounts>().record_game(&t, res.gold, kills, coins));

    if let Some(session) = world.resource_mut::<Sessions>().0.get_mut(&victim) {
        session.last_death = pos;
    }

    let killer_name = killer.as_ref().map_or_default(|(_, n)| n.clone());
    {
        let mut out = world.resource_mut::<Outbox>();
        // Account totals first, so the death screen can show coins earned.
        if let Some(acc) = account {
            out.send_reliable(
                Target::Player(victim),
                PacketType::Spawned,
                &SpawnedData {
                    id:           victim,
                    token:        acc.token.clone(),
                    best_score:   acc.best_score,
                    total_kills:  acc.total_kills,
                    games:        acc.games,
                    coins_earned: coins,
                },
            );
        }
        out.send_reliable(
            Target::Player(victim),
            PacketType::Death,
            &DeathData {
                killer: killer_name,
                score: res.gold,
                age,
                kills,
                time_alive,
            },
        );
        out.send_raw(
            Target::All,
            [vec![PacketType::RemovePlayer as u8], victim.to_le_bytes().to_vec()].concat(),
            true,
        );
    }

    world.despawn(entity);

    // Reward the killer.
    if let Some((killer_id, _)) = killer {
        let Some(&killer_entity) = world.resource::<PlayerMap>().map.get(killer_id) else {
            return;
        };
        if let Some(mut stats) = world.get_mut::<PlayerStats>(killer_entity) {
            stats.kills += 1;
        }
        let bounty = 100 + res.gold / 4;
        if let Some(mut r) = world.get_mut::<Resources>(killer_entity) {
            r.gold += bounty;
        }
        crate::systems::progression_system::grant_xp(world, killer_entity, 100 + age * 50);
        social_system::quest_progress(world, killer_entity, social_system::QuestKind::KillPlayers, 1);
        world
            .resource_mut::<crate::structs::components::Dirty>()
            .resources
            .insert(killer_entity);
        world
            .resource_mut::<Outbox>()
            .notice(*killer_id, format!("You killed a player! +{bounty} gold"), 2);
    }
}

/// Cleans up after a player disconnects.
pub fn disconnect(world: &mut World, id: u32) {
    if let Some(entity) = world.resource_mut::<PlayerMap>().map.remove(&id) {
        if let Some(name) = world.get::<Name>(entity) {
            tracing::info!("player {} ({}) left", name.0, id);
        }
        world.despawn(entity);
        world.resource_mut::<Outbox>().send_raw(
            Target::All,
            [vec![PacketType::RemovePlayer as u8], id.to_le_bytes().to_vec()].concat(),
            true,
        );
    }

    arena_system::leave(world, id);
    social_system::leave_clan(world, id);
    building_system::remove_player_buildings(world, id);
    world.resource_mut::<PlayerKnowledge>().remove_player(id);
    world.resource_mut::<Sessions>().0.remove(&id);
}
