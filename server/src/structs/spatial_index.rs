use bevy_ecs::prelude::Resource;
use rstar::{RTree, RTreeObject, AABB};

/// A point in the global spatial index covering every entity with a position.
/// The envelope is sized to the entity's collision radius so that
/// `locate_in_envelope_intersecting` correctly finds potentially-overlapping
/// pairs without any extra distance math at query time.
#[derive(Clone, Copy)]
pub struct EntityPoint {
    /// Position.
    pub pos:          [f32; 2],
    /// Entity index.
    pub entity_index: u32,
    /// Only Some if this entity is a connected player.
    #[allow(dead_code)]
    pub player_id:    Option<u32>,
    /// Collider radius.
    pub radius:       f32,
}

impl RTreeObject for EntityPoint {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        // Expand the envelope by the entity's radius in all directions.
        // This means "does my bounding box overlap yours?" becomes a correct
        // broad-phase for "could we be colliding?"
        AABB::from_corners(
            [self.pos[0] - self.radius, self.pos[1] - self.radius],
            [self.pos[0] + self.radius, self.pos[1] + self.radius],
        )
    }
}

#[derive(Resource, Default)]
/// R-tree of every entity with a position and collider. Rebuilt every tick.
pub struct GlobalSpatialIndex {
    /// The tree.
    pub tree: RTree<EntityPoint>,
}

impl GlobalSpatialIndex {
    /// Returns player IDs within `radius` world-units of `origin`.
    /// The bounding-box query is already tight because of fat envelopes,
    /// so we still do a secondary exact distance check to filter the corners
    /// of the AABB that aren't actually within the circle.
    #[allow(dead_code)]
    pub fn nearby_player_ids(&self, origin: (f32, f32), radius: f32) -> Vec<u32> {
        let aabb = AABB::from_corners(
            [origin.0 - radius, origin.1 - radius],
            [origin.0 + radius, origin.1 + radius],
        );
        let radius_sq = radius * radius;

        self.tree
            .locate_in_envelope_intersecting(&aabb)
            .filter_map(|pt| {
                let id = pt.player_id?;
                let dx = pt.pos[0] - origin.0;
                let dy = pt.pos[1] - origin.1;
                if dy.mul_add(dy, dx * dx) <= radius_sq {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }
}
