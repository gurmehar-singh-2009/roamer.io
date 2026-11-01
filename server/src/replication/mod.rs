//! Sending game state to clients.

/// Animals in view.
pub mod animals;
/// Player health updates.
pub mod health;
/// Attack animations.
pub mod hits;
/// Objects coming into view.
pub mod objects;
/// Player positions.
pub mod players;
/// Per-player state: resources, loadout, age, upgrades, shop.
pub mod upgrades;

use bevy_ecs::{entity::Entity, world::World};
use shared::to_client::PlayerTO;

use crate::{
    structs::{
        bevy::IDToConnection,
        components::{AimDir, Gear, Health, PlayerEntity, Position, Weapon, WeaponXp},
        outbox::{Outbox, Target},
        visibility::PlayerKnowledge,
    },
    systems::{cave_system::Underground, player_system::Sessions},
    CONFIG,
};

/// Max bytes in one datagram body (QUIC datagrams must fit in a packet).
pub const DATAGRAM_BUDGET: usize = 1000;

/// A player's state as sent to clients.
pub fn player_to(world: &World, entity: Entity) -> Option<PlayerTO> {
    let id = world.get::<PlayerEntity>(entity)?.0;
    let pos = world.get::<Position>(entity)?;
    let health = world.get::<Health>(entity)?;
    let weapon = world.get::<Weapon>(entity)?.0;
    let gear = world.get::<Gear>(entity)?;
    let variant = world.get::<WeaponXp>(entity).map_or(0, |x| x.variant(weapon));
    let clan = world.resource::<Sessions>().0.get(&id).and_then(|s| s.clan);
    Some(PlayerTO {
        id,
        x: pos.0,
        y: pos.1,
        aim: world.get::<AimDir>(entity).map_or(0.0, |a| a.0),
        health: health.0,
        max_health: health.1,
        weapon,
        variant,
        hat: gear.hat,
        accessory: gear.accessory,
        skin: gear.skin,
        clan,
        underground: world.get::<Underground>(entity).is_some(),
    })
}

/// Positions of living players: (id, x, y).
pub fn player_positions(world: &mut World) -> Vec<(u32, f32, f32)> {
    world
        .query::<(&PlayerEntity, &Position)>()
        .iter(world)
        .map(|(p, pos)| (p.0, pos.0, pos.1))
        .collect()
}

/// Ids of the players in the caves.
pub fn underground_players(world: &mut World) -> std::collections::HashSet<u32> {
    world
        .query_filtered::<&PlayerEntity, bevy_ecs::query::With<Underground>>()
        .iter(world)
        .map(|p| p.0)
        .collect()
}

/// Whether a point is in view of a player at (px, py).
pub fn in_view(px: f32, py: f32, x: f32, y: f32) -> bool {
    let (dx, dy) = (x - px, y - py);
    // A little extra so things don't pop in at the screen edge.
    dx.mul_add(dx, dy * dy) <= CONFIG.players.view_radius_sq * 1.2
}

/// Sends one message to one connection.
fn deliver(conns: &IDToConnection, id: u32, bytes: &[u8], reliable: bool) {
    let Some(conn) = conns.get(&id) else {
        return;
    };
    let result = if reliable {
        conn.send_reliable(bytes)
    } else {
        conn.send_encrypted(bytes)
    };
    if let Err(e) = result {
        tracing::debug!("send to {} failed: {}", id, e);
    }
}

/// Sends everything queued in the outbox.
pub fn flush(world: &mut World, conns: &IDToConnection) {
    let messages = std::mem::take(&mut world.resource_mut::<Outbox>().0);
    if messages.is_empty() {
        return;
    }
    let positions = player_positions(world);
    let underground = underground_players(world);
    let connected: Vec<u32> = conns.iter().map(|c| *c.key()).collect();

    for msg in messages {
        let recipients: Vec<u32> = match msg.target {
            Target::Player(id) => vec![id],
            Target::All => connected.clone(),
            Target::AllExcept(skip) => connected.iter().copied().filter(|&id| id != skip).collect(),
            Target::Near(x, y) => positions
                .iter()
                .filter(|&&(_, px, py)| in_view(px, py, x, y))
                .map(|&(id, _, _)| id)
                .collect(),
            Target::NearLayer(x, y, under) => positions
                .iter()
                .filter(|&&(id, px, py)| underground.contains(&id) == under && in_view(px, py, x, y))
                .map(|&(id, _, _)| id)
                .collect(),
            Target::KnowsObject(obj) => world.resource::<PlayerKnowledge>().players_knowing(obj).collect(),
            Target::ForgetObject(obj) => world.resource_mut::<PlayerKnowledge>().forget_object(obj),
        };
        for id in recipients {
            deliver(conns, id, &msg.bytes, msg.reliable);
        }
    }
}

/// Splits `items` into chunks whose encoded size stays under the datagram
/// budget.
pub fn chunk_by_size<T: borsh::BorshSerialize + Clone>(items: &[T]) -> Vec<Vec<T>> {
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let item_size = borsh::to_vec(first).map_or(64, |v| v.len()).max(1) + 8;
    let per_chunk = (DATAGRAM_BUDGET / item_size).max(1);
    items.chunks(per_chunk).map(<[T]>::to_vec).collect()
}

/// Sends the per-tick view: nearby players, animals and new objects.
pub fn replicate_view(world: &mut World) {
    let positions = player_positions(world);
    players::replicate_players(world, &positions);
    animals::replicate_animals(world, &positions);
    objects::replicate_objects(world, &positions);
}
