//! ECS components and the per-tick resources systems use to talk to each
//! other.

use std::collections::HashMap;

use bevy_ecs::{bundle::Bundle, component::Component, entity::Entity, resource::Resource};

use crate::{
    catalog::Owned,
    systems::{Collider, NonReactiveCollider, ReactiveCollider},
};

/// World position (x, y).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Position(pub f32, pub f32);

/// Velocity (vx, vy).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Velocity(pub f32, pub f32);

/// Player display name.
#[derive(Component, Clone, Debug)]
pub struct Name(pub String);

/// Marks a player. Holds their player id.
#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerEntity(pub u32);

/// An animal. Holds its catalog id.
#[derive(Component, Clone, Copy, Debug)]
pub struct AnimalEntity(pub u8);

/// Marks a map object (tree, stone, crate, building...).
#[derive(Component)]
pub struct ObjectEntity;

/// What a map object is.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// Gives wood.
    Tree,
    /// Gives stone.
    Stone,
    /// Gives food.
    Bush,
    /// Gives gold.
    Gold,
    /// Loot crate; the tier is the weapon variant needed to open it.
    Crate(u8),
    /// A player building (item id).
    Building(u8),
    /// A natural way down into the caves.
    CaveEntrance,
}

impl ObjectKind {
    /// The object type id sent to clients.
    pub const fn type_id(self) -> u8 {
        match self {
            Self::Tree => 0,
            Self::Stone => 1,
            Self::Bush => 2,
            Self::Gold => 3,
            Self::CaveEntrance => 4,
            Self::Crate(tier) => 10 + tier,
            Self::Building(item) => 50 + item,
        }
    }
}

/// A player's resources.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resources {
    /// Wood.
    pub wood:  u32,
    /// Stone.
    pub stone: u32,
    /// Food.
    pub food:  u32,
    /// Gold (also the score).
    pub gold:  u32,
    /// Gun ammo.
    pub ammo:  u32,
}

/// Age and XP.
#[derive(Component, Clone, Copy, Debug)]
pub struct Progress {
    /// Current age (starts at 1).
    pub age:         u32,
    /// XP into the current age.
    pub xp:          u32,
    /// The next age whose upgrade hasn't been picked yet.
    pub upgrade_age: u32,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            age:         1,
            xp:          0,
            upgrade_age: 2,
        }
    }
}

/// Weapons and items owned.
#[derive(Component, Clone, Debug)]
pub struct Loadout(pub Owned);

/// The weapon being held (weapon id).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapon(pub u8);

/// XP earned with each weapon, for variants.
#[derive(Component, Clone, Debug, Default)]
pub struct WeaponXp(pub HashMap<u8, u32>);

impl WeaponXp {
    /// Variant reached with a weapon.
    pub fn variant(&self, weapon: u8) -> u8 {
        crate::catalog::weapons::variant_for_xp(self.0.get(&weapon).copied().unwrap_or(0))
    }
}

/// Hats and accessories owned and worn.
#[derive(Component, Clone, Debug, Default)]
pub struct Gear {
    /// Worn hat (0 = none).
    pub hat:         u8,
    /// Worn accessory (0 = none).
    pub accessory:   u8,
    /// Owned hats.
    pub hats:        Vec<u8>,
    /// Owned accessories.
    pub accessories: Vec<u8>,
    /// Skin color (from the account).
    pub skin:        u8,
}

/// Per-life stats.
#[derive(Component, Clone, Debug, Default)]
pub struct PlayerStats {
    /// Players killed.
    pub kills:      u32,
    /// Tick the player spawned on.
    pub spawn_tick: u64,
    /// Last player to damage them: (player id, name).
    pub last_hit:   Option<(u32, String)>,
}

/// Movement direction in radians, `None` when standing still.
#[derive(Component)]
pub struct MoveDir(pub Option<f32>);

/// Aim direction in radians.
#[derive(Component, Clone, Copy)]
pub struct AimDir(pub f32);

/// Health: current, max.
#[derive(Component, Clone, Copy, Debug)]
pub struct Health(pub f32, pub f32);

