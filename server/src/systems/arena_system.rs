//! Arena team fights (1v1 up to 4v4) with custom loadouts, map presets and
//! rules. Each room fights in its own walled area east of the main map;
//! players' main-world state is saved when the fight starts and restored
//! afterwards.

use std::collections::HashMap;

use bevy_ecs::{entity::Entity, resource::Resource, world::World};
use shared::{
    to_client::{ArenaRoomTO, ArenaStateData, ClanMemberTO},
    to_server::ArenaMessage,
    PacketType,
};

use crate::{
    catalog::{
        weapons::{weapon, Slot},
        Owned,
    },
    replication::upgrades::send_all as refresh_player,
    structs::{
        components::{
            AimDir, Health, InArena, Loadout, ObjectBundle, ObjectEntity, ObjectKind, Position, Progress, ReloadState,
            Resources, SolidObjectBundle, Velocity, Weapon, WeaponXp,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        building_system::remove_object, damage_system::player_entity, player_system::Sessions,
        progression_system::switch_weapon, Collider, NonReactiveCollider,
    },
};

/// Size of an arena.
const ARENA_SIZE: f32 = 2400.0;
/// Where the first arena starts (east of the main map's wall).
const ARENA_ORIGIN_X: f32 = 17_500.0;
/// Space between arenas.
const ARENA_SPACING: f32 = 3000.0;
/// Room limit.
const MAX_ROOMS: usize = 8;

/// A player's main-world state while they're in the arena.
#[derive(Clone, Debug)]
struct Saved {
    /// Position.
    pos:      Position,
    /// Health.
    health:   Health,
    /// Resources.
    res:      Resources,
    /// Age and XP.
    progress: Progress,
    /// Weapons and items.
    loadout:  Loadout,
    /// Weapon XP.
    wxp:      WeaponXp,
    /// Held weapon.
    weapon:   Weapon,
}

/// An arena room.
#[derive(Clone, Debug)]
pub struct Room {
    /// Room id.
    pub id:            u32,
    /// Host player id.
    pub host:          u32,
    /// Players per team.
    pub team_size:     u8,
    /// Map preset.
    pub map:           u8,
    /// Max health.
    pub max_health:    f32,
    /// Building damage multiplier.
    pub object_damage: f32,
    /// Knockback multiplier.
    pub bounciness:    f32,
    /// Players in join order; the first half is team 0.
    pub players:       Vec<u32>,
    /// Chosen loadouts: (primary, secondary).
    pub loadouts:      HashMap<u32, (u8, Option<u8>)>,
    /// Players still alive in the fight.
    pub alive:         Vec<u32>,
    /// Whether the fight is on.
    pub started:       bool,
    /// Arena slot (position on the map).
    slot:              u32,
    /// Saved main-world state.
    saved:             HashMap<u32, Saved>,
    /// Arena walls and obstacles.
    objects:           Vec<Entity>,
}

impl Room {
    /// The team a player is on (0 or 1).
    pub fn team(&self, id: u32) -> Option<usize> {
        let index = self.players.iter().position(|&p| p == id)?;
        Some(usize::from(index >= usize::from(self.team_size)))
    }

    /// The arena's center.
    fn center(&self) -> (f32, f32) {
        // Slots are a handful of small numbers.
        #[allow(clippy::cast_precision_loss)]
        let x = (self.slot as f32).mul_add(ARENA_SPACING, ARENA_ORIGIN_X) + ARENA_SIZE / 2.0;
        (x, 1500.0 + ARENA_SIZE / 2.0)
    }
}

/// Every arena room, plus last results by player.
#[derive(Resource, Default)]
pub struct ArenaRooms {
    /// Next room id.
    next_id:   u32,
    /// Rooms by id.
    pub rooms: HashMap<u32, Room>,
    /// Last result shown to each player.
    results:   HashMap<u32, String>,
}

impl ArenaRooms {
    /// The room a player is in.
    pub fn room_of(&self, id: u32) -> Option<&Room> {
        self.rooms.values().find(|r| r.players.contains(&id))
    }
}

/// Whether two players are on the same team in a running arena fight.
/// `None` if neither is fighting in an arena.
pub fn arena_allies(world: &World, a: u32, b: u32) -> Option<bool> {
    let rooms = world.resource::<ArenaRooms>();
    let room_a = rooms.rooms.values().find(|r| r.started && r.players.contains(&a));
    let room_b = rooms.rooms.values().find(|r| r.started && r.players.contains(&b));
    match (room_a, room_b) {
        (None, None) => None,
        (Some(ra), Some(rb)) if ra.id == rb.id => Some(ra.team(a) == ra.team(b)),
        _ => Some(false),
    }
}

/// Rules of the arena a player is fighting in: (building damage, knockback).
pub fn arena_rules(world: &World, id: u32) -> Option<(f32, f32)> {
    world
        .resource::<ArenaRooms>()
        .rooms
        .values()
        .find(|r| r.started && r.alive.contains(&id))
        .map(|r| (r.object_damage, r.bounciness))
}

/// Sends room state to everyone.
pub fn send_state(world: &mut World) {
    let name = |world: &World, id: u32| {
        world
            .resource::<Sessions>()
            .0
            .get(&id)
            .map_or_default(|s| s.name.clone())
    };
    let rooms: Vec<ArenaRoomTO> = world
        .resource::<ArenaRooms>()
        .rooms
        .values()
        .map(|r| ArenaRoomTO {
            id:            r.id,
            host:          name(world, r.host),
            team_size:     r.team_size,
            players:       r
                .players
                .iter()
                .map(|&p| ClanMemberTO {
                    id:   p,
                    name: name(world, p),
                    role: u8::try_from(r.team(p).unwrap_or(0)).unwrap_or(0),
                })
                .collect(),
            started:       r.started,
            map:           r.map,
            max_health:    r.max_health,
            object_damage: r.object_damage,
            bounciness:    r.bounciness,
        })
        .collect();
    let recipients: Vec<u32> = world.resource::<Sessions>().0.keys().copied().collect();
    for id in recipients {
        let rooms_state = world.resource::<ArenaRooms>();
        let mine = rooms_state.room_of(id).map(|r| r.id);
        let result = rooms_state.results.get(&id).cloned().unwrap_or_default();
        world.resource_mut::<Outbox>().send_reliable(
            Target::Player(id),
            PacketType::ArenaState,
            &ArenaStateData {
                rooms: rooms.clone(),
                mine,
                result,
            },
        );
    }
}

/// Handles an arena action.
pub fn arena_action(world: &mut World, id: u32, msg: &ArenaMessage) {
    let in_room = world
        .resource::<ArenaRooms>()
        .room_of(id)
        .map(|r| (r.id, r.host, r.started));
    let error = match msg.action {
        // Create.
        0 => {
            let slots_used: Vec<u32> = world.resource::<ArenaRooms>().rooms.values().map(|r| r.slot).collect();
            if in_room.is_some() {
                Some("Leave your room first")
            } else if slots_used.len() >= MAX_ROOMS {
                Some("All arenas are busy")
            } else {
                let slot = (0..u32::try_from(MAX_ROOMS).unwrap_or(u32::MAX))
                    .find(|s| !slots_used.contains(s))
                    .unwrap_or(0);
                let mut rooms = world.resource_mut::<ArenaRooms>();
                rooms.next_id += 1;
                let room_id = rooms.next_id;
                rooms.rooms.insert(
                    room_id,
                    Room {
                        id: room_id,
                        host: id,
                        team_size: msg.team_size.clamp(1, 4),
                        map: msg.map.min(2),
                        max_health: msg.max_health.clamp(50.0, 500.0),
                        object_damage: msg.object_damage.clamp(0.0, 5.0),
                        bounciness: msg.bounciness.clamp(0.0, 3.0),
                        players: vec![id],
                        loadouts: HashMap::new(),
                        alive: Vec::new(),
                        started: false,
                        slot,
                        saved: HashMap::new(),
                        objects: Vec::new(),
                    },
                );
                None
            }
        }
        // Join.
        1 => {
            if in_room.is_some() {
                Some("Leave your room first")
            } else {
                let mut rooms = world.resource_mut::<ArenaRooms>();
                match rooms.rooms.get_mut(&msg.room) {
                    Some(r) if r.started => Some("That fight already started"),
                    Some(r) if r.players.len() >= usize::from(r.team_size) * 2 => Some("That room is full"),
                    Some(r) => {
                        r.players.push(id);
                        None
                    }
                    None => Some("That room doesn't exist"),
                }
            }
        }
        // Leave.
        2 => {
            leave(world, id);
            None
        }
        // Start.
        3 => match in_room {
            Some((room, host, false)) if host == id => start(world, room).err(),
            Some(_) => Some("Only the host can start"),
            None => Some("You're not in a room"),
        },
        // Loadout.
        4 => {
            let primary_ok = weapon(msg.primary).is_some_and(|w| w.slot == Slot::Primary);
            let secondary = (msg.secondary != u8::MAX).then_some(msg.secondary);
            let secondary_ok = secondary.is_none_or(|s| weapon(s).is_some_and(|w| w.slot == Slot::Secondary));
            match (in_room, primary_ok && secondary_ok) {
                (Some((room, _, false)), true) => {
                    if let Some(r) = world.resource_mut::<ArenaRooms>().rooms.get_mut(&room) {
                        r.loadouts.insert(id, (msg.primary, secondary));
                    }
                    None
                }
                (Some(_), false) => Some("Invalid loadout"),
                _ => Some("Join a room that hasn't started"),
            }
        }
        _ => None,
    };
    if let Some(text) = error {
        world.resource_mut::<Outbox>().notice(id, text, 1);
    }
    send_state(world);
}

/// Spawns the arena's walls and obstacles.
fn build_arena(world: &mut World, room: &Room) -> Vec<Entity> {
    let (cx, cy) = room.center();
    let half = ARENA_SIZE / 2.0;
    let mut objects = vec![
        world
            .spawn((
                Position(cx, cy - half - 50.0),
                Collider::rect(half + 100.0, 50.0),
                NonReactiveCollider,
            ))
            .id(),
        world
            .spawn((
                Position(cx, cy + half + 50.0),
                Collider::rect(half + 100.0, 50.0),
                NonReactiveCollider,
            ))
            .id(),
        world
            .spawn((
                Position(cx - half - 50.0, cy),
                Collider::rect(50.0, half + 100.0),
                NonReactiveCollider,
            ))
            .id(),
        world
            .spawn((
                Position(cx + half + 50.0, cy),
                Collider::rect(50.0, half + 100.0),
                NonReactiveCollider,
            ))
            .id(),
    ];
    let rocks: Vec<(f32, f32, f32)> = match room.map {
        // Pillars: a grid of big stones.
        1 => (0..4)
            .flat_map(|i| (0..4).map(move |j| (i, j)))
            .map(|(i, j)| {
                #[allow(clippy::cast_precision_loss)]
                let (fi, fj) = (i as f32, j as f32);
                (fi.mul_add(480.0, cx - 720.0), fj.mul_add(480.0, cy - 720.0), 70.0)
            })
            .collect(),
        // Maze: rows of stones with gaps.
        2 => (0..5)
            .flat_map(|row| (0..12).map(move |col| (row, col)))
            .filter(|(row, col)| (col + row * 3) % 5 != 0)
            .map(|(row, col)| {
                #[allow(clippy::cast_precision_loss)]
                let (fr, fc) = (row as f32, col as f32);
                (fc.mul_add(190.0, cx - 1045.0), fr.mul_add(400.0, cy - 800.0), 55.0)
            })
            .collect(),
        _ => Vec::new(),
    };
    for (x, y, r) in rocks {
        objects.push(
            world
                .spawn(SolidObjectBundle {
                    object: ObjectBundle {
                        object:   ObjectEntity,
                        kind:     ObjectKind::Stone,
                        pos:      Position(x, y),
                        dir:      AimDir(0.0),
                        health:   Health(1.0, 1.0),
                        collider: Collider::circle(r),
                    },
                    solid:  NonReactiveCollider,
                })
                .id(),
        );
    }
    objects
}

/// Starts a fight: saves everyone's state and moves them into the arena.
fn start(world: &mut World, room_id: u32) -> Result<(), &'static str> {
    let Some(mut room) = world.resource::<ArenaRooms>().rooms.get(&room_id).cloned() else {
        return Err("That room doesn't exist");
    };
    if room.players.len() < usize::from(room.team_size) * 2 {
        return Err("Wait for the room to fill up");
    }
    if room.players.iter().any(|&p| player_entity(world, p).is_none()) {
        return Err("Everyone needs to be alive to start");
    }

    room.objects = build_arena(world, &room);
    let (cx, cy) = room.center();
    for &id in &room.players.clone() {
        let Some(entity) = player_entity(world, id) else {
            continue;
        };
        let saved = Saved {
            pos:      world.get::<Position>(entity).copied().unwrap_or(Position(0.0, 0.0)),
            health:   world.get::<Health>(entity).copied().unwrap_or(Health(100.0, 100.0)),
            res:      world.get::<Resources>(entity).copied().unwrap_or_default(),
            progress: world.get::<Progress>(entity).copied().unwrap_or_default(),
            loadout:  world
                .get::<Loadout>(entity)
                .cloned()
                .unwrap_or_else(|| Loadout(Owned::starting())),
            wxp:      world.get::<WeaponXp>(entity).cloned().unwrap_or_default(),
            weapon:   world.get::<Weapon>(entity).copied().unwrap_or(Weapon(0)),
        };
        room.saved.insert(id, saved);

        let team = room.team(id).unwrap_or(0);
        #[allow(clippy::cast_precision_loss)]
        let side: f32 = if team == 0 { -1.0 } else { 1.0 };
        #[allow(clippy::cast_precision_loss)]
        let offset = (room.players.iter().position(|&p| p == id).unwrap_or(0) % usize::from(room.team_size)) as f32;
        let spawn = (side.mul_add(ARENA_SIZE * 0.4, cx), offset.mul_add(150.0, cy - 225.0));

        let (primary, secondary) = room.loadouts.get(&id).copied().unwrap_or((0, None));
        let mut owned = Owned::starting();
        owned.primary = primary;
        owned.secondary = secondary;
        crate::systems::cave_system::surface(world, entity);
        world.entity_mut(entity).insert((
            Position(spawn.0, spawn.1),
            Velocity(0.0, 0.0),
            Health(room.max_health, room.max_health),
            Resources {
                wood:  500,
                stone: 500,
                food:  500,
                gold:  0,
                ammo:  300,
            },
            Loadout(owned),
            InArena(room_id),
            ReloadState(0, 300),
        ));
        switch_weapon(world, entity, primary);
        refresh_player(world, entity);
    }
    room.alive.clone_from(&room.players);
    room.started = true;
    world.resource_mut::<ArenaRooms>().rooms.insert(room_id, room);
    Ok(())
}

