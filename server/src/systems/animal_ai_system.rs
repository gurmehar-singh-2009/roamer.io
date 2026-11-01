//! Animal behavior: fish schools in the ocean, and land animals that wander,
//! flee, fight back or hunt players.

use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{
    entity::Entity,
    system::{Query, Res},
    world::World,
};
use nanorand::{Rng, WyRand};
use rstar::AABB;

use crate::{
    catalog::animals::{animal, Behavior, Habitat},
    config::Config,
    structs::components::{
        AiState, AiTarget, AnimalEntity, BiteCooldown, Damage, DamageEvents, DamageSource, PlayerEntity, Position,
        Velocity,
    },
    systems::cave_system::Underground,
    systems::pathfinding::{rebuild_nav_grid, AnimalPath, NavGrid},
    systems::{Collider, FishSpatialIndex, TICK_MS},
    CONFIG,
};

/// Seed counter so every animal gets different random numbers.
static AI_SEED: AtomicU64 = AtomicU64::new(0);

/// Fastest a fish can swim.
const FISH_MAX_SPEED: f32 = 67.0;
/// Slowest a fish can swim.
const FISH_MIN_SPEED: f32 = 6.7;
/// How far hostile animals notice players.
const AGGRO_RANGE: f32 = 500.0;
/// How far a chase goes before giving up.
const CHASE_GIVE_UP: f32 = 1200.0;
/// Time between bites.
const BITE_COOLDOWN_MS: u32 = 1000;
/// How hard a bite knocks players back.
const BITE_KNOCKBACK: f32 = 60.0;
/// Most path searches per tick (the rest keep their old paths for now).
const MAX_PATHS_PER_TICK: u32 = 12;
/// How often an animal plans its path again.
const REPATH_MS: u32 = 1500;
/// Ticks between rebuilds of the walkable grid (buildings come and go).
const NAV_REBUILD_TICKS: u64 = 15;
/// How far fleeing animals try to run.
const FLEE_DISTANCE: f32 = 500.0;

/// Updates an animal's path towards `dest` (when needed and the search
/// budget allows) and returns the point to head for next.
fn follow_path(
    nav: &NavGrid,
    path: &mut AnimalPath,
    pos: Position,
    dest: (f32, f32),
    budget: &mut u32,
    rng: &mut WyRand,
) -> (f32, f32) {
    path.repath_ms = path.repath_ms.saturating_sub(TICK_MS);
    let goal_moved = path.goal.is_none_or(|g| (g.0 - dest.0).hypot(g.1 - dest.1) > 120.0);
    let lost = path.waypoints.is_empty() && (pos.0 - dest.0).hypot(pos.1 - dest.1) > 60.0;
    if (goal_moved || lost || path.repath_ms == 0) && *budget > 0 {
        *budget -= 1;
        path.waypoints = nav.find_path((pos.0, pos.1), dest).unwrap_or_default();
        path.goal = Some(dest);
        // Spread the repaths out over time.
        path.repath_ms = REPATH_MS + rng.generate_range(0..500_u32);
    }
    // Reached a waypoint: on to the next one.
    while path.waypoints.len() > 1 {
        let &(wx, wy) = path.waypoints.last().unwrap_or(&dest);
        if (wx - pos.0).hypot(wy - pos.1) < 50.0 {
            path.waypoints.pop();
        } else {
            break;
        }
    }
    path.waypoints.last().copied().unwrap_or(dest)
}

/// Returns a random number generator with a fresh seed.
fn new_rng() -> WyRand {
    WyRand::new_seed(AI_SEED.fetch_add(1, Ordering::Relaxed))
}

/// Schools of fish (boids).
// Bevy systems take their parameters by value.
#[allow(clippy::needless_pass_by_value)]
pub fn fish_system(
    index: Res<FishSpatialIndex>,
    config: Res<Config>,
    mut query: Query<(&mut Velocity, &mut Position, &AnimalEntity)>,
) {
    query.par_iter_mut().for_each(|(mut vel, mut pos, a)| {
        if !animal(a.0).is_some_and(|d| d.behavior == Behavior::Fish) {
            return;
        }
        fish_wander(&mut vel, *pos, &index, &config);
        move_and_bounce(&mut pos, &mut vel, Habitat::Ocean, &config);
    });
}

