use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{
    query::With,
    system::{Query, Res},
    world::World,
};
use nanorand::{Rng, WyRand};
use rstar::AABB;

use crate::{
    structs::components::{AiState, AiTarget, AnimalEntity, AnimalType, Position, Velocity},
    systems::{Collider, FishSpatialIndex, ReactiveCollider},
    CONFIG,
};

static AI_SEED: AtomicU64 = AtomicU64::new(0);

/// Handles movement for every animal.
pub fn animal_ai_system(
    index: Res<FishSpatialIndex>,
    config: Res<crate::config::Config>,
    mut query: Query<(&mut Velocity, &mut Position, &mut AiState, &mut AiTarget, &AnimalType), With<AnimalEntity>>,
) {
    let snapshots = &index.snapshots;

    query
        .par_iter_mut()
        .for_each(|(mut vel, mut pos, state, mut target, animal_type)| {
            match *state {
                AiState::Idle => {
                    vel.0 *= 0.8;
                    vel.1 *= 0.8;
                }

                AiState::Wander => match animal_type {
                    AnimalType::Wolf => {
                        let dx = target.1 - pos.0;
                        let dy = target.2 - pos.1;
                        let dist_sq = dx * dx + dy * dy;

                        if dist_sq < 1000.0 {
                            let seed = AI_SEED.fetch_add(1, Ordering::Relaxed);
                            let mut rng = WyRand::new_seed(seed);

                            let land_max_x = config.map.ocean_start_x as f32;
                            let map_size = config.map.size as f32;

                            target.1 = (pos.0 + (rng.generate::<f32>() - 0.5) * 1024.0).clamp(0.0, land_max_x);
                            target.2 = (pos.1 + (rng.generate::<f32>() - 0.5) * 1024.0).clamp(0.0, map_size);
                        }

                        let dist = dist_sq.sqrt();
                        if dist > 0.0 {
                            let desired_vx = (dx / dist) * 15.0;
                            let desired_vy = (dy / dist) * 15.0;
                            vel.0 += (desired_vx - vel.0) * 0.1;
                            vel.1 += (desired_vy - vel.1) * 0.1;
                        }
                    }

                    AnimalType::Fish => {
                        let mut neighbor_count = 0u32;
                        let mut close_dx = 0.0_f32;
                        let mut close_dy = 0.0_f32;
                        let mut avg_vel_x = 0.0_f32;
                        let mut avg_vel_y = 0.0_f32;
                        let mut center_x = 0.0_f32;
                        let mut center_y = 0.0_f32;

                        let aabb = AABB::from_corners(
                            [
                                pos.0 - config.animals.fish_visual_range,
                                pos.1 - config.animals.fish_visual_range,
                            ],
                            [
                                pos.0 + config.animals.fish_visual_range,
                                pos.1 + config.animals.fish_visual_range,
                            ],
                        );

                        for point in index.tree.locate_in_envelope_intersecting(&aabb) {
                            let (other_vel, other_pos, _) = &snapshots[point.index];

                            let dx = pos.0 - other_pos.0;
                            let dy = pos.1 - other_pos.1;
                            let dist_sq = dx * dx + dy * dy;

                            if dist_sq == 0.0 || dist_sq > config.animals.fish_visual_range_sq {
                                continue;
                            }

                            neighbor_count += 1;

                            if dist_sq < config.animals.fish_protect_range_sq {
                                let force = (config.animals.fish_protect_range_sq - dist_sq)
                                    / config.animals.fish_protect_range_sq;
                                close_dx += dx * force;
                                close_dy += dy * force;
                            }

                            avg_vel_x += other_vel.0;
                            avg_vel_y += other_vel.1;
                            center_x += other_pos.0;
                            center_y += other_pos.1;
                        }

                        if neighbor_count > 0 {
                            vel.0 += close_dx * config.animals.fish_separation_factor;
                            vel.1 += close_dy * config.animals.fish_separation_factor;

                            let n = neighbor_count as f32;
                            vel.0 += (avg_vel_x / n - vel.0) * config.animals.fish_alignment_factor;
                            vel.1 += (avg_vel_y / n - vel.1) * config.animals.fish_alignment_factor;
                            vel.0 += (center_x / n - pos.0) * config.animals.fish_cohesion_factor;
                            vel.1 += (center_y / n - pos.1) * config.animals.fish_cohesion_factor;
                        }

                        let speed = (vel.0 * vel.0 + vel.1 * vel.1).sqrt();
                        let wander_scale = if neighbor_count > 0 { 0.02 } else { 0.4 };
                        let seed = AI_SEED.fetch_add(1, Ordering::Relaxed);
                        let mut rng = WyRand::new_seed(seed);
                        let wander_angle = (rng.generate::<f32>() - 0.5) * wander_scale;

                        if speed > 0.1 {
                            let cos_a = wander_angle.cos();
                            let sin_a = wander_angle.sin();
                            let new_vx = vel.0 * cos_a - vel.1 * sin_a;
                            let new_vy = vel.0 * sin_a + vel.1 * cos_a;
                            vel.0 = new_vx;
                            vel.1 = new_vy;
                        } else {
                            vel.0 += (rng.generate::<f32>() - 0.5) * 2.0;
                            vel.1 += (rng.generate::<f32>() - 0.5) * 2.0;
                        }

                        let speed_sq = vel.0 * vel.0 + vel.1 * vel.1;
                        const MAX_SPD: f32 = 67.0;
                        const MIN_SPD: f32 = 6.7;
                        if speed_sq > MAX_SPD * MAX_SPD {
                            let s = MAX_SPD / speed_sq.sqrt();
                            vel.0 *= s;
                            vel.1 *= s;
                        } else if speed_sq < MIN_SPD * MIN_SPD && speed_sq > 0.0 {
                            let s = MIN_SPD / speed_sq.sqrt();
                            vel.0 *= s;
                            vel.1 *= s;
                        }
                    }
                },

                AiState::Chase => match animal_type {
                    AnimalType::Wolf => {
                        let dx = target.1 - pos.0;
                        let dy = target.2 - pos.1;
                        let dist_sq = dx * dx + dy * dy;
                        let dist = dist_sq.sqrt();
                        if dist > 0.0 {
                            let desired_vx = (dx / dist) * 30.0;
                            let desired_vy = (dy / dist) * 30.0;
                            vel.0 += (desired_vx - vel.0) * 0.15;
                            vel.1 += (desired_vy - vel.1) * 0.15;
                        }
                    }
                    AnimalType::Fish => {}
                },

                AiState::Flee => match animal_type {
                    AnimalType::Wolf => {
                        let dx = pos.0 - target.1;
                        let dy = pos.1 - target.2;
                        let dist_sq = dx * dx + dy * dy;
                        let dist = dist_sq.sqrt();
                        if dist > 0.0 {
                            let desired_vx = (dx / dist) * 35.0;
                            let desired_vy = (dy / dist) * 35.0;
                            vel.0 += (desired_vx - vel.0) * 0.2;
                            vel.1 += (desired_vy - vel.1) * 0.2;
                        }
                    }
                    AnimalType::Fish => {}
                },

                AiState::Eat => {
                    vel.0 *= 0.5;
                    vel.1 *= 0.5;
                }
            }

            // Boundary reflection.
            let (min_x, max_x) = match animal_type {
                AnimalType::Wolf => (0.0, config.map.ocean_start_x as f32),
                AnimalType::Fish => (config.map.ocean_start_x as f32, config.map.ocean_end_x as f32),
            };
            let min_y = 0.0;
            let max_y = config.map.size as f32;

            pos.0 += vel.0;
            pos.1 += vel.1;

            if pos.0 <= min_x {
                pos.0 = min_x;
                if vel.0 < 0.0 {
                    vel.0 *= -1.;
                }
                // vel.0 += 2.0;
            } else if pos.0 >= max_x {
                pos.0 = max_x;
                if vel.0 > 0.0 {
                    vel.0 *= -1.;
                }
                // vel.0 -= 2.0;
            }

            if pos.1 <= min_y {
                pos.1 = min_y;
                if vel.1 < 0.0 {
                    vel.1 *= -1.;
                }
                // vel.1 += 2.0;
            } else if pos.1 >= max_y {
                pos.1 = max_y;
                if vel.1 > 0.0 {
                    vel.1 *= -1.;
                }
                // vel.1 -= 2.0;
            }

            pos.0 = pos.0.clamp(min_x, max_x);
            pos.1 = pos.1.clamp(min_y, max_y);
        });
}

