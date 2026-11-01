//! Projectiles: arrows, musket balls, bullets and turret shots.

use std::sync::atomic::{AtomicU32, Ordering};

use bevy_ecs::{entity::Entity, query::Has, world::World};
use shared::{
    to_client::{AddProjectileData, RemoveProjectileData},
    PacketType,
};

use crate::{
    structs::{
        components::{
            AnimalEntity, Building, Damage, DamageEvents, DamageSource, ObjectEntity, ObjectKind, PlayerEntity,
            Position, Projectile,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        cave_system::{in_rock, Underground},
        social_system::are_allies,
        Collider,
    },
};

/// Weapon id used for turret shots.
pub const TURRET_WEAPON: u8 = 255;

/// Next projectile id. Ids are never reused, so a client can't mix up an old
/// projectile with a new one.
static NEXT_PROJECTILE_ID: AtomicU32 = AtomicU32::new(0);

/// Everything needed to fire one projectile.
#[derive(Clone, Copy, Debug)]
pub struct ProjectileSpec {
    /// Shooter (can't be hit by it).
    pub owner:           Option<Entity>,
    /// Shooter's player id (kill credit, friendly fire).
    pub owner_player:    Option<u32>,
    /// Shooter's position.
    pub from:            (f32, f32),
    /// Distance from `from` where it appears.
    pub muzzle:          f32,
    /// Direction in radians.
    pub angle:           f32,
    /// Weapon id.
    pub weapon:          u8,
    /// Weapon variant.
    pub variant:         u8,
    /// Distance per tick.
    pub speed:           f32,
    /// Max distance.
    pub range:           f32,
    /// Damage to players and animals.
    pub damage:          f32,
    /// Damage to buildings.
    pub building_damage: f32,
    /// Knockback.
    pub knockback:       f32,
    /// Flies over buildings.
    pub over_buildings:  bool,
    /// Fired in the caves.
    pub underground:     bool,
}

/// Spawns a projectile and tells nearby clients about it.
pub fn spawn_projectile(world: &mut World, spec: ProjectileSpec) {
    let dir = (spec.angle.cos(), spec.angle.sin());
    let x = dir.0.mul_add(spec.muzzle, spec.from.0);
    let y = dir.1.mul_add(spec.muzzle, spec.from.1);
    let id = NEXT_PROJECTILE_ID.fetch_add(1, Ordering::Relaxed);

    world.spawn(Projectile {
        id,
        owner: spec.owner,
        owner_player: spec.owner_player,
        weapon: spec.weapon,
        variant: spec.variant,
        x,
        y,
        dir,
        speed: spec.speed,
        range_left: spec.range,
        damage: spec.damage,
        building_damage: spec.building_damage,
        knockback: spec.knockback,
        over_buildings: spec.over_buildings,
        underground: spec.underground,
    });
    world.resource_mut::<Outbox>().send(
        Target::NearLayer(x, y, spec.underground),
        PacketType::AddProjectile,
        &AddProjectileData {
            id,
            x,
            y,
            dir: spec.angle,
            speed: spec.speed,
            range: spec.range,
            weapon: spec.weapon,
            owner: spec.owner_player.unwrap_or(u32::MAX),
        },
    );
}

/// Distance along a ray (from `origin`, unit `dir`) to where it enters a
/// circle, or `None` if it misses within `max`.
pub fn ray_circle_hit(origin: (f32, f32), dir: (f32, f32), center: (f32, f32), radius: f32, max: f32) -> Option<f32> {
    let f = (origin.0 - center.0, origin.1 - center.1);
    let c = radius.mul_add(-radius, f.1.mul_add(f.1, f.0 * f.0));
    if c <= 0.0 {
        // Already inside.
        return Some(0.0);
    }

    let b = f.1.mul_add(dir.1, f.0 * dir.0);
    let disc = b.mul_add(b, -c);
    if disc < 0.0 {
        return None;
    }

    let t = -b - disc.sqrt();
    (0.0..=max).contains(&t).then_some(t)
}

/// What a projectile can run into.
#[derive(Clone, Copy, Debug)]
enum Hittable {
    /// A player (id).
    Player(u32),
    /// An animal.
    Animal,
    /// A building (owner id).
    Building(u32),
    /// A crate.
    Crate,
    /// A tree, stone, bush or gold: blocks, takes no damage.
    Natural,
}

/// Something a projectile can hit: entity, center, radius, what it is, and
/// whether it's underground.
type ShotTarget = (Entity, (f32, f32), f32, Hittable, bool);

/// Moves projectiles and queues damage for whatever they hit.
#[allow(clippy::too_many_lines, reason = "one pass over every projectile, step by step")]
pub fn projectile_system(world: &mut World) {
    let projectiles: Vec<(Entity, Projectile)> = world
        .query::<(Entity, &Projectile)>()
        .iter(world)
        .map(|(e, p)| (e, *p))
        .collect();
    if projectiles.is_empty() {
        return;
    }

    let targets: Vec<ShotTarget> = world
        .query::<(
            Entity,
            &Position,
            &Collider,
            Option<&PlayerEntity>,
            Option<&AnimalEntity>,
            Option<&ObjectKind>,
            Option<&Building>,
            Option<&ObjectEntity>,
            Has<Underground>,
        )>()
        .iter(world)
        .filter_map(|(e, pos, col, player, animal, kind, building, object, under)| {
            if kind == Some(&ObjectKind::CaveEntrance) {
                return None;
            }
            let what = match (player, animal, kind, building, object) {
                (Some(p), ..) => Hittable::Player(p.0),
                (_, Some(_), ..) => Hittable::Animal,
                (_, _, _, Some(b), _) => Hittable::Building(b.owner),
                (_, _, Some(ObjectKind::Crate(_)), ..) => Hittable::Crate,
                (_, _, _, _, Some(_)) => Hittable::Natural,
                _ => return None,
            };
            Some((e, (pos.0, pos.1), col.rad, what, under))
        })
        .collect();

    let mut damage = Vec::new();
    let mut removed = Vec::new();

    for (entity, mut p) in projectiles {
        let step = p.speed.min(p.range_left);
        let owner = p.owner_player;
        let ally = |id: u32| owner.is_some_and(|o| o == id || are_allies(world, o, id));

        let hit = targets
            .iter()
            .filter(|(e, _, _, what, under)| {
                Some(*e) != p.owner
                    && *under == p.underground
                    && match *what {
                        Hittable::Player(id) => !ally(id),
                        Hittable::Building(owner_id) => !p.over_buildings && !ally(owner_id),
                        Hittable::Natural => !p.over_buildings,
                        Hittable::Animal | Hittable::Crate => true,
                    }
            })
            .filter_map(|(e, center, r, what, _)| {
                ray_circle_hit((p.x, p.y), p.dir, *center, *r, step).map(|t| (t, *e, *what))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));

        let Some((t, target, what)) = hit else {
            p.x = p.dir.0.mul_add(step, p.x);
            p.y = p.dir.1.mul_add(step, p.y);
            p.range_left -= step;
            // In the caves, rock stops shots.
            if p.underground && in_rock(world, p.x, p.y) {
                removed.push((p.id, p.x, p.y, true));
                world.despawn(entity);
                continue;
            }
            if p.range_left <= 0.0 {
                world.despawn(entity);
            } else if let Some(mut live) = world.get_mut::<Projectile>(entity) {
                *live = p;
            }
            continue;
        };

        let source = DamageSource {
            entity:  p.owner,
            player:  p.owner_player,
            pos:     (p.x, p.y),
            variant: p.variant,
            melee:   false,
        };
        if !matches!(what, Hittable::Natural) {
            damage.push(Damage {
                target,
                amount: p.damage,
                building_amount: p.building_damage,
                knockback: p.knockback,
                weapon: (p.weapon != TURRET_WEAPON).then_some(p.weapon),
                source,
                ..Damage::default()
            });
        }
        removed.push((p.id, p.dir.0.mul_add(t, p.x), p.dir.1.mul_add(t, p.y), p.underground));
        world.despawn(entity);
    }

    world.resource_mut::<DamageEvents>().0.extend(damage);
    let mut out = world.resource_mut::<Outbox>();
    for (id, x, y, underground) in removed {
        out.send(
            Target::NearLayer(x, y, underground),
            PacketType::RemoveProjectile,
            &RemoveProjectileData { id },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::ray_circle_hit;

    #[test]
    fn ray_hits_and_misses() {
        assert_eq!(
            ray_circle_hit((0.0, 0.0), (1.0, 0.0), (100.0, 0.0), 10.0, 200.0),
            Some(90.0)
        );
        assert_eq!(ray_circle_hit((0.0, 0.0), (1.0, 0.0), (100.0, 50.0), 10.0, 200.0), None);
        assert_eq!(ray_circle_hit((0.0, 0.0), (1.0, 0.0), (100.0, 0.0), 10.0, 50.0), None);
        assert_eq!(
            ray_circle_hit((100.0, 0.0), (1.0, 0.0), (100.0, 0.0), 10.0, 50.0),
            Some(0.0)
        );
    }
}
