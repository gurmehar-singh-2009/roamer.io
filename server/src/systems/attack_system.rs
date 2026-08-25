use std::f32;

use bevy_ecs::{
    entity::Entity,
    query::With,
    system::{Query, ResMut},
};
use shared::objects::{GameObjects, StaticGameObjects};

use crate::{
    structs::{
        components::{
            AimDir, AnimalEntity, AttackState, Experience, HitEvent, HitEvents, Level, ObjectEntity, PlayerEntity,
            Position, ReloadState, Resources, UpgradeEvent, UpgradeEvents,
        },
        weapons::{get_weapon_range, get_weapon_resource_multiplier, weapon_upgrade_hierachy, Weapon},
    },
    systems::Collider,
};

/// Attack System fn.
/// Handles Player attack actions.
// I don't know how you could separate this into its own `type`. Tried, didn't work.
#[allow(clippy::type_complexity)]
pub fn attack_system(
    mut attackers: Query<
        (
            Entity,
            &Position,
            &Collider,
            &Weapon,
            &mut ReloadState,
            &AttackState,
            &AimDir,
            &mut Resources,
            &mut Experience,
            &mut Level,
        ),
        With<PlayerEntity>,
    >,
    object_targets: Query<(Entity, &Position, &Collider, &GameObjects), With<ObjectEntity>>,
    player_targets: Query<(Entity, &Position, &Collider), With<PlayerEntity>>,
    animal_targets: Query<(Entity, &Position, &Collider), With<AnimalEntity>>,
    mut hit_events: ResMut<HitEvents>,
    mut upgrade_events: ResMut<UpgradeEvents>,
) {
    // Clear past hit events.
    hit_events.0.clear();
    upgrade_events.0.clear();

    // Tuple containing:
    // attacker id, attacker position, attacker collider, attacker weapon, attacker
    // reload state, attacker attack state, and attacker resources.
    for (
        attacker_id,
        pos,
        collider,
        weapon,
        mut reload_state,
        attack_state,
        aim,
        mut resources,
        mut experience,
        mut level,
    ) in attackers.iter_mut()
    {
        // If the attacker reload is greater than zero.
        // This represents how much time left until we can register their next hit
        // event.
        if reload_state.0 > 0 {
            reload_state.0 = reload_state.0.saturating_sub(67);
        }
        // If the attacker's reload state is less than or equal to zero, we can register their next hit. AND if their
        // attack state is true, which represents if they're attacking or not.
        else if attack_state.0 {
            // Store objects hit by the attacker.
            let mut object_hits = Vec::new();

            // Loop through all players, collecting their entity, position, and collider.
            let mut players_hits: Vec<Entity> = Vec::new();

            let _animal_hits: Vec<Entity> = Vec::new();

            for (target_entity, target_pos, target_collider) in player_targets.iter() {
                // Prevent self-hit.
                if target_entity == attacker_id {
                    continue;
                }

                // Get the distance between the attacker and the target.
                let dist = get_distance(pos, target_pos);

                // If the distance between the attacker and the target is less than the
                // scales + the attacker's weapon range, we can proceed.
                if dist < collider.rad + target_collider.rad + get_weapon_range(weapon) {
                    // TODO: fix angle logic.
                    // Get the angle, in radians, of the attacker's position and the game object's
                    // position.
                    let angle = (pos.1 - target_pos.1).atan2(pos.0 - target_pos.0);

                    // If the angle difference is greather than PI.
                    if angle_diff(aim.0, angle).abs() > f32::consts::PI / 2.0 {
                        // This satisfies all criteria, add it to the Vec.
                        players_hits.push(target_entity);
                    }
                }
            }

            for (target_entity, target_pos, target_collider) in animal_targets.iter() {
                // Get the distance between the attacker and the target.
                let dist = get_distance(pos, target_pos);

                // If the distance between the attacker and the target is less than the
                // scales + the attacker's weapon range, we can proceed.
                if dist < collider.rad + target_collider.rad + get_weapon_range(weapon) {
                    // TODO: fix angle logic.
                    // Get the angle, in radians, of the attacker's position and the game object's
                    // position.
                    let angle = (pos.1 - target_pos.1).atan2(pos.0 - target_pos.0);

                    // If the angle difference is greather than PI.
                    if angle_diff(aim.0, angle).abs() > f32::consts::PI / 2.0 {
                        // This satisfies all criteria, add it to the Vec.
                        players_hits.push(target_entity);
                    }
                }
            }

            // Loop through all game objects, collecting their id, position, collider, and
            // their type.
            for (target_id, target_pos, target_collider, game_obj) in object_targets.iter() {
                // Get the distance between the attacker and the game object.
                let dist = get_distance(pos, target_pos);

                // If the distance between the attacker and the game object is less than the
                // scales + the attacker's weapon range, we can proceed.
                if dist < collider.rad + target_collider.rad + get_weapon_range(weapon) {
                    // TODO: fix angle logic.
                    // Get the angle, in radians, of the attacker's position and the game object's
                    // position.
                    let angle = (pos.1 - target_pos.1).atan2(pos.0 - target_pos.0);

                    // If the angle difference is greather than PI.
                    if angle_diff(aim.0, angle).abs() > f32::consts::PI / 2.0 {
                        // This satisfies all criteria, add it to the Vec.
                        object_hits.push((target_id, angle));

                        let resource_multiplier = get_weapon_resource_multiplier(weapon);

                        // Handle resources.
                        match game_obj {
                            GameObjects::StaticGameObjects(StaticGameObjects::Tree) => {
                                resources.0 += resource_multiplier;
                            }
                            GameObjects::StaticGameObjects(StaticGameObjects::Stone) => {
                                resources.1 += resource_multiplier;
                            }
                            GameObjects::StaticGameObjects(StaticGameObjects::Bush) => {
                                resources.2 += resource_multiplier;
                            }
                            _ => {}
                        }

                        experience.0 += resource_multiplier * 2;

                        if experience.0 >= experience.1 {
                            level.0 += 1;

                            experience.0 = 0;
                            experience.1 *= 2;

                            upgrade_events.0.push(UpgradeEvent {
                                id:      attacker_id,
                                weapons: weapon_upgrade_hierachy(level.0),
                            });
                        }

                        // break; // one object per swing
                    }
                }
            }

            // Register a HitEvent.
            hit_events.0.push(HitEvent {
                attacker: attacker_id,
                attacker_pos: *pos,
                affected: players_hits,
                object_hits,
            });

            // Set the current reload (cooldown) to the weapon reload speed.
            // Essentially keeps a delay between registering hit events.
            reload_state.0 = reload_state.1;
        }
    }
}

/// Function that returns the Euclidean distance between 2 points.
fn get_distance(a: &Position, b: &Position) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    ((dx * dx) + (dy * dy)).sqrt()
}

/// Function that returns the angle difference between 2 angles.
fn angle_diff(a: f32, b: f32) -> f32 {
    let diff = (a - b).rem_euclid(std::f32::consts::TAU);
    if diff > std::f32::consts::PI {
        diff - std::f32::consts::TAU
    } else {
        diff
    }
}
