//! Tunnels and caves.
//!
//! Under the map is solid rock, sampled on a grid of points `SAMPLE` apart.
//! Dug-out points are "open", and the cave floor is the marching-squares
//! outline around them, so tunnels have smooth, diagonal walls (the client
//! draws the same outline, see `web/src/render/caves.js`).
//!
//! Players reach the caves through entrances (natural ones made at world
//! generation, or Tunnel items that players place). Underground, they can only
//! walk on the floor, and swinging a melee weapon digs a round hole in front of
//! them. Surface and underground players can't see or hit each other.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use bevy_ecs::{component::Component, entity::Entity, query::With, resource::Resource, world::World};
use nanorand::Rng;
use shared::{
    to_client::{CaveCellTO, CaveCellsData, CaveDugTO},
    PacketType,
};

use crate::{
    catalog::{
        items::{item_def, ItemKind},
        weapons::Melee,
    },
    structs::{
        components::{
            AimDir, Dirty, Health, ObjectBundle, ObjectEntity, ObjectKind, PadCooldown, PlayerEntity, Position,
            Resources, Trapped, Velocity,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        player_system::{random_land_spot, PLAYER_RADIUS},
        progression_system::{grant_weapon_xp, grant_xp},
        rng, Collider, ReactiveCollider,
    },
    CONFIG,
};

/// Distance between rock sample points.
pub const SAMPLE: f32 = 50.0;
/// Radius of a natural cave entrance.
pub const ENTRANCE_RADIUS: f32 = 55.0;
/// Room dug out under every entrance.
const ENTRANCE_ROOM: f32 = 90.0;
/// Natural caves made at world generation.
const NATURAL_CAVES: u32 = 18;
/// Steps of the random walk that carves each natural cave.
const CAVE_STEPS: u32 = 150;
/// Width of natural tunnels (radius).
const TUNNEL_RADIUS: f32 = 70.0;
/// Size of the occasional chamber (radius).
const CHAMBER_RADIUS: f32 = 150.0;
/// Digging needed to open one sample point.
const SAMPLE_HARDNESS: u32 = 6;
/// Radius of the hole one swing digs.
const DIG_RADIUS: f32 = 60.0;
/// Radius of a shovel's hole.
const SHOVEL_DIG_RADIUS: f32 = 85.0;
/// Most points that can be open at once (keeps the cave sync small).
const MAX_OPEN: usize = 120_000;
/// Stone for each dug point.
const STONE_PER_POINT: u32 = 1;
/// Chance (in %) that a dug point also has gold.
const GOLD_CHANCE: u32 = 4;
/// Gold in a gold point.
const GOLD_PER_POINT: u32 = 4;
/// XP for each dug point.
const XP_PER_POINT: u32 = 2;
/// Time between going down or up, in ms.
const LAYER_COOLDOWN_MS: u32 = 600;

/// Marks a player who is in the caves.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Underground;

/// A rock sample point (column, row).
pub type Point = (i16, i16);

/// The dug-out points and the digging progress on the others.
#[derive(Resource, Default)]
pub struct Caves {
    /// Open points.
    pub open:     HashSet<Point>,
    /// Digging done so far on closed points.
    pub progress: HashMap<Point, u32>,
}

impl Caves {
    /// Whether the point is dug out.
    pub fn is_open(&self, p: Point) -> bool {
        self.open.contains(&p)
    }

    /// Marching-squares case of the square whose top-left point is `(i, j)`.
    fn case(&self, i: i16, j: i16) -> usize {
        usize::from(self.is_open((i, j))) << 3
            | usize::from(self.is_open((i + 1, j))) << 2
            | usize::from(self.is_open((i + 1, j + 1))) << 1
            | usize::from(self.is_open((i, j + 1)))
    }

    /// Whether a world position is on the cave floor.
    pub fn is_floor(&self, x: f32, y: f32) -> bool {
        let square = square_at(x, y);
        let local = (x / SAMPLE - f32::from(square.0), y / SAMPLE - f32::from(square.1));
        point_in_polygon(&SHAPES[self.case(square.0, square.1)].polygon, local)
    }
}

// ---------------------------------------------------------------------------
// Marching squares

/// Corners and edge midpoints of a unit square: TL, TR, BR, BL, then top,
/// right, bottom and left midpoints.
const VERTS: [(f32, f32); 8] = [
    (0.0, 0.0),
    (1.0, 0.0),
    (1.0, 1.0),
    (0.0, 1.0),
    (0.5, 0.0),
    (1.0, 0.5),
    (0.5, 1.0),
    (0.0, 0.5),
];
/// Top-left corner.
const TL: usize = 0;
/// Top-right corner.
const TR: usize = 1;
/// Bottom-right corner.
const BR: usize = 2;
/// Bottom-left corner.
const BL: usize = 3;
/// Top midpoint.
const T: usize = 4;
/// Right midpoint.
const R: usize = 5;
/// Bottom midpoint.
const B: usize = 6;
/// Left midpoint.
const L: usize = 7;

/// Floor polygon for each case (bit 8 = TL open, 4 = TR, 2 = BR, 1 = BL).
/// The two saddle cases (5 and 10) are joined, so diagonal tunnels connect.
/// Must match `CASES` in `web/src/render/caves.js`.
const CASES: [&[usize]; 16] = [
    &[],
    &[L, B, BL],
    &[B, R, BR],
    &[L, R, BR, BL],
    &[T, TR, R],
    &[T, TR, R, B, BL, L],
    &[T, TR, BR, B],
    &[L, T, TR, BR, BL],
    &[TL, T, L],
    &[TL, T, B, BL],
    &[TL, T, R, BR, B, L],
    &[TL, T, R, BR, BL],
    &[TL, TR, R, L],
    &[TL, TR, R, B, BL],
    &[TL, TR, BR, B, L],
    &[TL, TR, BR, BL],
];

/// One marching-squares case: the floor polygon and its walls (the edges
/// that cut through the square), in unit square coordinates.
struct Shape {
    /// Floor polygon.
    polygon: Vec<(f32, f32)>,
    /// Walls between floor and rock.
    walls:   Vec<((f32, f32), (f32, f32))>,
}

/// All 16 cases.
static SHAPES: LazyLock<Vec<Shape>> = LazyLock::new(|| {
    CASES
        .iter()
        .map(|verts| {
            let polygon: Vec<(f32, f32)> = verts.iter().map(|&v| VERTS[v]).collect();
            let walls = (0..verts.len())
                .map(|k| (verts[k], verts[(k + 1) % verts.len()]))
                // Edges between two midpoints cut through the square.
                .filter(|&(a, b)| a >= T && b >= T)
                .map(|(a, b)| (VERTS[a], VERTS[b]))
                .collect();
            Shape { polygon, walls }
        })
        .collect()
});

/// Whether `p` is inside a polygon (ray casting). Empty polygons contain
/// nothing.
fn point_in_polygon(poly: &[(f32, f32)], (px, py): (f32, f32)) -> bool {
    let mut inside = false;
    let n = poly.len();
    for k in 0..n {
        let (x1, y1) = poly[k];
        let (x2, y2) = poly[(k + n - 1) % n];
        if (y1 > py) != (y2 > py) && px < (x2 - x1) * (py - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
    }
    inside
}

/// The square containing a position (its top-left point).
#[allow(
    clippy::cast_possible_truncation,
    reason = "positions are inside the map (at most 16384), so squares fit in i16"
)]
pub fn square_at(x: f32, y: f32) -> Point {
    ((x / SAMPLE).floor() as i16, (y / SAMPLE).floor() as i16)
}

