use bevy_ecs::{bundle::Bundle, component::Component, entity::Entity};
use shared::objects::GameObjects;

use crate::{
    structs::weapons::Weapon,
    systems::{Collider, NonReactiveCollider, ReactiveCollider},
};

#[derive(Component, Clone, Copy, Debug)]
// x, y
pub struct Position(pub f32, pub f32);

#[derive(Component, Clone, Copy, Debug)]
// vx, vy
pub struct Velocity(pub f32, pub f32);

#[derive(Component, Clone, Debug)]
pub struct Name(pub String);

#[derive(Component)]
#[allow(dead_code)]
pub struct Wall;

#[derive(Component)]
pub struct PlayerEntity;

#[derive(Component)]
pub struct AnimalEntity;

#[derive(Component)]
pub struct ObjectEntity;

#[derive(Component)]
// wood, stone, berry, gold, kills
#[allow(dead_code)]
pub struct Resources(pub u32, pub u32, pub u32, pub u32, pub u32);

#[derive(Component)]
// current, amount needed to level up
pub struct Experience(pub u32, pub u32);

#[derive(Component)]
// current level
pub struct Level(pub u32);

#[derive(Component)]
pub struct MoveDir(pub Option<f32>);

#[derive(Component)]
pub struct AimDir(pub f32);

#[derive(Component, Clone, Copy, Debug)]
#[repr(u8)]
pub enum AnimalType {
    Wolf,
    Fish,
}

#[derive(Component, Clone, Copy, Debug)]
// current, max
#[allow(dead_code)]
pub struct Health(pub f32, pub f32);

#[derive(Component, Clone, Debug)]
pub struct AttackState(pub bool);

// potential use in the future
#[derive(Component, Clone, Copy, Debug)]
pub enum AiState {
    Idle,
    Wander,
    Chase,
    Flee,
    Eat,
}

#[derive(Component, Clone, Copy, Debug)]
// current, max
pub struct ReloadState(pub u32, pub u32);

#[derive(Component, Clone, Copy, Debug)]
// target, x, y
#[allow(dead_code)]
pub struct AiTarget(pub Option<Entity>, pub f32, pub f32);

#[derive(Bundle)]
pub struct PlayerBundle(
    pub PlayerEntity,
    pub Name,
    pub Position,
    pub Velocity,
    pub MoveDir,
    pub AimDir,
    pub Weapon,
    pub ReloadState,
    pub Health,
    pub AttackState,
    pub Resources,
    pub Experience,
    pub Level,
    pub Collider,
    pub ReactiveCollider,
);

#[derive(Bundle)]
pub struct ObjectBundle(
    pub ObjectEntity,
    pub GameObjects,
    pub Position,
    pub AimDir,
    pub Health,
    pub Resources,
    pub Collider,
    pub NonReactiveCollider,
);

#[derive(bevy_ecs::prelude::Resource, Default)]
pub struct HitEvents(pub Vec<HitEvent>);

#[derive(bevy_ecs::prelude::Resource, Default)]
pub struct HealthUpdates(pub Vec<(u32, f32)>);

pub struct HitEvent {
    pub attacker:     Entity,
    pub attacker_pos: Position,
    #[allow(dead_code)]
    pub affected:     Vec<Entity>,
    pub object_hits:  Vec<(Entity, f32)>,
}

#[derive(bevy_ecs::prelude::Resource, Default)]
pub struct UpgradeEvents(pub Vec<UpgradeEvent>);

pub struct UpgradeEvent {
    pub id:      Entity,
    pub weapons: Vec<Weapon>,
}

#[allow(dead_code)]
pub struct PlayerHitEvent {
    pub attacker:     Entity,
    pub attacker_pos: Position,
    pub affected:     Vec<Entity>,
    pub object_hits:  Vec<(Entity, f32)>,
}
