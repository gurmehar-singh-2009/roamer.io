//! World generation and upkeep: natural objects, loot crates and animals.

use bevy_ecs::{entity::Entity, world::World};
use nanorand::Rng;

use crate::{
    catalog::animals::{animal, AnimalDef, Habitat, ANIMALS},
    config::Config,
    structs::components::{
        AiState, AiTarget, AimDir, AnimalEntity, BiteCooldown, Health, ObjectBundle, ObjectKind, Position,
        SolidObjectBundle, TickCount, Velocity,
    },
    systems::{
        building_system::random_dir, player_system::random_land_spot, rng, Collider, NonReactiveCollider,
        ReactiveCollider, TICK_MS,
    },
    CONFIG,
};

/// Natural object counts.
const TREES: u32 = 900;
/// Stones.
const STONES: u32 = 380;
/// Berry bushes.
const BUSHES: u32 = 260;
/// Gold rocks.
const GOLD_ROCKS: u32 = 45;
/// Stones on the sand strip east of the ocean.
const SAND_STONES: u32 = 70;
/// Gold rocks on the sand.
const SAND_GOLD: u32 = 10;
/// Bushes on the sand.
const SAND_BUSHES: u32 = 30;
/// Loot crates kept on the map.
pub const CRATES: usize = 70;
/// Chance of each crate tier (plain, gold, diamond, ruby, emerald), in %.
const CRATE_TIER_WEIGHTS: [u32; 5] = [50, 25, 13, 8, 4];
/// Crate health per tier.
const CRATE_HEALTH: [f32; 5] = [80.0, 150.0, 250.0, 400.0, 600.0];

/// Which biome a y position is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    /// Top band.
    Snow,
    /// Second band.
    Grass,
    /// Third band.
    Desert,
    /// Bottom band.
    Lava,
}

/// The biome at height `y`.
fn biome(config: &Config, y: f32) -> Biome {
    if y < f32::from(config.map.snow_end) {
        Biome::Snow
    } else if y < f32::from(config.map.grassland_end) {
        Biome::Grass
    } else if y < f32::from(config.map.desert_end) {
        Biome::Desert
    } else {
        Biome::Lava
    }
}

/// Spawns a natural object.
fn spawn_natural(world: &mut World, kind: ObjectKind, (x, y): (f32, f32), radius: f32) {
    world.spawn(SolidObjectBundle {
        object: ObjectBundle {
            object: crate::structs::components::ObjectEntity,
            kind,
            pos: Position(x, y),
            dir: AimDir(random_dir()),
            health: Health(1.0, 1.0),
            collider: Collider::circle(radius),
        },
        solid:  NonReactiveCollider,
    });
}

/// Most of an object that another may cover (0.8 = 80% of the smaller one's
/// width).
const MAX_OVERLAP: f32 = 0.8;

/// How much of the smaller of two circles the other covers, from 0 (apart)
/// to 1 (fully covered), measured across its width.
pub fn overlap((x1, y1, r1): (f32, f32, f32), (x2, y2, r2): (f32, f32, f32)) -> f32 {
    let dist = (x1 - x2).hypot(y1 - y2);
    ((r1 + r2 - dist) / (2.0 * r1.min(r2))).clamp(0.0, 1.0)
}

/// Places natural objects so that none overlaps another by more than
/// `MAX_OVERLAP`.
#[derive(Default)]
struct Placer {
    /// Objects placed so far: (x, y, radius).
    placed: Vec<(f32, f32, f32)>,
}

impl Placer {
    /// Places `count` objects of a kind; `pick` suggests a spot and radius (or
    /// `None` for a spot outside the kind's biome). Gives up after many
    /// failed tries, so a crowded map can't hang the server.
    fn fill(
        &mut self,
        world: &mut World,
        rng: &mut nanorand::WyRand,
        count: u32,
        kind: ObjectKind,
        mut pick: impl FnMut(&mut nanorand::WyRand) -> Option<((f32, f32), f32)>,
    ) {
        let mut placed = 0;
        for _ in 0..count * 60 {
            if placed >= count {
                return;
            }
            let Some(((x, y), r)) = pick(rng) else {
                continue;
            };
            if self.placed.iter().any(|&o| overlap(o, (x, y, r)) > MAX_OVERLAP) {
                continue;
            }
            spawn_natural(world, kind, (x, y), r);
            self.placed.push((x, y, r));
            placed += 1;
        }
    }
}

