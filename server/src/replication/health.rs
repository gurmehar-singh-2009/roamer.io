use shared::to_client::UpdateHealthData;

use crate::{
    broadcast,
    structs::{
        bevy::{IDToConnection, PlayerMap, World},
        components::Name,
        visibility::PlayerKnowledge,
    },
    CONFIG,
};

pub fn replicate_health(
    world: &mut bevy_ecs::world::World,
    mut updates: Vec<(u32, f32)>,
    player_positions: &Vec<(u32, f32, f32)>,
    rt_handle: &tokio::runtime::Handle,
    player_connections: &IDToConnection,
) {
    for (player_id, new_health) in updates.drain(..) {
        // Try to find the origin position for the player so we can limit
        // visibility; if missing, fall back to broadcasting to all players.
        if let Some(pos) = player_positions.iter().find(|(i, _, _)| *i == player_id) {
            let origin_x = pos.1;
            let origin_y = pos.2;

            let msg = crate::net::serialization::encode(
                8,
                UpdateHealthData {
                    id: player_id,
                    new_health,
                },
            )
            .unwrap();

            for &(viewer_id, vx, vy) in player_positions {
                let dx = vx - origin_x;
                let dy = vy - origin_y;
                if dx * dx + dy * dy <= CONFIG.players.view_radius_sq {
                    broadcast!(to, rt_handle, player_connections, viewer_id, msg.clone());
                }
            }
        } else {
            let msg = crate::net::serialization::encode(
                8,
                UpdateHealthData {
                    id: player_id,
                    new_health,
                },
            )
            .unwrap();
            broadcast!(rt_handle, player_connections, msg);
        }

        // If the player's health hit zero, notify clients and remove their entity.
        if new_health <= 0.0 {
            // Broadcast a remove-player message (opcode 2) to all clients.
            if let Ok(remove_msg) = crate::net::serialization::encode(2u8, player_id) {
                broadcast!(rt_handle, player_connections, remove_msg);
            }

            // Send a death message (opcode 12) to the dead player's reliable channel so
            // their client can return to the home screen.
            if let Ok(death_msg) = crate::net::serialization::encode(12u8, player_id) {
                broadcast!(reliable_to, rt_handle, player_connections, player_id, death_msg);
            }

            // Despawn their entity and clean up server-side state.
            if let Some(e) = world.resource_mut::<PlayerMap>().map.remove(&player_id) {
                if let Some(p) = world.get::<Name>(e) {
                    tracing::info!("player {} ({}) died", p.0, player_id);
                }
                world.despawn(e);
            }
            world.resource_mut::<PlayerKnowledge>().remove_player(player_id);
        }
    }
}