pub fn init_animals(world: &mut World, rng: &mut nanorand::WyRand) {
    let min_x = CONFIG.map.ocean_start_x as f32;
    let max_x = CONFIG.map.ocean_end_x as f32;
    let min_y = 0.0;
    let max_y = CONFIG.map.size as f32;

    for _ in 0..CONFIG.animals.max_fish_alive {
        let x = min_x + rng.generate::<f32>() * (max_x - min_x);
        let y = min_y + rng.generate::<f32>() * (max_y - min_y);

        world.spawn((
            Position(x, y),
            Velocity(
                (rng.generate::<f32>() - 0.5) * 50.0,
                (rng.generate::<f32>() - 0.5) * 50.0,
            ),
            AiState::Wander,
            AiTarget(
                None,
                x + (rng.generate::<f32>() - 0.5) * 100.0,
                y + (rng.generate::<f32>() - 0.5) * 100.0,
            ),
            AnimalType::Fish,
            AnimalEntity,
            Collider::circle(35.0),
        ));
    }

    for _ in 0..CONFIG.animals.max_wolf {
        let land_max_x = CONFIG.map.ocean_start_x as f32;
        let map_size = CONFIG.map.size as f32;

        let x = rng.generate::<f32>() * land_max_x;
        let y = rng.generate::<f32>() * map_size;

        world.spawn((
            Position(x, y),
            Velocity(
                (rng.generate::<f32>() - 0.5) * 50.0,
                (rng.generate::<f32>() - 0.5) * 50.0,
            ),
            AiState::Wander,
            AiTarget(
                None,
                (x + (rng.generate::<f32>() - 0.5) * 300.0).clamp(0.0, land_max_x),
                (y + (rng.generate::<f32>() - 0.5) * 300.0).clamp(0.0, map_size),
            ),
            AnimalType::Wolf,
            AnimalEntity,
            Collider::circle(80.0),
            ReactiveCollider,
        ));
    }
}
