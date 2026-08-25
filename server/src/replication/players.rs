use shared::to_client::{PlayerTO, UpdatePlayerData};

use crate::{
    broadcast,
    structs::{
        bevy::{IDToConnection, PlayerMap, World},
        components::{AimDir, Health, Name, Position},
    },
};

pub fn replicate_players(
    world: &bevy_ecs::world::World,
    player_map: &PlayerMap,
    rt_handle: &tokio::runtime::Handle,
    player_connections: &IDToConnection,
) {
    let updates: Vec<PlayerTO> = player_map
        .map
        .iter()
        .filter_map(|(i, &entity)| {
            let name = world.get::<Name>(entity)?;
            let pos = world.get::<Position>(entity)?;
            let aim = world.get::<AimDir>(entity)?;
            let health = world.get::<Health>(entity)?;
            Some(PlayerTO {
                id:           *i,
                name:         name.0.clone(),
                x:            pos.0,
                y:            pos.1,
                aim:          aim.0,
                health:       health.0,
                weapon_index: Some(0),
            })
        })
        .collect();

    if !updates.is_empty() {
        let msg = crate::net::serialization::encode(3, UpdatePlayerData { players: updates }).unwrap();
        broadcast!(rt_handle, player_connections, msg);
    }
}
