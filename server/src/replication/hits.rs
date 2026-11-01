//! Attack animations.

use bevy_ecs::world::World;
use shared::{to_client::HitEventTO, PacketType};

use crate::structs::{
    bevy::PlayerMap,
    components::{AttackAnims, Position},
    outbox::{Outbox, Target},
};

/// Tells nearby players who attacked this tick.
pub fn replicate_attacks(world: &mut World) {
    let attackers = std::mem::take(&mut world.resource_mut::<AttackAnims>().0);
    for id in attackers {
        let Some(&entity) = world.resource::<PlayerMap>().map.get(&id) else {
            continue;
        };
        let Some(pos) = world.get::<Position>(entity).copied() else {
            continue;
        };
        world.resource_mut::<Outbox>().send(
            Target::Near(pos.0, pos.1),
            PacketType::HitEvent,
            &HitEventTO { entity_id: id },
        );
    }
}
