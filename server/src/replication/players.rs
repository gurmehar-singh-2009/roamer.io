//! Player positions.

use bevy_ecs::world::World;
use shared::{
    to_client::{PlayerTO, UpdatePlayerData},
    PacketType,
};

use crate::{
    replication::{chunk_by_size, in_view, player_to, underground_players},
    structs::{
        bevy::PlayerMap,
        outbox::{Outbox, Target},
    },
};

/// Sends each player the state of the players in their view.
pub fn replicate_players(world: &mut World, positions: &[(u32, f32, f32)]) {
    let all: Vec<PlayerTO> = world
        .resource::<PlayerMap>()
        .map
        .values()
        .filter_map(|&e| player_to(world, e))
        .collect();
    let underground = underground_players(world);

    let mut out = world.resource_mut::<Outbox>();
    for &(id, px, py) in positions {
        // Players in the caves and on the surface can't see each other.
        let layer = underground.contains(&id);
        let visible: Vec<PlayerTO> = all
            .iter()
            .filter(|p| p.id == id || (p.underground == layer && in_view(px, py, p.x, p.y)))
            .cloned()
            .collect();
        for players in chunk_by_size(&visible) {
            out.send(
                Target::Player(id),
                PacketType::UpdatePlayers,
                &UpdatePlayerData { players },
            );
        }
    }
}