/// World position of a sample point.
pub fn point_pos((i, j): Point) -> (f32, f32) {
    (f32::from(i) * SAMPLE, f32::from(j) * SAMPLE)
}

/// Whether a sample point is inside the map.
fn in_map((i, j): Point) -> bool {
    let (x, y) = point_pos((i, j));
    let size = f32::from(CONFIG.map.size);
    x > 0.0 && y > 0.0 && x < size && y < size
}

/// Sample points within `radius` of a position.
fn points_in_circle(x: f32, y: f32, radius: f32) -> Vec<Point> {
    let (i0, j0) = square_at(x - radius, y - radius);
    let (i1, j1) = square_at(x + radius, y + radius);
    let mut out = Vec::new();
    for i in i0..=i1 + 1 {
        for j in j0..=j1 + 1 {
            let (px, py) = point_pos((i, j));
            if (px - x).hypot(py - y) <= radius && in_map((i, j)) {
                out.push((i, j));
            }
        }
    }
    out
}

/// Whether the entity is underground.
pub fn is_underground(world: &World, entity: Entity) -> bool {
    world.get::<Underground>(entity).is_some()
}

// ---------------------------------------------------------------------------
// Generation and sync

/// Carves the natural caves and puts an entrance at the start of each.
pub fn generate_caves(world: &mut World, rng: &mut nanorand::WyRand) {
    let mut open = HashSet::new();
    let mut entrances = Vec::new();
    let size = f32::from(CONFIG.map.size);

    for _ in 0..NATURAL_CAVES {
        // Entrances go on clear ground, away from trees and rocks.
        let (mut x, mut y) = random_land_spot(world, ENTRANCE_RADIUS + 70.0);
        entrances.push((x, y));
        open.extend(points_in_circle(x, y, ENTRANCE_ROOM));
        let mut dir = rng.generate::<f32>() * std::f32::consts::TAU;
        for step in 0..CAVE_STEPS {
            let radius = if step % 25 == 24 { CHAMBER_RADIUS } else { TUNNEL_RADIUS };
            open.extend(points_in_circle(x, y, radius));
            // Wander: small turns all the time, big ones now and then.
            dir = (rng.generate::<f32>() - 0.5).mul_add(0.8, dir);
            if rng.generate_range(0..100_u32) < 6 {
                dir = (rng.generate::<f32>() - 0.5).mul_add(3.0, dir);
            }
            let (nx, ny) = (dir.cos().mul_add(SAMPLE, x), dir.sin().mul_add(SAMPLE, y));
            if nx < 200.0 || ny < 200.0 || nx > size - 200.0 || ny > size - 200.0 {
                dir += std::f32::consts::PI;
            } else {
                (x, y) = (nx, ny);
            }
        }
    }
    world.resource_mut::<Caves>().open = open;

    for (x, y) in entrances {
        world.spawn(ObjectBundle {
            object:   ObjectEntity,
            kind:     ObjectKind::CaveEntrance,
            pos:      Position(x, y),
            dir:      AimDir(0.0),
            health:   Health(1.0, 1.0),
            collider: Collider::circle(ENTRANCE_RADIUS),
        });
    }
}

