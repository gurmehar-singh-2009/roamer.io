//! Script detection: perfect aim and unusual hat switching. Flagged players
//! are disconnected and their IP is blacklisted for a day.

use std::{
    collections::{HashMap, VecDeque},
    sync::LazyLock,
    time::{Duration, Instant},
};

use bevy_ecs::{entity::Entity, resource::Resource, world::World};
use dashmap::DashMap;

use crate::{
    structs::components::{AimDir, PlayerEntity, Position, TickCount, Velocity},
    systems::{social_system::are_allies, TICK_MS},
};

/// How long a blacklisted IP stays blocked.
pub const BLACKLIST_TIME: Duration = Duration::from_hours(24);
/// Attacks remembered for the aim check.
const AIM_WINDOW: usize = 20;
/// How many of those must be perfect to flag.
const AIM_FLAG: usize = 18;
/// Aim closer than this to the exact target angle counts as perfect.
const PERFECT_AIM: f32 = 0.002;
/// Only aim at moving targets is checked (anyone can aim at a still target).
const MIN_TARGET_SPEED: f32 = 10.0;
/// Hat switches counted over this window...
const HAT_WINDOW_MS: u32 = 5000;
/// ...more than this many is flagged.
const HAT_FLAG: usize = 30;

/// Blacklisted IPs and when their ban ends.
static BLACKLIST: LazyLock<DashMap<String, Instant>> = LazyLock::new(DashMap::new);

/// Whether an IP is blacklisted right now.
pub fn is_blacklisted(ip: &str) -> bool {
    match BLACKLIST.get(ip).map(|until| *until) {
        Some(until) if until > Instant::now() => true,
        Some(_) => {
            BLACKLIST.remove(ip);
            false
        }
        None => false,
    }
}

/// Blacklists an IP.
pub fn blacklist(ip: &str) {
    BLACKLIST.insert(ip.to_string(), Instant::now() + BLACKLIST_TIME);
}

/// Players to disconnect and blacklist: (player id, reason).
#[derive(Resource, Default)]
pub struct Kicks(pub Vec<(u32, &'static str)>);

/// Per-player history used by the checks.
#[derive(Resource, Default)]
pub struct CheatHistory {
    /// Whether each recent attack had perfect aim.
    aim:  HashMap<u32, VecDeque<bool>>,
    /// Ticks of recent hat switches.
    hats: HashMap<u32, VecDeque<u64>>,
}

impl CheatHistory {
    /// Forgets a player.
    pub fn remove(&mut self, id: u32) {
        self.aim.remove(&id);
        self.hats.remove(&id);
    }
}

/// Records a hat switch. Returns true if the player was flagged.
pub fn note_hat_switch(world: &mut World, id: u32) -> bool {
    let tick = world.resource::<TickCount>().0;
    let window = u64::from(HAT_WINDOW_MS / TICK_MS);
    let mut history = world.resource_mut::<CheatHistory>();
    let switches = history.hats.entry(id).or_default();
    switches.push_back(tick);
    while switches.front().is_some_and(|&t| t + window < tick) {
        switches.pop_front();
    }
    if switches.len() > HAT_FLAG {
        history.remove(id);
        world.resource_mut::<Kicks>().0.push((id, "hat switching script"));
        return true;
    }
    false
}

/// Checks an attack's aim against the nearest moving enemy.
pub fn note_attack(world: &mut World, attacker: Entity) {
    let (Some(id), Some(pos), Some(aim)) = (
        world.get::<PlayerEntity>(attacker).map(|p| p.0),
        world.get::<Position>(attacker).copied(),
        world.get::<AimDir>(attacker).map(|a| a.0),
    ) else {
        return;
    };

    let nearest = world
        .query::<(Entity, &PlayerEntity, &Position, &Velocity)>()
        .iter(world)
        .filter(|(e, p, ..)| *e != attacker && !are_allies(world, id, p.0))
        .map(|(_, _, p, v)| (*p, *v, (p.0 - pos.0).hypot(p.1 - pos.1)))
        .filter(|(.., d)| *d < 800.0)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((target, vel, _)) = nearest else {
        return;
    };
    if vel.0.hypot(vel.1) < MIN_TARGET_SPEED {
        return;
    }

    let exact = (target.1 - pos.1).atan2(target.0 - pos.0);
    let perfect = crate::systems::attack_system::angle_diff(aim, exact).abs() < PERFECT_AIM;

    let mut history = world.resource_mut::<CheatHistory>();
    let shots = history.aim.entry(id).or_default();
    shots.push_back(perfect);
    if shots.len() > AIM_WINDOW {
        shots.pop_front();
    }
    if shots.len() == AIM_WINDOW && shots.iter().filter(|&&p| p).count() >= AIM_FLAG {
        history.remove(id);
        world.resource_mut::<Kicks>().0.push((id, "aim script"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blacklist_blocks_and_expires() {
        assert!(!is_blacklisted("10.9.8.7"));
        blacklist("10.9.8.7");
        assert!(is_blacklisted("10.9.8.7"));
        BLACKLIST.insert(
            "10.9.8.6".into(),
            Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
        );
        assert!(!is_blacklisted("10.9.8.6"));
    }

    #[test]
    fn rapid_hat_switching_is_flagged() {
        let mut world = World::new();
        world.insert_resource(TickCount(100));
        world.insert_resource(CheatHistory::default());
        world.insert_resource(Kicks::default());
        world.insert_resource(crate::systems::player_system::Sessions::default());
        for _ in 0..HAT_FLAG {
            assert!(!note_hat_switch(&mut world, 1));
        }
        assert!(note_hat_switch(&mut world, 1));
        assert_eq!(world.resource::<Kicks>().0.len(), 1);
    }
}