/// Boids flocking: separation, alignment and cohesion with nearby fish, plus
/// some random wandering.
fn fish_wander(vel: &mut Velocity, pos: Position, index: &FishSpatialIndex, config: &Config) {
    let animals = &config.animals;

    let mut neighbor_count = 0.0_f32;
    let mut close = (0.0_f32, 0.0_f32);
    let mut avg_vel = (0.0_f32, 0.0_f32);
    let mut center = (0.0_f32, 0.0_f32);

    let range = animals.fish_visual_range;
    let aabb = AABB::from_corners([pos.0 - range, pos.1 - range], [pos.0 + range, pos.1 + range]);

    for point in index.tree.locate_in_envelope_intersecting(&aabb) {
        let (other_vel, other_pos) = &index.snapshots[point.index];

        let dx = pos.0 - other_pos.0;
        let dy = pos.1 - other_pos.1;
        let dist_sq = f32::mul_add(dy, dy, dx * dx);

        if dist_sq == 0.0 || dist_sq > animals.fish_visual_range_sq {
            continue;
        }

        neighbor_count += 1.0;

        if dist_sq < animals.fish_protect_range_sq {
            let force = (animals.fish_protect_range_sq - dist_sq) / animals.fish_protect_range_sq;
            close.0 = f32::mul_add(dx, force, close.0);
            close.1 = f32::mul_add(dy, force, close.1);
        }

        avg_vel.0 += other_vel.0;
        avg_vel.1 += other_vel.1;
        center.0 += other_pos.0;
        center.1 += other_pos.1;
    }

    if neighbor_count > 0.0 {
        let n = neighbor_count;
        vel.0 = close.0.mul_add(animals.fish_separation_factor, vel.0);
        vel.1 = close.1.mul_add(animals.fish_separation_factor, vel.1);
        vel.0 = (avg_vel.0 / n - vel.0).mul_add(animals.fish_alignment_factor, vel.0);
        vel.1 = (avg_vel.1 / n - vel.1).mul_add(animals.fish_alignment_factor, vel.1);
        vel.0 = (center.0 / n - pos.0).mul_add(animals.fish_cohesion_factor, vel.0);
        vel.1 = (center.1 / n - pos.1).mul_add(animals.fish_cohesion_factor, vel.1);
    }

    let speed = vel.0.hypot(vel.1);
    let wander_scale = if neighbor_count > 0.0 { 0.02 } else { 0.4 };
    let mut rng = new_rng();
    let wander_angle = (rng.generate::<f32>() - 0.5) * wander_scale;

    if speed > 0.1 {
        let (sin_a, cos_a) = wander_angle.sin_cos();
        let new_vx = f32::mul_add(vel.1, -sin_a, vel.0 * cos_a);
        let new_vy = f32::mul_add(vel.1, cos_a, vel.0 * sin_a);
        vel.0 = new_vx;
        vel.1 = new_vy;
    } else {
        vel.0 = f32::mul_add(rng.generate::<f32>() - 0.5, 2.0, vel.0);
        vel.1 = f32::mul_add(rng.generate::<f32>() - 0.5, 2.0, vel.1);
    }

    let speed_sq = f32::mul_add(vel.1, vel.1, vel.0 * vel.0);
    if speed_sq > FISH_MAX_SPEED * FISH_MAX_SPEED {
        let s = FISH_MAX_SPEED / speed_sq.sqrt();
        vel.0 *= s;
        vel.1 *= s;
    } else if speed_sq < FISH_MIN_SPEED * FISH_MIN_SPEED && speed_sq > 0.0 {
        let s = FISH_MIN_SPEED / speed_sq.sqrt();
        vel.0 *= s;
        vel.1 *= s;
    }
}

/// Moves an animal and bounces it off the edges of its habitat.
fn move_and_bounce(pos: &mut Position, vel: &mut Velocity, habitat: Habitat, config: &Config) {
    let (min_x, max_x) = match habitat {
        Habitat::Land => (0.0, f32::from(config.map.ocean_start_x)),
        Habitat::Ocean => (f32::from(config.map.ocean_start_x), f32::from(config.map.ocean_end_x)),
    };
    let (min_y, max_y) = (0.0, f32::from(config.map.size));

    pos.0 += vel.0;
    pos.1 += vel.1;

    if pos.0 <= min_x || pos.0 >= max_x {
        vel.0 = -vel.0;
    }
    if pos.1 <= min_y || pos.1 >= max_y {
        vel.1 = -vel.1;
    }
    pos.0 = pos.0.clamp(min_x, max_x);
    pos.1 = pos.1.clamp(min_y, max_y);
}

/// Turns the velocity towards the direction (`dx`, `dy`) at `speed`.
/// `factor` (0..1) is how quickly it turns.
fn steer(vel: &mut Velocity, dx: f32, dy: f32, speed: f32, factor: f32) {
    let dist = dx.hypot(dy);
    if dist > 0.0 {
        vel.0 = f32::mul_add((dx / dist).mul_add(speed, -vel.0), factor, vel.0);
        vel.1 = f32::mul_add((dy / dist).mul_add(speed, -vel.1), factor, vel.1);
    }
}