/// Digs out a round room (under a new Tunnel, or an entrance being used).
pub fn open_area_at(world: &mut World, x: f32, y: f32) {
    let points: Vec<Point> = points_in_circle(x, y, ENTRANCE_ROOM)
        .into_iter()
        .filter(|&p| world.resource_mut::<Caves>().open.insert(p))
        .collect();
    broadcast_points(world, &points, Vec::new());
}

/// Tells everyone about newly opened (and partly dug) points.
fn broadcast_points(world: &mut World, points: &[Point], dug: Vec<CaveDugTO>) {
    if points.is_empty() && dug.is_empty() {
        return;
    }
    world.resource_mut::<Outbox>().send_reliable(
        Target::All,
        PacketType::CaveCells,
        &CaveCellsData {
            reset: false,
            cells: points.iter().map(|&(x, y)| CaveCellTO { x, y }).collect(),
            dug,
        },
    );
}

/// Digging progress as sent to clients (1-254).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 1-254 first"
)]
fn dug_amount(progress: u32) -> u8 {
    (f64::from(progress) / f64::from(SAMPLE_HARDNESS) * 255.0).clamp(1.0, 254.0) as u8
}

/// Sends a player every open point (on spawn).
pub fn send_all_cells(world: &mut World, id: u32) {
    let caves = world.resource::<Caves>();
    let cells = caves.open.iter().map(|&(x, y)| CaveCellTO { x, y }).collect();
    let dug = caves
        .progress
        .iter()
        .map(|(&(x, y), &p)| CaveDugTO {
            x,
            y,
            amount: dug_amount(p),
        })
        .collect();
    world.resource_mut::<Outbox>().send_reliable(
        Target::Player(id),
        PacketType::CaveCells,
        &CaveCellsData {
            reset: true,
            cells,
            dug,
        },
    );
}

