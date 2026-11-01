/// Accounts: lifetime stats, coins and skins.
pub mod accounts;
/// Animal behavior.
mod animal_ai_system;
/// Script detection.
pub mod anticheat_system;
/// Arena team fights.
pub mod arena_system;
/// Player attacks, turrets.
pub mod attack_system;
/// Items and buildings.
pub mod building_system;
/// Tunnels and caves.
pub mod cave_system;
/// Collision resolution.
mod collision_system;
/// Applying damage.
pub mod damage_system;
/// Status effects.
pub mod health_system;
/// World generation and upkeep.
pub mod map_system;
/// Player movement.
mod movement_system;
/// Animal pathfinding.
pub mod pathfinding;
/// Spawning, dying and leaving.
pub mod player_system;
/// XP, ages, upgrades and weapon variants.
pub mod progression_system;
/// Clans, shop, quests, events, leaderboard, minimap.
pub mod social_system;
/// Spatial index rebuilding.
mod spatial_index_system;
/// Projectiles.
pub mod weapon_system;

use std::sync::atomic::{AtomicU64, Ordering};

pub use animal_ai_system::*;
pub use collision_system::*;
pub use movement_system::*;
pub use spatial_index_system::*;

/// Length of one game tick in milliseconds.
pub const TICK_MS: u32 = 67;

/// Seed counter for `rng`.
static RNG_SEED: AtomicU64 = AtomicU64::new(0x5eed);

/// A random number generator with a fresh seed (not for secrets).
pub fn rng() -> nanorand::WyRand {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    nanorand::WyRand::new_seed(RNG_SEED.fetch_add(1, Ordering::Relaxed) ^ u64::from(nanos))
}