/// Whether the player is attacking.
#[derive(Component, Clone, Debug)]
pub struct AttackState(pub bool);

/// Attack cooldown in ms: remaining, full.
#[derive(Component, Clone, Copy, Debug)]
pub struct ReloadState(pub u32, pub u32);

/// Damage over time.
#[derive(Component, Clone, Copy, Debug)]
pub struct Poisoned {
    /// Damage per second.
    pub dps:          f32,
    /// Milliseconds left.
    pub remaining_ms: u32,
    /// Player who poisoned them.
    pub source:       Option<u32>,
}

/// A player knocked back by an animal: no control for this many ms.
#[derive(Component, Clone, Copy, Debug)]
pub struct Stagger(pub u32);

/// Stuck in a pit trap.
#[derive(Component, Clone, Copy, Debug)]
pub struct Trapped(pub Entity);

/// Milliseconds until pads (boost, teleporter) affect this player again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PadCooldown(pub u32);

/// The game tick the player last ate on. Used to limit eating to once per tick.
#[derive(Component)]
pub struct LastEatTick(pub u64);

/// Account token linking this player to saved lifetime stats.
#[derive(Component, Clone, Debug)]
pub struct Account(pub String);

/// Arena room the player is fighting in.
#[derive(Component, Clone, Copy, Debug)]
pub struct InArena(pub u32);

/// What an animal is doing.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum AiState {
    /// Walking to a random spot.
    Wander,
    /// Chasing an entity.
    Chase(Entity),
    /// Running from a position.
    Flee {
        /// Where the threat was.
        from: (f32, f32),
        /// Milliseconds left.
        ms:   u32,
    },
}

/// Wander target: x, y.
#[derive(Component, Clone, Copy, Debug)]
pub struct AiTarget(pub f32, pub f32);

/// Milliseconds until an animal can bite again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct BiteCooldown(pub u32);

/// Components every player has.
#[derive(Bundle)]
pub struct PlayerBundle {
    /// Player id.
    pub player:   PlayerEntity,
    /// Name.
    pub name:     Name,
    /// Position.
    pub pos:      Position,
    /// Velocity.
    pub vel:      Velocity,
    /// Movement input.
    pub move_dir: MoveDir,
    /// Aim input.
    pub aim:      AimDir,
    /// Held weapon.
    pub weapon:   Weapon,
    /// Attack cooldown.
    pub reload:   ReloadState,
    /// Health.
    pub health:   Health,
    /// Attack input.
    pub attack:   AttackState,
    /// Resources.
    pub res:      Resources,
    /// Age and XP.
    pub progress: Progress,
    /// Owned weapons and items.
    pub loadout:  Loadout,
    /// Weapon XP.
    pub wxp:      WeaponXp,
    /// Hats.
    pub gear:     Gear,
    /// Stats.
    pub stats:    PlayerStats,
    /// Collider.
    pub collider: Collider,
    /// Pushed by collisions.
    pub reactive: ReactiveCollider,
    /// Eating limit.
    pub eat:      LastEatTick,
    /// Pad cooldown.
    pub pads:     PadCooldown,
}

/// Components every map object has.
#[derive(Bundle)]
pub struct ObjectBundle {
    /// Marker.
    pub object:   ObjectEntity,
    /// What it is.
    pub kind:     ObjectKind,
    /// Position.
    pub pos:      Position,
    /// Rotation.
    pub dir:      AimDir,
    /// Health.
    pub health:   Health,
    /// Collider.
    pub collider: Collider,
}

/// Natural objects and walls block movement.
#[derive(Bundle)]
pub struct SolidObjectBundle {
    /// The object.
    pub object: ObjectBundle,
    /// Blocks movement.
    pub solid:  NonReactiveCollider,
}

/// A building placed by a player.
#[derive(Component, Clone, Copy, Debug)]
pub struct Building {
    /// Player id (not entity) of the owner, so it survives respawns.
    pub owner:         u32,
    /// Item id.
    pub item:          u8,
    /// Milliseconds until the next regeneration step.
    pub regen_wait_ms: u32,
    /// Milliseconds until its next action (turret shot, TNT blast, resource
    /// tick).
    pub timer_ms:      u32,
}