// ---------------------------------------------------------------------------
// Going down and up

/// The entrance (natural or Tunnel) the player is standing on, if any.
fn entrance_at(world: &mut World, pos: Position) -> Option<Position> {
    world
        .query::<(&ObjectKind, &Position, &Collider)>()
        .iter(world)
        .find(|(kind, p, c)| {
            let is_entrance = match kind {
                ObjectKind::CaveEntrance => true,
                ObjectKind::Building(item) => item_def(*item).is_some_and(|i| i.kind == ItemKind::Tunnel),
                _ => false,
            };
            is_entrance && (p.0 - pos.0).hypot(p.1 - pos.1) < PLAYER_RADIUS.mul_add(0.5, c.rad)
        })
        .map(|(_, p, _)| *p)
}

/// Goes down (or up) the entrance the player is standing on.
pub fn toggle_layer(world: &mut World, entity: Entity) {
    let (Some(id), Some(pos)) = (
        world.get::<PlayerEntity>(entity).map(|p| p.0),
        world.get::<Position>(entity).copied(),
    ) else {
        return;
    };
    if world.get::<PadCooldown>(entity).is_some_and(|c| c.0 > 0) {
        return;
    }
    let Some(entrance) = entrance_at(world, pos) else {
        let text = if is_underground(world, entity) {
            "Stand under an entrance to climb up"
        } else {
            "Stand on a cave entrance or Tunnel to go down"
        };
        world.resource_mut::<Outbox>().notice(id, text, 1);
        return;
    };

    // Come out (or go in) right at the entrance.
    world.entity_mut(entity).insert((
        Position(entrance.0, entrance.1),
        Velocity(0.0, 0.0),
        PadCooldown(LAYER_COOLDOWN_MS),
    ));
    if is_underground(world, entity) {
        surface(world, entity);
    } else {
        // There's always room under an entrance.
        open_area_at(world, entrance.0, entrance.1);
        world
            .entity_mut(entity)
            .remove::<(ReactiveCollider, Trapped)>()
            .insert(Underground);
        world.resource_mut::<Outbox>().notice(
            id,
            "You're in the caves. Swing to dig, press X under an entrance to climb up.",
            0,
        );
    }
}

/// Brings a player back to the surface (also used when they join the arena).
pub fn surface(world: &mut World, entity: Entity) {
    if is_underground(world, entity) {
        world
            .entity_mut(entity)
            .remove::<Underground>()
            .insert(ReactiveCollider);
    }
}

// ---------------------------------------------------------------------------
// Digging

/// Digging strength of a melee weapon (shovels set their own).
pub const fn dig_power(m: &Melee) -> u32 {
    if m.dig > 0 {
        return m.dig;
    }
    let base = if m.gather == 0 { 1 } else { m.gather + 1 };
    if m.building_mult > 1.0 {
        base + 3
    } else {
        base
    }
}