/// Moves a player back to the main world with their saved state.
fn restore(world: &mut World, id: u32, saved: Saved) {
    let Some(entity) = player_entity(world, id) else {
        return;
    };
    world.entity_mut(entity).insert((
        saved.pos,
        Velocity(0.0, 0.0),
        saved.health,
        saved.res,
        saved.progress,
        saved.loadout,
        saved.wxp,
        saved.weapon,
    ));
    world.entity_mut(entity).remove::<InArena>();
    refresh_player(world, entity);
}

/// A fighter went down: they go back to the main world, and the fight ends
/// if their team is out.
pub fn fighter_down(world: &mut World, room_id: u32, id: u32) {
    let saved = world
        .resource_mut::<ArenaRooms>()
        .rooms
        .get_mut(&room_id)
        .and_then(|room| {
            room.alive.retain(|&p| p != id);
            room.saved.remove(&id)
        });
    if let Some(saved) = saved {
        restore(world, id, saved);
    }
    world
        .resource_mut::<Outbox>()
        .notice(id, "You were knocked out of the arena", 1);
    check_end(world, room_id);
}

/// Ends the fight if one team has nobody left.
fn check_end(world: &mut World, room_id: u32) {
    let Some(room) = world.resource::<ArenaRooms>().rooms.get(&room_id).cloned() else {
        return;
    };
    if !room.started {
        return;
    }
    let alive_teams: Vec<usize> = room.alive.iter().filter_map(|&p| room.team(p)).collect();
    let team0 = alive_teams.contains(&0);
    let team1 = alive_teams.contains(&1);
    if team0 && team1 {
        return;
    }
    let result = match (team0, team1) {
        (true, false) => "Team 1 wins!",
        (false, true) => "Team 2 wins!",
        _ => "Draw!",
    };

    for &id in &room.players {
        if let Some(saved) = room.saved.get(&id).cloned().filter(|_| room.alive.contains(&id)) {
            restore(world, id, saved);
        }
        world
            .resource_mut::<ArenaRooms>()
            .results
            .insert(id, result.to_string());
        world.resource_mut::<Outbox>().notice(id, format!("Arena: {result}"), 2);
    }
    for e in room.objects {
        if world.get::<ObjectEntity>(e).is_some() {
            remove_object(world, e);
        } else {
            world.despawn(e);
        }
    }
    world.resource_mut::<ArenaRooms>().rooms.remove(&room_id);
    send_state(world);
}

/// Removes a player from their room (restoring them if they were fighting).
pub fn leave(world: &mut World, id: u32) {
    let Some(room) = world.resource::<ArenaRooms>().room_of(id).cloned() else {
        return;
    };
    if room.started {
        if let Some(saved) = room.saved.get(&id).cloned() {
            restore(world, id, saved);
        }
    }
    let mut rooms = world.resource_mut::<ArenaRooms>();
    if let Some(r) = rooms.rooms.get_mut(&room.id) {
        r.players.retain(|&p| p != id);
        r.alive.retain(|&p| p != id);
        r.saved.remove(&id);
        if r.host == id {
            if let Some(&next) = r.players.first() {
                r.host = next;
            }
        }
    }
    let empty = rooms.rooms.get(&room.id).is_some_and(|r| r.players.is_empty());
    if empty && !room.started {
        rooms.rooms.remove(&room.id);
    }
    if room.started {
        check_end(world, room.id);
    }
    send_state(world);
}
