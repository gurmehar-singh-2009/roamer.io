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

/// Seed counter so each collision gets different random numbers.
static COLLISION_SEED: AtomicU64 = AtomicU64::new(0);

/// The collider struct contained in ECS bundles.
#[derive(Component, Clone)]
pub struct Collider {
    /// Shape used for exact contact tests.
    pub shape:        SharedShape,
    /// Bounding radius.
    pub rad:          f32,
    /// Half width and half height.
    pub half_extents: (f32, f32),
}

impl Collider {
    /// A circle collider.
    pub fn circle(rad: f32) -> Self {
        Self {
            shape: SharedShape::new(Ball::new(rad)),
            rad,
            half_extents: (rad, rad),
        }
    }

    /// A rectangle collider.
    pub fn rect(half_w: f32, half_h: f32) -> Self {
        Self {
            shape:        SharedShape::new(Cuboid::new(Vector::new(half_w, half_h))),
            rad:          half_w.max(half_h),
            half_extents: (half_w, half_h),
        }
    }
}

#[derive(Component)]
/// A static collider that doesn't move when hit (trees, walls, buildings).
pub struct NonReactiveCollider;

#[derive(Component)]
/// A collider that gets pushed out of other colliders (players, wolves).
pub struct ReactiveCollider;

// I don't know how you could make them their own `type`, tried but didn't work
// well.
#[allow(clippy::type_complexity)]
// Bevy systems take their parameters by value.
#[allow(clippy::needless_pass_by_value)]
/// Pushes reactive colliders out of each other and out of static colliders.
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

        if f32::mul_add(dy, dy, dx * dx) < combined_rad * combined_rad {
            let iso1 = Isometry::translation(p1.0, p1.1);
            let iso2 = Isometry::translation(p2.0, p2.1);

            if let Ok(Some(contact)) = contact(&iso1, c1.shape.as_ref(), &iso2, c2.shape.as_ref(), precision) {
                if contact.dist < 0.0 {
                    let normal = contact.normal1;
                    let separation = contact.dist.abs() * 0.5;
                    p1.0 = f32::mul_add(normal.x, -separation, p1.0);
                    p1.1 = f32::mul_add(normal.y, -separation, p1.1);
                    p2.0 = f32::mul_add(normal.x, separation, p2.0);
                    p2.1 = f32::mul_add(normal.y, separation, p2.1);

                    let v1n = f32::mul_add(v1.1, normal.y, v1.0 * normal.x);
                    if v1n > 0.0 {
                        v1.0 = f32::mul_add(normal.x, -v1n, v1.0);
                        v1.1 = f32::mul_add(normal.y, -v1n, v1.1);
                    }
                    let v2n = f32::mul_add(v2.1, normal.y, v2.0 * normal.x);
                    if v2n < 0.0 {
                        v2.0 = f32::mul_add(normal.x, -v2n, v2.0);
                        v2.1 = f32::mul_add(normal.y, -v2n, v2.1);
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
                // Skip anything not in the non-reactive set (players, animals
                // etc.).
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
                            pos.0 = f32::mul_add(normal.x, -contact.dist.abs(), pos.0);
                            pos.1 = f32::mul_add(normal.y, -contact.dist.abs(), pos.1);

                            let vn = f32::mul_add(vel.1, normal.y, vel.0 * normal.x);
                            if vn > 0.0 {
                                vel.0 = f32::mul_add(normal.x, -vn, vel.0);
                                vel.1 = f32::mul_add(normal.y, -vn, vel.1);

                                if let Some(target) = target_opt.as_deref_mut() {
                                    let seed = COLLISION_SEED.fetch_add(1, Ordering::Relaxed);
                                    let mut rng = WyRand::new_seed(seed);
                                    let land_max_x = f32::from(config.map.ocean_start_x);
                                    let map_size = f32::from(config.map.size);
                                    target.0 =
                                        f32::mul_add(rng.generate::<f32>() - 0.5, 500.0, pos.0).clamp(0.0, land_max_x);
                                    target.1 =
                                        f32::mul_add(rng.generate::<f32>() - 0.5, 500.0, pos.1).clamp(0.0, map_size);
                                }
                            }
                        }
                    }
                }
            }
        });
}