/// Digs a round hole in front of an underground player.
pub fn dig(world: &mut World, entity: Entity, pos: Position, aim: f32, m: &Melee, weapon: u8) {
    let power = dig_power(m);
    let radius = if m.dig > 0 { SHOVEL_DIG_RADIUS } else { DIG_RADIUS };
    let reach = radius.mul_add(0.6, PLAYER_RADIUS);
    let (x, y) = (aim.cos().mul_add(reach, pos.0), aim.sin().mul_add(reach, pos.1));

    let mut opened = Vec::new();
    let mut dug = Vec::new();
    {
        let mut caves = world.resource_mut::<Caves>();
        if caves.open.len() >= MAX_OPEN {
            return;
        }
        for p in points_in_circle(x, y, radius) {
            if caves.is_open(p) {
                continue;
            }
            let progress = caves.progress.entry(p).or_insert(0);
            *progress += power;
            if *progress >= SAMPLE_HARDNESS {
                caves.progress.remove(&p);
                caves.open.insert(p);
                opened.push(p);
            } else {
                dug.push(CaveDugTO {
                    x:      p.0,
                    y:      p.1,
                    amount: dug_amount(*progress),
                });
            }
        }
    }
    broadcast_points(world, &opened, dug);
    if opened.is_empty() {
        return;
    }

    let count = u32::try_from(opened.len()).unwrap_or(u32::MAX);
    let mut rng = rng();
    let gold = (0..count)
        .filter(|_| rng.generate_range(0..100_u32) < GOLD_CHANCE)
        .count();
    let gold = u32::try_from(gold).unwrap_or(0) * GOLD_PER_POINT;
    if let Some(mut res) = world.get_mut::<Resources>(entity) {
        res.stone += count * STONE_PER_POINT;
        res.gold += gold;
    }
    world.resource_mut::<Dirty>().resources.insert(entity);
    grant_xp(world, entity, count * XP_PER_POINT);
    grant_weapon_xp(world, entity, weapon, count);
}

// ---------------------------------------------------------------------------
// Movement

/// Whether a point underground is inside rock.
pub fn in_rock(world: &World, x: f32, y: f32) -> bool {
    !world.resource::<Caves>().is_floor(x, y)
}

/// Keeps underground players on the cave floor: they slide along the walls.
pub fn cave_collision_system(world: &mut World) {
    let players: Vec<(Entity, Position, Velocity)> = world
        .query_filtered::<(Entity, &Position, &Velocity), With<Underground>>()
        .iter(world)
        .map(|(e, p, v)| (e, *p, *v))
        .collect();
    for (entity, pos, vel) in players {
        let (pos, vel) = resolve(world.resource::<Caves>(), pos, vel);
        world.entity_mut(entity).insert((pos, vel));
    }
}

/// Pushes a circle off the walls around it.
fn resolve(caves: &Caves, mut pos: Position, mut vel: Velocity) -> (Position, Velocity) {
    let r = PLAYER_RADIUS;

    // Inside rock (shouldn't happen): jump to the nearest open point.
    if !caves.is_floor(pos.0, pos.1) {
        let nearest = points_in_circle(pos.0, pos.1, SAMPLE * 3.0)
            .into_iter()
            .filter(|&p| caves.is_open(p))
            .map(point_pos)
            .min_by(|a, b| {
                let da = (a.0 - pos.0).hypot(a.1 - pos.1);
                let db = (b.0 - pos.0).hypot(b.1 - pos.1);
                da.total_cmp(&db)
            });
        return match nearest {
            Some((x, y)) => (Position(x, y), Velocity(0.0, 0.0)),
            None => (pos, Velocity(0.0, 0.0)),
        };
    }

    for _ in 0..3 {
        let (i0, j0) = square_at(pos.0 - r, pos.1 - r);
        let (i1, j1) = square_at(pos.0 + r, pos.1 + r);
        for i in i0..=i1 {
            for j in j0..=j1 {
                let (ox, oy) = point_pos((i, j));
                for &((ax, ay), (bx, by)) in &SHAPES[caves.case(i, j)].walls {
                    let a = (ax.mul_add(SAMPLE, ox), ay.mul_add(SAMPLE, oy));
                    let b = (bx.mul_add(SAMPLE, ox), by.mul_add(SAMPLE, oy));
                    let (qx, qy) = closest_on_segment(a, b, (pos.0, pos.1));
                    let (dx, dy) = (pos.0 - qx, pos.1 - qy);
                    let dist = dx.hypot(dy);
                    if dist >= r || dist <= f32::EPSILON {
                        continue;
                    }
                    let (nx, ny) = (dx / dist, dy / dist);
                    pos.0 = nx.mul_add(r, qx);
                    pos.1 = ny.mul_add(r, qy);
                    let vn = vel.0.mul_add(nx, vel.1 * ny);
                    if vn < 0.0 {
                        vel.0 = (-vn).mul_add(nx, vel.0);
                        vel.1 = (-vn).mul_add(ny, vel.1);
                    }
                }
            }
        }
    }
    (pos, vel)
}

