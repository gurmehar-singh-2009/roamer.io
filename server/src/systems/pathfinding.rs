//! Animal pathfinding: A* over a coarse grid of the land, where cells near
//! trees, rocks, buildings and walls are blocked. Animals follow the path's
//! waypoints instead of walking straight into obstacles.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

use bevy_ecs::{component::Component, query::With, resource::Resource, world::World};

use crate::{
    structs::components::Position,
    systems::{Collider, NonReactiveCollider},
    CONFIG,
};

/// Size of one grid cell.
pub const NAV_CELL: f32 = 80.0;
/// Extra room kept around obstacles (about an animal's radius).
const CLEARANCE: f32 = 40.0;
/// Most cells one search may expand (keeps a search cheap).
const MAX_EXPANDED: usize = 2500;
/// Colliders bigger than this are the map's edge walls, not obstacles.
const MAX_OBSTACLE: f32 = 1000.0;

/// A grid cell (column, row).
pub type Cell = (i32, i32);

/// Which cells animals can walk through.
#[derive(Resource, Default)]
pub struct NavGrid {
    /// Columns.
    pub width:   i32,
    /// Rows.
    pub height:  i32,
    /// Blocked cells, row by row.
    pub blocked: Vec<bool>,
}

/// The path an animal is following.
#[derive(Component, Clone, Debug, Default)]
pub struct AnimalPath {
    /// Waypoints still to visit, next one last.
    pub waypoints: Vec<(f32, f32)>,
    /// Where the path leads.
    pub goal:      Option<(f32, f32)>,
    /// Time until the path is planned again.
    pub repath_ms: u32,
}

impl NavGrid {
    /// An open grid covering `width` x `height` world units.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the map is 16384 units, so the grid is a few hundred cells"
    )]
    pub fn new(width: f32, height: f32) -> Self {
        let w = (width / NAV_CELL).ceil() as i32;
        let h = (height / NAV_CELL).ceil() as i32;
        Self {
            width:   w,
            height:  h,
            blocked: vec![false; usize::try_from(w * h).unwrap_or(0)],
        }
    }

    /// Index of a cell in `blocked` (None outside the grid).
    fn index(&self, (x, y): Cell) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height)
            .then(|| usize::try_from(y * self.width + x).unwrap_or(0))
    }

    /// Whether a cell is walkable (cells outside the grid aren't).
    pub fn walkable(&self, cell: Cell) -> bool {
        self.index(cell).is_some_and(|i| !self.blocked[i])
    }

    /// The cell containing a position.
    #[allow(clippy::cast_possible_truncation, reason = "positions are inside the map")]
    pub fn cell_at(x: f32, y: f32) -> Cell {
        ((x / NAV_CELL).floor() as i32, (y / NAV_CELL).floor() as i32)
    }

    /// Center of a cell.
    #[allow(clippy::cast_precision_loss, reason = "cell numbers are small")]
    pub fn center((x, y): Cell) -> (f32, f32) {
        ((x as f32 + 0.5) * NAV_CELL, (y as f32 + 0.5) * NAV_CELL)
    }

    /// Marks the cells within `radius` (plus clearance) of a point.
    pub fn block_circle(&mut self, x: f32, y: f32, radius: f32) {
        let reach = radius + CLEARANCE;
        let (x0, y0) = Self::cell_at(x - reach, y - reach);
        let (x1, y1) = Self::cell_at(x + reach, y + reach);
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                let (px, py) = Self::center((cx, cy));
                if (px - x).hypot(py - y) <= reach {
                    if let Some(i) = self.index((cx, cy)) {
                        self.blocked[i] = true;
                    }
                }
            }
        }
    }

    /// Whether a straight line between two cells only crosses walkable cells.
    fn line_clear(&self, a: Cell, b: Cell) -> bool {
        let (mut x, mut y) = a;
        let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
        let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            if !self.walkable((x, y)) {
                return false;
            }
            if (x, y) == b {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// The walkable cell nearest to `cell` (within a few cells).
    fn nearest_walkable(&self, cell: Cell) -> Option<Cell> {
        if self.walkable(cell) {
            return Some(cell);
        }
        (1..=4).find_map(|r| {
            (-r..=r)
                .flat_map(|dx| (-r..=r).map(move |dy| (cell.0 + dx, cell.1 + dy)))
                .find(|&c| self.walkable(c))
        })
    }

    /// A* from `from` to `to`. Returns waypoints (world positions, the next
    /// one last) with straight stretches merged, or a path to the closest
    /// reachable cell if `to` can't be reached within the search budget.
    pub fn find_path(&self, from: (f32, f32), to: (f32, f32)) -> Option<Vec<(f32, f32)>> {
        let start = Self::cell_at(from.0, from.1);
        let goal = self.nearest_walkable(Self::cell_at(to.0, to.1))?;
        if start == goal {
            return Some(vec![to]);
        }
        let h = |c: Cell| {
            let (dx, dy) = ((c.0 - goal.0).abs(), (c.1 - goal.1).abs());
            // Octile distance in tenths of a cell.
            10 * dx.max(dy) + 4 * dx.min(dy)
        };

        let mut open = BinaryHeap::new();
        let mut came: HashMap<Cell, Cell> = HashMap::new();
        let mut cost: HashMap<Cell, i32> = HashMap::new();
        cost.insert(start, 0);
        open.push(Reverse((h(start), start)));
        let mut best = (h(start), start);
        let mut expanded = 0;

        while let Some(Reverse((_, cell))) = open.pop() {
            if cell == goal {
                best = (0, cell);
                break;
            }
            expanded += 1;
            if expanded > MAX_EXPANDED {
                break;
            }
            let g = cost[&cell];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let next = (cell.0 + dx, cell.1 + dy);
                if !self.walkable(next) {
                    continue;
                }
                // No cutting corners past blocked cells.
                if dx != 0
                    && dy != 0
                    && (!self.walkable((cell.0 + dx, cell.1)) || !self.walkable((cell.0, cell.1 + dy)))
                {
                    continue;
                }
                let step = if dx != 0 && dy != 0 { 14 } else { 10 };
                let ng = g + step;
                if cost.get(&next).is_none_or(|&c| ng < c) {
                    cost.insert(next, ng);
                    came.insert(next, cell);
                    let hn = h(next);
                    open.push(Reverse((ng + hn, next)));
                    if hn < best.0 {
                        best = (hn, next);
                    }
                }
            }
        }

        // Walk back from the goal (or the closest cell we got to).
        let mut cells = vec![best.1];
        while let Some(&prev) = came.get(cells.last()?) {
            cells.push(prev);
        }
        cells.reverse(); // start ... end

        // Keep only the turns: skip cells while there's a clear straight line.
        let mut points = Vec::new();
        let mut anchor = 0;
        let mut k = 1;
        while k < cells.len() {
            if k + 1 < cells.len() && self.line_clear(cells[anchor], cells[k + 1]) {
                k += 1;
                continue;
            }
            points.push(Self::center(cells[k]));
            anchor = k;
            k += 1;
        }
        if best.1 == goal {
            if let Some(last) = points.last_mut() {
                *last = to;
            }
        }
        points.reverse(); // next waypoint last
        Some(points)
    }
}

