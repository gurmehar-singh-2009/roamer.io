//! Player movement.

use bevy_ecs::prelude::*;

use crate::{
    catalog::{hats::hat_effects, weapons::weapon_or_fists},
    structs::components::{
        AttackState, Gear, MoveDir, PlayerEntity, Position, ReloadState, Stagger, Trapped, Velocity, Weapon,
    },
    systems::{cave_system::Underground, TICK_MS},
    CONFIG,
};

/// Top walking speed.
pub const PLAYER_MAX_SPEED: f32 = 45.0;
/// Acceleration while moving.
const PLAYER_ACCEL: f32 = 120.0;
/// Deceleration when not moving.
const PLAYER_FRICTION: f32 = 90.0;
/// Deceleration on ice (snow biome): you glide for a while before stopping.
const SNOW_FRICTION: f32 = 4.0;
/// Acceleration on ice: speeding up, turning and stopping all take time, so
/// you drift.
const SNOW_ACCEL: f32 = 12.0;
/// Speed kept each tick while swimming (flipper hat removes the slowdown).
const WATER_DRAG: f32 = 0.6;
/// Fastest a player can move along each axis.
const MAX_AXIS_SPEED: f32 = 67.0;
/// Deceleration while staggered (so the knockback carries).
const STAGGER_FRICTION: f32 = 25.0;
/// Movement time step per tick.
const DT: f32 = 0.45;

/// What movement reads and writes for each player.
type MoveQuery<'a> = (
    &'a mut Position,
    &'a mut Velocity,
    &'a MoveDir,
    &'a ReloadState,
    &'a AttackState,
    &'a Weapon,
    &'a Gear,
    Has<Trapped>,
    Has<Underground>,
    Option<&'a mut Stagger>,
);

/// Moves players. Players stop for a moment when they swing, swim slowly,
/// and slide on snow. The held weapon and the hat change the top speed, and
/// trapped players can't move.
pub fn movement_system(mut query: Query<MoveQuery, With<PlayerEntity>>) {
    let dt = DT;
    let snow_end = f32::from(CONFIG.map.snow_end);
    let ocean_x = f32::from(CONFIG.map.ocean_start_x);
    let ocean_end = f32::from(CONFIG.map.ocean_end_x);

    query.par_iter_mut().for_each(
        |(mut pos, mut vel, move_dir, reload, attack, weapon, gear, trapped, underground, stagger)| {
            if trapped {
                *vel = Velocity(0.0, 0.0);
                return;
            }
            let hat = hat_effects(gear.hat);
            // Snow is slippery (unless you wear the winter cap); the caves
            // have no snow or water.
            let on_snow = pos.1 < snow_end && !hat.snow_immune && !underground;
            let in_water = pos.0 > ocean_x && pos.0 < ocean_end && !underground;
            let max_speed = PLAYER_MAX_SPEED * weapon_or_fists(weapon.0).speed_mult * hat.speed;

            let (mut vx, mut vy) = (vel.0, vel.1);

            // Swinging stops you for a moment; water slows you down.
            let mut decel = if reload.0 == 0 && attack.0 { 0.0 } else { 1.0 };
            if in_water && !hat.water_immune {
                decel *= WATER_DRAG;
            }

            // Knocked back by an animal: no steering until it wears off.
            let staggered = stagger.is_some_and(|mut s| {
                s.0 = s.0.saturating_sub(TICK_MS);
                s.0 > 0
            });
            let move_dir = if staggered { None } else { move_dir.0 };

            if let Some(dir) = move_dir {
                let target_vx = dir.cos() * max_speed;
                let target_vy = dir.sin() * max_speed;
                let (dx, dy) = (target_vx - vx, target_vy - vy);
                let dist = dx.hypot(dy);
                let accel = if on_snow { SNOW_ACCEL } else { PLAYER_ACCEL };
                let max_step = accel * dt;

                if dist > max_step {
                    vx = (dx / dist).mul_add(max_step, vx);
                    vy = (dy / dist).mul_add(max_step, vy);
                } else {
                    vx = target_vx;
                    vy = target_vy;
                }

                vx *= decel;
                vy *= decel;
            } else {
                let speed = vx.hypot(vy);
                let drop = if staggered {
                    // Slide with the knockback.
                    STAGGER_FRICTION * dt
                } else if on_snow {
                    SNOW_FRICTION * dt
                } else {
                    PLAYER_FRICTION * dt
                };
                if speed > drop {
                    vx = f32::mul_add(vx / speed, -drop, vx);
                    vy = f32::mul_add(vy / speed, -drop, vy);
                } else {
                    vx = 0.0;
                    vy = 0.0;
                }
            }

            vx = vx.clamp(-MAX_AXIS_SPEED, MAX_AXIS_SPEED);
            vy = vy.clamp(-MAX_AXIS_SPEED, MAX_AXIS_SPEED);

            vel.0 = vx;
            vel.1 = vy;
            pos.0 = f32::mul_add(vx, dt, pos.0);
            pos.1 = f32::mul_add(vy, dt, pos.1);
        },
    );
}