/// Land animals: wander, flee, chase and bite.
#[allow(
    clippy::too_many_lines,
    reason = "one state machine; splitting it scatters the states"
)]
pub fn land_animal_system(world: &mut World) {
    let tick = world.resource::<crate::structs::components::TickCount>().0;
    if tick.is_multiple_of(NAV_REBUILD_TICKS) || !world.contains_resource::<NavGrid>() {
        rebuild_nav_grid(world);
    }
    let nav = world.remove_resource::<NavGrid>().unwrap_or_default();
    let mut budget = MAX_PATHS_PER_TICK;
    let mut path_rng = new_rng();

    let players: Vec<(Entity, u32, Position, f32)> = world
        .query_filtered::<(Entity, &PlayerEntity, &Position, &Collider), bevy_ecs::query::Without<Underground>>()
        .iter(world)
        .map(|(e, p, pos, c)| (e, p.0, *pos, c.rad))
        .collect();

    let mut bites = Vec::new();
    let land_x = f32::from(CONFIG.map.ocean_start_x);
    let size = f32::from(CONFIG.map.size);

    for (entity, kind, mut pos, mut vel, mut state, mut target, mut bite, col, mut path) in world
        .query::<(
            Entity,
            &AnimalEntity,
            &mut Position,
            &mut Velocity,
            &mut AiState,
            &mut AiTarget,
            &mut BiteCooldown,
            &Collider,
            &mut AnimalPath,
        )>()
        .iter_mut(world)
    {
        let Some(def) = animal(kind.0) else {
            continue;
        };
        if def.behavior == Behavior::Fish {
            continue;
        }
        bite.0 = bite.0.saturating_sub(TICK_MS);

        // Hostile animals go after the nearest player in range.
        if def.behavior == Behavior::Hostile && *state == AiState::Wander {
            if let Some((p, ..)) = players
                .iter()
                .map(|(e, _, p, _)| (e, (p.0 - pos.0).hypot(p.1 - pos.1)))
                .filter(|(_, d)| *d < AGGRO_RANGE)
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                *state = AiState::Chase(*p);
            }
        }

        match *state {
            AiState::Wander => {
                let (dx, dy) = (target.0 - pos.0, target.1 - pos.1);
                if dx.hypot(dy) < 40.0 {
                    let mut rng = new_rng();
                    target.0 = f32::mul_add(rng.generate::<f32>() - 0.5, 1024.0, pos.0).clamp(0.0, land_x);
                    target.1 = f32::mul_add(rng.generate::<f32>() - 0.5, 1024.0, pos.1).clamp(0.0, size);
                }
                let next = follow_path(&nav, &mut path, *pos, (target.0, target.1), &mut budget, &mut path_rng);
                steer(&mut vel, next.0 - pos.0, next.1 - pos.1, def.speed * 0.4, 0.1);
            }
            AiState::Flee { from, ms } => {
                // Run to a spot away from the threat, around obstacles.
                let (ax, ay) = (pos.0 - from.0, pos.1 - from.1);
                let len = ax.hypot(ay).max(1.0);
                let dest = (
                    (ax / len).mul_add(FLEE_DISTANCE, pos.0).clamp(0.0, land_x),
                    (ay / len).mul_add(FLEE_DISTANCE, pos.1).clamp(0.0, size),
                );
                let next = follow_path(&nav, &mut path, *pos, dest, &mut budget, &mut path_rng);
                steer(&mut vel, next.0 - pos.0, next.1 - pos.1, def.speed, 0.2);
                let ms = ms.saturating_sub(TICK_MS);
                *state = if ms == 0 {
                    AiState::Wander
                } else {
                    AiState::Flee { from, ms }
                };
            }
            AiState::Chase(prey) => {
                let Some(&(_, id, ppos, prad)) = players.iter().find(|(e, ..)| *e == prey) else {
                    *state = AiState::Wander;
                    continue;
                };
                let (dx, dy) = (ppos.0 - pos.0, ppos.1 - pos.1);
                let dist = dx.hypot(dy);
                if dist > CHASE_GIVE_UP {
                    *state = AiState::Wander;
                    continue;
                }
                // Close enough: straight at them. Otherwise follow the path.
                let next = if dist < 160.0 {
                    (ppos.0, ppos.1)
                } else {
                    follow_path(&nav, &mut path, *pos, (ppos.0, ppos.1), &mut budget, &mut path_rng)
                };
                steer(&mut vel, next.0 - pos.0, next.1 - pos.1, def.speed, 0.2);
                if dist < col.rad + prad + 15.0 && bite.0 == 0 && def.damage > 0.0 {
                    bite.0 = BITE_COOLDOWN_MS;
                    bites.push(Damage {
                        target: prey,
                        amount: def.damage,
                        knockback: BITE_KNOCKBACK,
                        source: DamageSource {
                            entity: Some(entity),
                            pos: (pos.0, pos.1),
                            ..DamageSource::default()
                        },
                        ..Damage::default()
                    });
                    let _ = id;
                }
            }
        }

        // Friction so they don't slide forever after knockback.
        vel.0 *= 0.9;
        vel.1 *= 0.9;
        let mut p = *pos;
        let mut v = *vel;
        move_and_bounce(&mut p, &mut v, Habitat::Land, &CONFIG);
        *pos = p;
        *vel = v;
    }

    world.insert_resource(nav);
    world.resource_mut::<DamageEvents>().0.extend(bites);
}