/// Initializes the map: natural objects, wall boundaries, crates and
/// animals.
pub fn init_map(world: &mut World, rng: &mut nanorand::WyRand, config: &Config) {
    let land_x = f32::from(config.map.ocean_start_x);
    let size = f32::from(config.map.size);
    let point = |rng: &mut nanorand::WyRand| (rng.generate::<f32>() * land_x, rng.generate::<f32>() * size);
    let sand_x = f32::from(config.map.ocean_end_x);
    let sand_w = size - sand_x;
    let sand_point = |rng: &mut nanorand::WyRand| {
        (
            rng.generate::<f32>().mul_add(sand_w - 160.0, sand_x + 80.0),
            rng.generate::<f32>() * size,
        )
    };
    let tree_r = |rng: &mut nanorand::WyRand| rng.generate::<f32>().mul_add(60.0, 60.0);
    let stone_r = |rng: &mut nanorand::WyRand| rng.generate::<f32>().mul_add(30.0, 35.0);
    let bush_r = |rng: &mut nanorand::WyRand| rng.generate::<f32>().mul_add(15.0, 45.0);
    let in_biome = |p: (f32, f32), biomes: &[Biome]| biomes.contains(&biome(config, p.1));

    let mut placer = Placer::default();
    placer.fill(world, rng, TREES, ObjectKind::Tree, |rng| {
        let p = point(rng);
        in_biome(p, &[Biome::Snow, Biome::Grass]).then(|| (p, tree_r(rng)))
    });
    placer.fill(world, rng, STONES, ObjectKind::Stone, |rng| {
        Some((point(rng), stone_r(rng)))
    });
    placer.fill(world, rng, BUSHES, ObjectKind::Bush, |rng| {
        let p = point(rng);
        in_biome(p, &[Biome::Grass, Biome::Desert]).then(|| (p, bush_r(rng)))
    });
    placer.fill(world, rng, GOLD_ROCKS, ObjectKind::Gold, |rng| {
        let p = point(rng);
        in_biome(p, &[Biome::Desert, Biome::Lava]).then_some((p, 55.0))
    });

    // The sand strip east of the ocean.
    placer.fill(world, rng, SAND_STONES, ObjectKind::Stone, |rng| {
        Some((sand_point(rng), stone_r(rng)))
    });
    placer.fill(world, rng, SAND_GOLD, ObjectKind::Gold, |rng| {
        Some((sand_point(rng), 55.0))
    });
    placer.fill(world, rng, SAND_BUSHES, ObjectKind::Bush, |rng| {
        Some((sand_point(rng), bush_r(rng)))
    });

    init_wall_boundaries(world);
    for _ in 0..CRATES {
        spawn_crate(world, None);
    }
    for def in &ANIMALS {
        for _ in 0..def.count {
            spawn_animal(world, def.id, None);
        }
    }
}

/// Initializes wall boundaries.
fn init_wall_boundaries(world: &mut World) {
    world.spawn((Position(8192., -67.), Collider::rect(8192., 67.), NonReactiveCollider));
    world.spawn((
        Position(8192., 16384. + 67.),
        Collider::rect(8192., 67.),
        NonReactiveCollider,
    ));
    world.spawn((Position(-67., 8192.), Collider::rect(67., 8192.), NonReactiveCollider));
    world.spawn((
        Position(16384. + 67., 8192.),
        Collider::rect(67., 8192.),
        NonReactiveCollider,
    ));
}

/// Picks a crate tier.
fn random_tier() -> u8 {
    let roll = rng().generate_range(0..CRATE_TIER_WEIGHTS.iter().sum::<u32>());
    let mut acc = 0;
    for (tier, w) in CRATE_TIER_WEIGHTS.iter().enumerate() {
        acc += w;
        if roll < acc {
            return u8::try_from(tier).unwrap_or(0);
        }
    }
    0
}