/// Closest point to `p` on the segment `a`-`b`.
fn closest_on_segment(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> (f32, f32) {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len_sq = abx.mul_add(abx, aby * aby);
    if len_sq <= f32::EPSILON {
        return a;
    }
    let t = ((p.0 - a.0).mul_add(abx, (p.1 - a.1) * aby) / len_sq).clamp(0.0, 1.0);
    (abx.mul_add(t, a.0), aby.mul_add(t, a.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caves(points: &[Point]) -> Caves {
        Caves {
            open:     points.iter().copied().collect(),
            progress: HashMap::new(),
        }
    }

    /// A straight corridor along y = 500, three points wide (y 450..550).
    fn corridor() -> Caves {
        let mut pts = Vec::new();
        for i in 0..20 {
            for j in 9..=11 {
                pts.push((i, j));
            }
        }
        caves(&pts)
    }

    #[test]
    fn marching_squares_table() {
        assert_eq!(SHAPES[0].polygon.len(), 0);
        assert_eq!(SHAPES[15].polygon.len(), 4);
        assert_eq!(SHAPES[15].walls.len(), 0);
        // One open corner: a triangle with one wall.
        assert_eq!(SHAPES[8].walls.len(), 1);
        // Joined saddles have two walls.
        assert_eq!(SHAPES[5].walls.len(), 2);
        assert_eq!(SHAPES[10].walls.len(), 2);
    }

    #[test]
    fn floor_follows_the_open_points() {
        let c = corridor();
        assert!(c.is_floor(300.0, 500.0));
        assert!(c.is_floor(300.0, 460.0));
        assert!(!c.is_floor(300.0, 420.0), "the wall is halfway to the next point");
        assert!(!c.is_floor(300.0, 700.0));
    }

    #[test]
    fn walls_push_players_back() {
        let c = corridor();
        // The floor ends at y = 425 (halfway between rows 8 and 9).
        let (pos, vel) = resolve(&c, Position(300.0, 440.0), Velocity(0.0, -20.0));
        assert!((pos.1 - (425.0 + PLAYER_RADIUS)).abs() < 0.5, "{pos:?}");
        assert!(vel.1.abs() < 0.01);
        // Free to move along the corridor.
        let (pos, vel) = resolve(&c, Position(300.0, 500.0), Velocity(30.0, 0.0));
        assert_eq!((pos.0, pos.1, vel.0), (300.0, 500.0, 30.0));
    }

    #[test]
    fn stuck_in_rock_moves_to_open_point() {
        let c = caves(&[(5, 5)]);
        let (pos, _) = resolve(&c, Position(300.0, 300.0), Velocity(0.0, 0.0));
        assert_eq!((pos.0, pos.1), (250.0, 250.0));
    }

    #[test]
    fn shovels_dig_hardest() {
        let melee = |gather, dig| Melee {
            range: 60.0,
            damage: 10.0,
            knockback: 0.0,
            gather,
            building_mult: 1.0,
            block: 1.0,
            steal_gold: 0,
            dig,
        };
        assert_eq!(dig_power(&melee(0, 0)), 1);
        assert_eq!(dig_power(&melee(1, 0)), 2);
        assert!(dig_power(&melee(1, 12)) > dig_power(&melee(7, 0)));
    }
}
