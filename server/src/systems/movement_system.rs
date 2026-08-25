use bevy_ecs::prelude::*;

use crate::{
    structs::components::{AttackState, MoveDir, PlayerEntity, Position, ReloadState, Velocity},
    CONFIG,
};

// TODO: move into config file.
const PLAYER_MAX_SPEED: f32 = 45.0;
const PLAYER_ACCEL: f32 = 120.0;
const PLAYER_FRICTION: f32 = 90.0;
const SNOW_FRICTION: f32 = 67.0;
const DT: f32 = 0.45;

/// System that handles player movement.
pub fn movement_system(
    mut query: Query<(&mut Position, &mut Velocity, &MoveDir, &ReloadState, &AttackState), With<PlayerEntity>>,
) {
    let dt = DT;

    // This threaded loop iterates through each query entry.
    query
        .par_iter_mut()
        .for_each(|(mut pos, mut vel, move_dir, reload_state, attack_state)| {
            let mut vx = vel.0;
            let mut vy = vel.1;

            // Apply deceleration.
            let mut decel = 1_f32;
            if reload_state.0 == 0 && attack_state.0 {
                decel *= 0.;
            }
            if pos.0 > CONFIG.map.ocean_start_x as f32 {
                decel *= 0.6;
            }

            // If the player is moving in any direction.
            if let Some(dir) = move_dir.0 {
                let target_vx = dir.cos() * PLAYER_MAX_SPEED;
                let target_vy = dir.sin() * PLAYER_MAX_SPEED;

                let dx = target_vx - vx;
                let dy = target_vy - vy;

                let dist = (dx * dx + dy * dy).sqrt();
                let max_step = PLAYER_ACCEL * dt;

                if dist > max_step {
                    vx += dx / dist * max_step;
                    vy += dy / dist * max_step;
                } else {
                    vx = target_vx;
                    vy = target_vy;
                }

                vx *= decel;
                vy *= decel;
            } else {
                let speed = (vx * vx + vy * vy).sqrt();
                

                let drop = if pos.1 < 4096. {
                    SNOW_FRICTION * dt
                } else {
                    PLAYER_FRICTION * dt
                };

                if speed > drop {
                    vx -= vx / speed * drop;
                    vy -= vy / speed * drop;
                } else {
                    vx = 0.0;
                    vy = 0.0;
                }
            }

            // if pos.0 > CONFIG.map.ocean_start_x as f32 {
            //     let (center_x, center_y) = (
            //         CONFIG.map.size as f32 - CONFIG.map.ocean_start_x as f32 / 2.,
            //         CONFIG.map.size as f32 / 2.,
            //     );
            //     let xx = center_x - pos.0;
            //     let yy = center_y - pos.1;

            //     let istance = (xx.powf(2.) + yy.powf(2.)).sqrt();

            //     vx += -xx / istance * 15.;
            //     vy += -yy / istance * 15.;

            //     vx += -yy / istance * 67.;
            //     vy += -xx / istance * 67.;

            //     // vx += istance.cos();
            //     // vy += istance.sin();
            // }

            vx = vx.clamp(-67., 67.);
            vy = vy.clamp(-67., 67.);

            vel.0 = vx;
            vel.1 = vy;

            pos.0 += vx * dt;
            pos.1 += vy * dt;
        });
}
