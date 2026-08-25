use bevy_ecs::entity::Entity;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use shared::to_client::{HitEventTO, ObjectHitAnimData, SetResourceData};

use crate::{
    broadcast,
    structs::{
        bevy::{IDToConnection, PlayerMap, World},
        components::{HitEvent, Resources},
    },
    CONFIG,
};

pub fn replicate_hits(
    hits: &Vec<HitEvent>,
    world: &mut bevy_ecs::world::World,
    player_positions: &Vec<(u32, f32, f32)>,
    rt_handle: &tokio::runtime::Handle,
    player_connections: &IDToConnection,
) {
    let player_map = world.resource::<PlayerMap>();
    let entity_to_id: std::collections::HashMap<Entity, u32> = player_map.map.iter().map(|e| (*e.1, *e.0)).collect();

    hits.par_iter().for_each(|hit| {
        let origin = (hit.attacker_pos.0, hit.attacker_pos.1);

        if let Some(&player_id) = entity_to_id.get(&hit.attacker) {
            let msg = crate::net::serialization::encode(6, HitEventTO { entity_id: player_id }).unwrap();

            for &(pid, px, py) in player_positions {
                let dx = px - origin.0;
                let dy = py - origin.1;
                if dx * dx + dy * dy <= CONFIG.players.view_radius_sq {
                    broadcast!(to, rt_handle, player_connections, pid, msg.clone());
                }
            }
        }

        for (obj_entity, angle) in &hit.object_hits {
            let msg = crate::net::serialization::encode(
                10,
                ObjectHitAnimData {
                    id:  obj_entity.index(),
                    dir: *angle,
                },
            )
            .unwrap();

            // Do the same for object hits
            for &(pid, px, py) in player_positions {
                let dx = px - origin.0;
                let dy = py - origin.1;
                if dx * dx + dy * dy <= CONFIG.players.view_radius_sq {
                    broadcast!(to, rt_handle, player_connections, pid, msg.clone());
                }
            }
        }

        if let Some(&player_id) = entity_to_id.get(&hit.attacker) {
            if let Some(&entity) = world.resource::<PlayerMap>().map.get(&player_id) {
                if let Some(res) = world.get::<Resources>(entity) {
                    let msg = crate::net::serialization::encode(
                        11,
                        SetResourceData {
                            wood:  res.0,
                            stone: res.1,
                            food:  res.2,
                        },
                    )
                    .unwrap();
                    broadcast!(to, rt_handle, player_connections, player_id, msg);
                }
            }
        }
    });
}
