//! Animals in view.

use bevy_ecs::{entity::Entity, world::World};
use shared::{
    to_client::{AnimalTO, AnimalsData},
    PacketType,
};

use crate::{
    replication::{chunk_by_size, in_view},
    structs::{
        components::{AnimalEntity, Health, Position, Velocity},
        outbox::{Outbox, Target},
    },
};

/// Sends each player the animals in their view.
pub fn replicate_animals(world: &mut World, positions: &[(u32, f32, f32)]) {
    let all: Vec<AnimalTO> = world
        .query::<(Entity, &AnimalEntity, &Position, &Velocity, &Health)>()
        .iter(world)
        .map(|(e, a, p, v, h)| AnimalTO {
            id:     e.index(),
            x:      p.0,
            y:      p.1,
            dir:    v.1.atan2(v.0),
            kind:   a.0,
            health: (h.0 / h.1).clamp(0.0, 1.0),
        })
        .collect();

    let mut out = world.resource_mut::<Outbox>();
    for &(id, px, py) in positions {
        let visible: Vec<AnimalTO> = all.iter().filter(|a| in_view(px, py, a.x, a.y)).cloned().collect();
        for animals in chunk_by_size(&visible) {
            out.send(Target::Player(id), PacketType::Animals, &AnimalsData { animals });
        }
    }
}
