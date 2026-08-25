use std::f32;

use bevy_ecs::{
    entity::Entity,
    query::With,
    system::{Query, ResMut},
};
use ratatui::layout::Positions;
use shared::objects::{GameObjects, StaticGameObjects};

use crate::{
    structs::{
        components::{
            AimDir, AttackState, HitEvent, HitEvents, ObjectEntity, PlayerEntity, Position, ReloadState, Resources,
        },
        weapons::{get_weapon_range, Weapon},
    },
    systems::Collider,
};

/// Attack System fn.
/// Handles Player attack actions.
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
        ),
        With<PlayerEntity>,
    >,
    object_targets: Query<(Entity, &Position, &Collider, &GameObjects), With<ObjectEntity>>,
    player_targets: Query<(Entity, &Position, &Collider), With<PlayerEntity>>,
    mut hit_events: ResMut<HitEvents>,
) {
    // Clear past hit events.
    hit_events.0.clear();

    // Tuple containing:
    // attacker id, attacker position, attacker collider, attacker weapon, attacker
    // reload state, attacker attack state, and attacker resources.
    for (attacker_id, pos, collider, weapon, mut reload_state, attack_state, aim, mut resources) in attackers.iter_mut()
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
            // // Store players hit by the attacker.
            // let mut players_hits = Vec::new();

            // // Loop through all players, collecting their id, position, and collider.
            // for (target_id, target_pos, target_collider) in player_targets.iter() {
            //     // Get the distance between the attacker and the target.
            //     let dist = get_distance(pos, target_pos);

            //     // If the distance between the attacker and the target is less than the
            //     // scales + the attacker's weapon range, we can proceed.
            //     if dist < collider.rad + target_collider.rad + get_weapon_range(weapon) {
            //         // TODO: fix angle logic.
            //         // Get the angle, in radians, of the attacker's position and the game object's
            //         // position.
            //         let angle = (pos.1 - target_pos.1).atan2(pos.0 - target_pos.0);

            //         // If the angle difference is greather than PI.
            //         if angle_diff(aim.0, angle).abs() > f32::consts::PI / 2.0 {
            //             // This satisfies all criteria, add it to the Vec.
            //             players_hits.push((target_id, angle));

                        
            //         }
            //     }
            // }

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

                        // Handle resources.
                        match game_obj {
                            GameObjects::StaticGameObjects(StaticGameObjects::Tree) => {
                                resources.0 += 1;
                            }
                            _ => {}
                        }
                        // break; // one object per swing
                    }
                }
            }

            // Register a HitEvent.
            hit_events.0.push(HitEvent {
                attacker: attacker_id,
                attacker_pos: *pos,
                affected: vec![],
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
