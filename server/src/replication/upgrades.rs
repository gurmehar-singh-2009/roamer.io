use bevy_ecs::entity::Entity;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use shared::to_client::SetUpgradesData;

use crate::{
    broadcast,
    structs::{
        bevy::{IDToConnection, PlayerMap, World},
        components::UpgradeEvent,
    },
};

pub fn replicate_upgrades(
    upgrades: Vec<UpgradeEvent>,
    world: &mut bevy_ecs::world::World,
    rt_handle: &tokio::runtime::Handle,
    player_connections: &IDToConnection,
) {
    let player_map = world.resource::<PlayerMap>();
    let entity_to_id: std::collections::HashMap<Entity, u32> = player_map.map.iter().map(|e| (*e.1, *e.0)).collect();

    upgrades.par_iter().for_each(|event| {
        if let Some(&player_id) = entity_to_id.get(&event.id) {
            let msg = crate::net::serialization::encode(
                14,
                SetUpgradesData {
                    upgrades: event.weapons.iter().map(|w| *w as u8).collect(),
                },
            )
            .unwrap();

            broadcast!(reliable_to, rt_handle, player_connections, player_id, msg);
        }
    });
}
