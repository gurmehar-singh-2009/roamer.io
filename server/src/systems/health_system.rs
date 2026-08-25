use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::structs::bevy::PlayerMap;
use crate::structs::components::{Health, HealthUpdates, HitEvents, Position, Velocity};
use crate::structs::weapons::{get_weapon_damage, get_weapon_knockback};

/// Applies accumulated HitEvents to Health components and records health
/// updates.
pub fn health_system(
    hit_events: ResMut<HitEvents>,
    mut health_q: Query<&mut Health>,
    mut vel_q: Query<&mut Velocity>,
    pos_q: Query<&Position>,
    weapon_q: Query<&crate::structs::weapons::Weapon>,
    player_map: Res<PlayerMap>,
    mut health_updates: ResMut<HealthUpdates>,
) {
    // Build a mapping from entity -> player id for broadcasts.
    let entity_to_id: std::collections::HashMap<bevy_ecs::entity::Entity, u32> =
        player_map.map.iter().map(|e| (*e.1, *e.0)).collect();

    let mut updates = Vec::new();

    for hit in hit_events.0.iter() {
        let weapon = weapon_q
            .get(hit.attacker)
            .unwrap_or(&crate::structs::weapons::Weapon::Fists);

        let damage = get_weapon_damage(weapon);

        for target in hit.affected.iter().copied() {
            if let Ok(mut health) = health_q.get_mut(target) {
                health.0 = (health.0 - damage).max(0.0);
                if let Some(&pid) = entity_to_id.get(&target) {
                    updates.push((pid, health.0));
                }
            }

            if let (Ok(attacker_pos), Ok(target_pos)) = (pos_q.get(hit.attacker), pos_q.get(target)) {
                let dx = target_pos.0 - attacker_pos.0;
                let dy = target_pos.1 - attacker_pos.1;

                let len = (dx * dx + dy * dy).sqrt();

                if len > 0.0 {
                    if let Ok(mut vel) = vel_q.get_mut(target) {
                        vel.0 += dx / len * get_weapon_knockback(weapon);
                        vel.1 += dy / len * get_weapon_knockback(weapon);
                    }
                }
            }
        }
    }

    // Append updates so broadcast_state can pick them up.
    health_updates.0.extend(updates);
}