/// Rebuilds the grid from the static colliders (trees, rocks, buildings).
pub fn rebuild_nav_grid(world: &mut World) {
    let mut grid = NavGrid::new(f32::from(CONFIG.map.ocean_start_x), f32::from(CONFIG.map.size));
    for (pos, col) in world
        .query_filtered::<(&Position, &Collider), With<NonReactiveCollider>>()
        .iter(world)
    {
        if col.rad < MAX_OBSTACLE {
            grid.block_circle(pos.0, pos.1, col.rad);
        }
    }
    world.insert_resource(grid);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_go_around_walls() {
        let mut grid = NavGrid::new(2000.0, 2000.0);
        // A wall across the middle with a gap at the bottom.
        for y in 0..20 {
            if let Some(i) = grid.index((12, y)) {
                grid.blocked[i] = true;
            }
        }
        let path = grid.find_path((200.0, 200.0), (1800.0, 200.0)).unwrap();
        assert_eq!(path.first(), Some(&(1800.0, 200.0)), "ends at the goal");
        // Some waypoint goes below the wall's end (row 20 => y >= 1600).
        assert!(path.iter().any(|p| p.1 >= 1600.0), "{path:?}");
    }

    #[test]
    fn open_ground_is_a_straight_line() {
        let grid = NavGrid::new(2000.0, 2000.0);
        let path = grid.find_path((100.0, 100.0), (1500.0, 900.0)).unwrap();
        assert_eq!(path, vec![(1500.0, 900.0)]);
    }

    #[test]
    fn unreachable_goal_gets_close() {
        let mut grid = NavGrid::new(1000.0, 1000.0);
        // Box the goal in.
        for c in [(9, 9), (9, 10), (9, 11), (10, 9), (10, 11), (11, 9), (11, 10), (11, 11)] {
            if let Some(i) = grid.index(c) {
                grid.blocked[i] = true;
            }
        }
        assert!(grid.find_path((100.0, 100.0), (840.0, 840.0)).is_some());
    }
}