/// Spawns a crate somewhere on land (random tier unless given).
pub fn spawn_crate(world: &mut World, tier: Option<u8>) -> Entity {
    let tier = tier.unwrap_or_else(random_tier);
    let (x, y) = random_land_spot(world, 80.0);
    let hp = CRATE_HEALTH[usize::from(tier)];
    world
        .spawn(SolidObjectBundle {
            object: ObjectBundle {
                object:   crate::structs::components::ObjectEntity,
                kind:     ObjectKind::Crate(tier),
                pos:      Position(x, y),
                dir:      AimDir(random_dir()),
                health:   Health(hp, hp),
                collider: Collider::circle(40.0),
            },
            solid:  NonReactiveCollider,
        })
        .id()
}

/// A random spot in an animal's habitat.
fn habitat_spot(world: &mut World, def: &AnimalDef) -> (f32, f32) {
    match def.habitat {
        Habitat::Land => random_land_spot(world, 100.0),
        Habitat::Ocean => {
            let mut rng = rng();
            let min_x = f32::from(CONFIG.map.ocean_start_x) + 50.0;
            let max_x = f32::from(CONFIG.map.ocean_end_x) - 50.0;
            (
                rng.generate::<f32>().mul_add(max_x - min_x, min_x),
                rng.generate::<f32>() * f32::from(CONFIG.map.size),
            )
        }
    }
}

/// Spawns an animal (at a random spot in its habitat unless given).
pub fn spawn_animal(world: &mut World, id: u8, at: Option<(f32, f32)>) -> Option<Entity> {
    let def = animal(id)?;
    let (x, y) = at.unwrap_or_else(|| habitat_spot(world, def));
    let mut rng = rng();
    let mut entity = world.spawn((
        AnimalEntity(id),
        Position(x, y),
        Velocity(
            (rng.generate::<f32>() - 0.5) * 10.0,
            (rng.generate::<f32>() - 0.5) * 10.0,
        ),
        Health(def.health, def.health),
        AiState::Wander,
        AiTarget(x, y),
        BiteCooldown::default(),
        crate::systems::pathfinding::AnimalPath::default(),
        Collider::circle(def.radius),
    ));
    if def.habitat == Habitat::Land {
        entity.insert(ReactiveCollider);
    }
    Some(entity.id())
}

/// Every few seconds, replaces opened crates and killed animals.
pub fn upkeep_system(world: &mut World) {
    let tick = world.resource::<TickCount>().0;
    if !tick.is_multiple_of(u64::from(5000 / TICK_MS)) {
        return;
    }

    let crates = world
        .query::<&ObjectKind>()
        .iter(world)
        .filter(|k| matches!(k, ObjectKind::Crate(_)))
        .count();
    for _ in crates..CRATES {
        spawn_crate(world, None);
    }

    let mut counts = [0_u32; ANIMALS.len()];
    for a in world.query::<&AnimalEntity>().iter(world) {
        if let Some(c) = counts.get_mut(usize::from(a.0)) {
            *c += 1;
        }
    }
    for def in &ANIMALS {
        let have = counts[usize::from(def.id)];
        // Refill a few at a time so it isn't obvious.
        for _ in have..def.count.min(have + 5) {
            spawn_animal(world, def.id, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::overlap;

    #[test]
    fn overlap_is_measured_on_the_smaller_object() {
        assert!(overlap((0.0, 0.0, 50.0), (200.0, 0.0, 50.0)) <= 0.0);
        // Touching edges: no overlap.
        assert!(overlap((0.0, 0.0, 50.0), (100.0, 0.0, 50.0)).abs() < 1e-6);
        // Same spot: fully covered.
        assert!((overlap((0.0, 0.0, 50.0), (0.0, 0.0, 20.0)) - 1.0).abs() < 1e-6);
        // Half of the small one's width covered.
        assert!((overlap((0.0, 0.0, 100.0), (110.0, 0.0, 20.0)) - 0.25).abs() < 1e-6);
    }
}
