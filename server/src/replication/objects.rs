//! Objects coming into view.

use bevy_ecs::{entity::Entity, world::World};
use shared::{
    to_client::{AddObjectData, ObjectTO},
    PacketType,
};

use crate::{
    catalog::items::{item_def, ItemKind},
    replication::in_view,
    structs::{
        components::{AimDir, Building, Health, ObjectEntity, ObjectKind, Position},
        outbox::{Outbox, Target},
        visibility::PlayerKnowledge,
    },
    systems::{social_system::are_allies, Collider},
};

/// Owner id sent for objects nobody owns.
pub const NO_OWNER: u32 = u32::MAX;

/// Snapshot of every map object.
pub fn object_snapshots(world: &mut World) -> Vec<ObjectTO> {
    world
        .query::<(
            Entity,
            &ObjectEntity,
            &ObjectKind,
            &Position,
            &AimDir,
            &Collider,
            &Health,
            Option<&Building>,
        )>()
        .iter(world)
        .map(|(e, _, kind, pos, dir, col, health, building)| ObjectTO {
            id:         e.index(),
            x:          pos.0,
            y:          pos.1,
            dir:        dir.0,
            scale:      col.rad,
            type_obj:   kind.type_id(),
            health:     health.0,
            max_health: health.1,
            owner:      building.map_or(NO_OWNER, |b| b.owner),
        })
        .collect()
}

/// Whether a player may see an object. Pit traps are hidden from enemies.
fn may_see(world: &World, player: u32, obj: &ObjectTO) -> bool {
    let is_trap = obj
        .type_obj
        .checked_sub(50)
        .and_then(item_def)
        .is_some_and(|i| i.kind == ItemKind::PitTrap);
    !is_trap || obj.owner == player || are_allies(world, player, obj.owner)
}

/// Sends each player the objects that just came into their view.
pub fn replicate_objects(world: &mut World, positions: &[(u32, f32, f32)]) {
    let all = object_snapshots(world);

    let mut sends = Vec::new();
    for &(id, px, py) in positions {
        let allowed: Vec<bool> = all.iter().map(|o| may_see(world, id, o)).collect();
        let new_objects = world.resource_mut::<PlayerKnowledge>().drain_new_objects(
            id,
            all.iter().enumerate().filter(|(i, _)| allowed[*i]).map(|(_, o)| o),
            |o| in_view(px, py, o.x, o.y),
        );
        if !new_objects.is_empty() {
            sends.push((id, new_objects));
        }
    }

    let mut out = world.resource_mut::<Outbox>();
    for (id, objects) in sends {
        out.send_reliable(Target::Player(id), PacketType::AddObject, &AddObjectData { objects });
    }
}
