use bevy_ecs::{
    entity::Entity,
    query::With,
    resource::Resource,
    system::{Query, Res, ResMut},
};
use rstar::{RTree, RTreeObject, AABB};

use crate::{
    structs::{
        bevy::PlayerMap,
        components::{AnimalEntity, AnimalType, Position, Velocity},
        spatial_index::{EntityPoint, GlobalSpatialIndex},
    },
    systems::Collider,
};

/// System that will build the spatial hash once per tick.
pub fn build_spatial_index_system(
    player_map: Res<PlayerMap>,
    all_positions: Query<(Entity, &Position, &Collider)>,
    mut index: ResMut<GlobalSpatialIndex>,
) {
    let entity_to_player_id: std::collections::HashMap<Entity, u32> =
        player_map.map.iter().map(|e| (*e.1, *e.0)).collect();

    // Maps all ECS entity positions to `EntityPoint`.
    let points: Vec<EntityPoint> = all_positions
        .iter()
        .map(|(entity, pos, col)| EntityPoint {
            pos:          [pos.0, pos.1],
            entity_index: entity.index(),
            player_id:    entity_to_player_id.get(&entity).copied(),
            radius:       col.rad,
        })
        .collect();

    // Add all the points to the tree.
    index.tree = RTree::bulk_load(points);
}

/// A single fish represented as a point in the R-tree.
/// `index` is the position of this fish inside `FishSpatialIndex::snapshots`.
#[derive(Clone, Copy)]
pub struct FishPoint {
    pub pos:   [f32; 2],
    pub index: usize,
}

impl RTreeObject for FishPoint {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.pos)
    }
}

#[derive(Resource, Default)]
pub struct FishSpatialIndex {
    pub snapshots: Vec<(Velocity, Position, AnimalType)>,
    pub tree:      RTree<FishPoint>,
}

/// Rebuilds `FishSpatialIndex` every tick from the live ECS fish entities.
pub fn build_fish_spatial_index_system(
    query: Query<(&Position, &Velocity, &AnimalType), With<AnimalEntity>>,
    mut fish_index: ResMut<FishSpatialIndex>,
) {
    // Collect (velocity, position, type) snapshots for every fish.
    let snapshots: Vec<(Velocity, Position, AnimalType)> = query
        .iter()
        .filter_map(|(pos, vel, atype)| {
            if matches!(atype, AnimalType::Fish) {
                Some((*vel, *pos, *atype))
            } else {
                None
            }
        })
        .collect();

    // Build the R-tree so boid neighbours can be found in O(log n).
    let points: Vec<FishPoint> = snapshots
        .iter()
        .enumerate()
        .map(|(i, (_, pos, _))| FishPoint {
            pos:   [pos.0, pos.1],
            index: i,
        })
        .collect();

    fish_index.snapshots = snapshots;
    fish_index.tree = RTree::bulk_load(points);
}
