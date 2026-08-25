use shared::{to_client::ObjectTO, to_server::ClientMessages};

use crate::{
    broadcast,
    structs::{
        bevy::{IDToConnection, World},
        visibility::{ObjSnapshot, PlayerKnowledge},
    },
};

pub fn replicate_object(
    obj: &[ObjSnapshot],
    player_id: &u32,
    (px, py): (&f32, &f32),
    world: &mut bevy_ecs::world::World,
    rt_handle: &tokio::runtime::Handle,
    player_connections: &IDToConnection,
) {
    let new_objects: Vec<ObjectTO> = {
        let mut knowledge = world.resource_mut::<PlayerKnowledge>();
        knowledge
            .drain_new_objects(*player_id, obj.iter(), *px, *py)
            .into_iter()
            .map(|o| ObjectTO {
                id:       o.id,
                x:        o.x,
                y:        o.y,
                dir:      o.dir,
                scale:    o.scale,
                type_obj: o.type_obj,
            })
            .collect()
    };

    if !new_objects.is_empty() {
        let msg = crate::net::serialization::encode(
            7,
            ClientMessages::AddObject(shared::to_client::AddObjectData { objects: new_objects }),
        )
        .unwrap();

        broadcast!(reliable_to, rt_handle, player_connections, *player_id, msg);
    }
}