/// Resources stored in a chest.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ChestStore(pub Resources);

/// A projectile in flight. Projectiles have no `Position` so the other
/// systems ignore them.
#[derive(Component, Clone, Copy, Debug)]
pub struct Projectile {
    /// Id sent to clients.
    pub id:              u32,
    /// Who fired it (can't hit themselves).
    pub owner:           Option<Entity>,
    /// Owner's player id, for kill credit.
    pub owner_player:    Option<u32>,
    /// Weapon that fired it (255 for turrets).
    pub weapon:          u8,
    /// Weapon variant when fired.
    pub variant:         u8,
    /// Current position.
    pub x:               f32,
    /// Current position.
    pub y:               f32,
    /// Unit direction.
    pub dir:             (f32, f32),
    /// Distance per tick.
    pub speed:           f32,
    /// Distance left before it disappears.
    pub range_left:      f32,
    /// Damage to players and animals.
    pub damage:          f32,
    /// Damage to buildings.
    pub building_damage: f32,
    /// Knockback.
    pub knockback:       f32,
    /// Flies over buildings (fired from a platform).
    pub over_buildings:  bool,
    /// Flies through the caves (only hits underground players, stops at rock).
    pub underground:     bool,
}

/// What dealt some damage.
#[derive(Clone, Debug, Default)]
pub struct DamageSource {
    /// Entity responsible (player, animal, turret).
    pub entity:  Option<Entity>,
    /// Player responsible, for kill credit and friendly fire.
    pub player:  Option<u32>,
    /// Where it came from (for shields and knockback).
    pub pos:     (f32, f32),
    /// Weapon variant (crates need a high enough one).
    pub variant: u8,
    /// Whether this was a melee hit (for spike gear reflection).
    pub melee:   bool,
}

/// One instance of damage to apply.
#[derive(Clone, Debug)]
pub struct Damage {
    /// Who gets hit.
    pub target:          Entity,
    /// Damage to players and animals.
    pub amount:          f32,
    /// Damage to buildings and crates.
    pub building_amount: f32,
    /// Knockback speed (away from the source).
    pub knockback:       f32,
    /// Poison the target.
    pub poison:          bool,
    /// Fraction healed back to the attacker.
    pub lifesteal:       f32,
    /// Weapon XP goes to this weapon of the attacker.
    pub weapon:          Option<u8>,
    /// Gold stolen (`McGrabby`).
    pub steal_gold:      u32,
    /// Who did it.
    pub source:          DamageSource,
}

impl Default for Damage {
    fn default() -> Self {
        Self {
            target:          Entity::PLACEHOLDER,
            amount:          0.0,
            building_amount: 0.0,
            knockback:       0.0,
            poison:          false,
            lifesteal:       0.0,
            weapon:          None,
            steal_gold:      0,
            source:          DamageSource::default(),
        }
    }
}

/// Damage queued this tick, applied by `damage_system`.
#[derive(Resource, Default)]
pub struct DamageEvents(pub Vec<Damage>);

/// Players that attacked this tick (swing / recoil animation): player ids.
#[derive(Resource, Default)]
pub struct AttackAnims(pub Vec<u32>);

/// Players who died this tick: (player id, killer).
#[derive(Resource, Default)]
pub struct Deaths(pub Vec<(u32, Option<(u32, String)>)>);

/// Number of game ticks since the server started.
#[derive(Resource, Default)]
pub struct TickCount(pub u64);

/// Players whose resources, loadout, age or upgrades changed this tick, so
/// they get fresh state messages.
#[derive(Resource, Default)]
pub struct Dirty {
    /// Resources changed.
    pub resources: std::collections::HashSet<Entity>,
    /// Loadout or weapon XP changed.
    pub loadout:   std::collections::HashSet<Entity>,
    /// Age or XP changed.
    pub age:       std::collections::HashSet<Entity>,
}
