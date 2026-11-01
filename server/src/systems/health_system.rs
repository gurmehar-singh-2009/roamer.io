//! Status effects: poison, hat regeneration and drain, hat gold.

use bevy_ecs::{component::Component, entity::Entity, world::World};
use nanorand::Rng;

use crate::{
    catalog::hats::hat_effects,
    replication::health::send_health,
    structs::{
        components::{
            Damage, DamageEvents, DamageSource, Dirty, Gear, Health, PlayerEntity, Poisoned, Position, Resources,
        },
        outbox::Outbox,
    },
    systems::{rng, TICK_MS},
};

/// Ticks in one second (rounded).
const TICKS_PER_SECOND: u32 = 1000 / TICK_MS;

/// When a player's next once-a-second hat effects (bull drain, medic regen,
/// windmill gold) apply.
///
/// The bull helmet drain is a known timing target for scripts ("bull
/// ticks"), so each player's schedule drifts by a random 1-2 ticks every
/// time.
#[derive(Component, Clone, Copy, Debug)]
pub struct HatTick {
    /// Ticks until the next application.
    pub ticks_left: u32,
}

/// Ticks until the next hat tick: about a second, plus 1-2 random ticks.
fn next_hat_tick() -> u32 {
    TICKS_PER_SECOND + rng().generate_range(1..=2)
}

/// Applies poison and once-a-second hat effects.
pub fn status_system(world: &mut World) {
    let mut damage = Vec::new();

    // Poison ticks once a second.
    let mut cured = Vec::new();
    for (entity, mut poison, pos) in world.query::<(Entity, &mut Poisoned, &Position)>().iter_mut(world) {
        let before = poison.remaining_ms;
        poison.remaining_ms = poison.remaining_ms.saturating_sub(TICK_MS);
        if before / 1000 != poison.remaining_ms / 1000 {
            damage.push(Damage {
                target: entity,
                amount: poison.dps,
                source: DamageSource {
                    player: poison.source,
                    pos: (pos.0, pos.1),
                    ..DamageSource::default()
                },
                ..Damage::default()
            });
        }
        if poison.remaining_ms == 0 {
            cured.push(entity);
        }
    }
    for e in cured {
        world.entity_mut(e).remove::<Poisoned>();
    }

    // Hat effects.
    let mut new_timers = Vec::new();
    let mut heals = Vec::new();
    let mut gold = Vec::new();
    for (entity, player, gear, pos, timer) in world
        .query::<(Entity, &PlayerEntity, &Gear, &Position, Option<&mut HatTick>)>()
        .iter_mut(world)
    {
        let effects = hat_effects(gear.hat);
        if effects.drain == 0.0 && effects.regen == 0.0 && effects.gold_per_sec == 0.0 {
            continue;
        }
        let Some(mut timer) = timer else {
            new_timers.push(entity);
            continue;
        };
        timer.ticks_left = timer.ticks_left.saturating_sub(1);
        if timer.ticks_left > 0 {
            continue;
        }
        timer.ticks_left = next_hat_tick();

        if effects.drain > 0.0 {
            damage.push(Damage {
                target: entity,
                amount: effects.drain,
                source: DamageSource {
                    pos: (pos.0, pos.1),
                    ..DamageSource::default()
                },
                ..Damage::default()
            });
        }
        if effects.regen > 0.0 {
            heals.push((entity, player.0, *pos, effects.regen));
        }
        if effects.gold_per_sec > 0.0 {
            gold.push(entity);
        }
    }
    for e in new_timers {
        world.entity_mut(e).insert(HatTick {
            ticks_left: next_hat_tick(),
        });
    }

    for (entity, id, pos, amount) in heals {
        heal(world, entity, id, (pos.0, pos.1), amount);
    }
    for e in gold {
        if let Some(mut r) = world.get_mut::<Resources>(e) {
            r.gold += 1;
        }
        world.resource_mut::<Dirty>().resources.insert(e);
    }

    world.resource_mut::<DamageEvents>().0.extend(damage);
}

/// Heals a player and tells nearby clients.
pub fn heal(world: &mut World, entity: Entity, id: u32, pos: (f32, f32), amount: f32) {
    let Some(mut health) = world.get_mut::<Health>(entity) else {
        return;
    };
    if health.0 >= health.1 || health.0 <= 0.0 {
        return;
    }
    health.0 = (health.0 + amount).min(health.1);
    let (hp, max) = (health.0, health.1);
    send_health(&mut world.resource_mut::<Outbox>(), id, pos, hp, max);
}
