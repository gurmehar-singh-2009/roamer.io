use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{With, Without},
    system::{Query, Res},
};
use nanorand::{Rng, WyRand};
use rapier2d::{
    math::{Isometry, Vector},
    parry::{
        query::contact,
        shape::{Ball, Cuboid},
    },
    prelude::SharedShape,
};
use rstar::AABB;

use crate::structs::{
    components::{AiTarget, Position, Velocity},
    spatial_index::GlobalSpatialIndex,
};

static COLLISION_SEED: AtomicU64 = AtomicU64::new(0);

/// The collider struct contained in ECS bundles.
#[derive(Component, Clone)]
pub struct Collider {
    pub shape:        SharedShape,
    pub rad:          f32,
    pub half_extents: (f32, f32),
}

impl Collider {
    pub fn circle(rad: f32) -> Self {
        Self {
            shape: SharedShape::new(Ball::new(rad)),
            rad,
            half_extents: (rad, rad),
        }
    }

    pub fn rect(half_w: f32, half_h: f32) -> Self {
        Self {
            shape:        SharedShape::new(Cuboid::new(Vector::new(half_w, half_h))),
            rad:          half_w.max(half_h),
            half_extents: (half_w, half_h),
        }
    }
}

#[derive(Component)]
pub struct NonReactiveCollider;

#[derive(Component)]
pub struct ReactiveCollider;

// I don't know how you could make them their own `type`, tried but didn't work
// well.
#[allow(clippy::type_complexity)]
pub fn collision_resolution_system(
    index: Res<GlobalSpatialIndex>,
    config: Res<crate::config::Config>,
    mut reactive_query: Query<
        (Entity, &mut Position, &mut Velocity, &Collider, Option<&mut AiTarget>),
        With<ReactiveCollider>,
    >,
    non_reactive_query: Query<(Entity, &Position, &Collider), (With<NonReactiveCollider>, Without<ReactiveCollider>)>,
) {
    let precision = 0.1;

    // Reactive vs Reactive

    let mut combinations = reactive_query.iter_combinations_mut();
    while let Some([(_, mut p1, mut v1, c1, _), (_, mut p2, mut v2, c2, _)]) = combinations.fetch_next() {
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let combined_rad = c1.rad + c2.rad;

        if dx * dx + dy * dy < combined_rad * combined_rad {
            let iso1 = Isometry::translation(p1.0, p1.1);
            let iso2 = Isometry::translation(p2.0, p2.1);

            if let Ok(Some(contact)) = contact(&iso1, c1.shape.as_ref(), &iso2, c2.shape.as_ref(), precision) {
                if contact.dist < 0.0 {
                    let normal = contact.normal1;
                    let separation = contact.dist.abs() * 0.5;
                    p1.0 -= normal.x * separation;
                    p1.1 -= normal.y * separation;
                    p2.0 += normal.x * separation;
                    p2.1 += normal.y * separation;

                    let v1n = v1.0 * normal.x + v1.1 * normal.y;
                    if v1n > 0.0 {
                        v1.0 -= normal.x * v1n;
                        v1.1 -= normal.y * v1n;
                    }
                    let v2n = v2.0 * normal.x + v2.1 * normal.y;
                    if v2n < 0.0 {
                        v2.0 -= normal.x * v2n;
                        v2.1 -= normal.y * v2n;
                    }
                }
            }
        }
    }

    // Reactive vs Non-Reactive

    // Collect non-reactive entities into an owned HashMap so we can share it.
    let non_reactives: std::collections::HashMap<u32, ((f32, f32), Collider)> = non_reactive_query
        .iter()
        .map(|(entity, pos, col)| (entity.index(), ((pos.0, pos.1), col.clone())))
        .collect();

    let index_ref: &GlobalSpatialIndex = &index;

    reactive_query
        .par_iter_mut()
        .for_each(|(_, mut pos, mut vel, col, mut target_opt)| {
            let query_aabb = AABB::from_corners([pos.0 - col.rad, pos.1 - col.rad], [pos.0 + col.rad, pos.1 + col.rad]);

            for point in index_ref.tree.locate_in_envelope_intersecting(&query_aabb) {
                // Skip anything not in the non-reactive set (players, animals etc.).
                let Some(((wx, wy), wall_col)) = non_reactives.get(&point.entity_index) else {
                    continue;
                };

                let dx = wx - pos.0;
                let dy = wy - pos.1;
                let combined_x = col.half_extents.0 + wall_col.half_extents.0;
                let combined_y = col.half_extents.1 + wall_col.half_extents.1;

                if dx.abs() < combined_x && dy.abs() < combined_y {
                    let iso_a = Isometry::translation(pos.0, pos.1);
                    let iso_b = Isometry::translation(*wx, *wy);

                    if let Ok(Some(contact)) =
                        contact(&iso_a, col.shape.as_ref(), &iso_b, wall_col.shape.as_ref(), precision)
                    {
                        if contact.dist < 0.0 {
                            let normal = contact.normal1;
                            pos.0 -= normal.x * contact.dist.abs();
                            pos.1 -= normal.y * contact.dist.abs();

                            let vn = vel.0 * normal.x + vel.1 * normal.y;
                            if vn > 0.0 {
                                vel.0 -= normal.x * vn;
                                vel.1 -= normal.y * vn;

                                if let Some(target) = target_opt.as_deref_mut() {
                                    let seed = COLLISION_SEED.fetch_add(1, Ordering::Relaxed);
                                    let mut rng = WyRand::new_seed(seed);
                                    let land_max_x = config.map.ocean_start_x as f32;
                                    let map_size = config.map.size as f32;
                                    target.1 = (pos.0 + (rng.generate::<f32>() - 0.5) * 500.0).clamp(0.0, land_max_x);
                                    target.2 = (pos.1 + (rng.generate::<f32>() - 0.5) * 500.0).clamp(0.0, map_size);
                                }
                            }
                        }
                    }
                }
            }
        });
}
