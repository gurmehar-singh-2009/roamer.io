//! Connections, player input and the game loop.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use bevy_ecs::prelude::*;
use bytes::Bytes;
use dashmap::DashMap;
use shared::{to_client::ChatData, PacketType};
use tokio::sync::mpsc;
use wtransport::{Connection, VarInt};

use crate::{
    errors::InternalGameMessages,
    net::SessionCrypto,
    replication,
    structs::{
        components::{AimDir, AttackState, MoveDir, PlayerEntity, TickCount},
        outbox::{Outbox, Target},
    },
    systems::{
        anticheat_system::{blacklist, CheatHistory, Kicks},
        arena_system, building_system, cave_system, player_system, progression_system, social_system, TICK_MS,
    },
};

/// A player connection.
#[derive(Clone)]
pub struct PlayerConnection {
    /// The WebTransport connection.
    pub connection: Connection,
    /// Session encryption.
    pub crypto:     SessionCrypto,
    /// The client's IP address.
    pub ip:         String,
    /// Total bytes sent.
    pub bytes_sent: Arc<AtomicU64>,
    /// Total bytes received.
    pub bytes_recv: Arc<AtomicU64>,
    /// Encrypted reliable messages, written in order on one stream by a
    /// background task.
    reliable_tx:    mpsc::UnboundedSender<Vec<u8>>,
}

impl PlayerConnection {
    /// Wraps a connection and starts its reliable-stream writer. Must be
    /// called inside the tokio runtime.
    pub fn new(connection: Connection, crypto: SessionCrypto, ip: String) -> Self {
        let (reliable_tx, mut reliable_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let writer_conn = connection.clone();
        tokio::spawn(async move {
            // One stream for everything keeps reliable messages in order.
            let Ok(opening) = writer_conn.open_uni().await else {
                return;
            };
            let Ok(mut stream) = opening.await else {
                return;
            };
            while let Some(packet) = reliable_rx.recv().await {
                let Ok(len) = u32::try_from(packet.len()) else {
                    continue;
                };
                if stream.write_all(&len.to_be_bytes()).await.is_err() || stream.write_all(&packet).await.is_err() {
                    break;
                }
            }
        });
        Self {
            connection,
            crypto,
            ip,
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_recv: Arc::new(AtomicU64::new(0)),
            reliable_tx,
        }
    }

    /// Sends a message as an encrypted datagram.
    ///
    /// # Errors
    /// Returns an error if encryption or sending fails.
    pub fn send_encrypted(&self, plaintext: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let packet = self
            .crypto
            .encrypt(plaintext)
            .map_err(|e| format!("encrypt failed: {e}"))?;
        self.bytes_sent.fetch_add(packet.len() as u64, Ordering::Relaxed);
        self.connection
            .send_datagram(Bytes::from(packet))
            .map_err(|e| format!("send_datagram failed: {e}"))?;
        Ok(())
    }

    /// Queues an encrypted message on the reliable stream.
    ///
    /// # Errors
    /// Returns an error if encryption fails or the writer has stopped.
    pub fn send_reliable(&self, plaintext: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let packet = self
            .crypto
            .encrypt(plaintext)
            .map_err(|e| format!("encrypt failed: {e}"))?;
        self.bytes_sent.fetch_add(packet.len() as u64, Ordering::Relaxed);
        self.reliable_tx.send(packet).map_err(|_| "reliable writer stopped")?;
        Ok(())
    }
}

/// Player id -> connection.
pub type IDToConnection = Arc<DashMap<u32, PlayerConnection>>;

/// The game world and the schedule that runs each tick.
pub struct World {
    /// The ECS world.
    pub bevy_world: bevy_ecs::world::World,
    /// Systems that run each tick.
    pub schedule:   bevy_ecs::schedule::Schedule,
}

/// Max queued messages handled per tick, so a flood of packets can't starve
/// the simulation.
const MAX_MESSAGES_PER_TICK: usize = 200;

impl World {
    /// Handles one message from a player.
    fn handle_message(&mut self, id: u32, msg: InternalGameMessages, input_map: &InputMap) {
        let bevy = &mut self.bevy_world;
        let entity = bevy.resource::<PlayerMap>().map.get(&id).copied();
        let tick = bevy.resource::<TickCount>().0;

        match msg {
            InternalGameMessages::Spawn { name, token } => {
                input_map.entry(id).or_insert_with(PlayerInput::new);
                player_system::spawn_player(bevy, id, &name, &token);
            }
            InternalGameMessages::Disconnect => {
                input_map.remove(&id);
                player_system::disconnect(bevy, id);
                bevy.resource_mut::<CheatHistory>().remove(id);
            }
            InternalGameMessages::ChatMessage(message) => {
                if message.starts_with('/') && dev_command(bevy, entity, &message) {
                    return;
                }
                bevy.resource_mut::<Outbox>()
                    .send(Target::All, PacketType::Chat, &ChatData { id, message });
            }
            InternalGameMessages::Clan(msg) => social_system::clan_action(bevy, id, &msg),
            InternalGameMessages::Trade(msg) => social_system::trade(bevy, id, &msg),
            InternalGameMessages::Arena(msg) => arena_system::arena_action(bevy, id, &msg),
            other => {
                // Everything else needs a living player.
                let Some(entity) = entity else {
                    return;
                };
                match other {
                    InternalGameMessages::Attack(attacking) => {
                        if let Some(mut a) = bevy.get_mut::<AttackState>(entity) {
                            a.0 = attacking;
                        }
                    }
                    InternalGameMessages::SetWeapon(weapon) => {
                        progression_system::switch_weapon(bevy, entity, weapon);
                    }
                    InternalGameMessages::UseItem(item) => {
                        building_system::use_item(bevy, entity, item, tick);
                    }
                    InternalGameMessages::ChooseUpgrade { is_item, id: pick } => {
                        progression_system::choose_upgrade(bevy, entity, is_item, pick);
                    }
                    InternalGameMessages::Shop(msg) => social_system::shop_action(bevy, entity, &msg),
                    InternalGameMessages::Chest(msg) => {
                        building_system::chest_action(bevy, entity, &msg);
                    }
                    InternalGameMessages::Cave => cave_system::toggle_layer(bevy, entity),
                    _ => {}
                }
            }
        }
    }

    /// Disconnects and blacklists flagged players.
    fn process_kicks(&mut self, player_connections: &IDToConnection) {
        let kicks = std::mem::take(&mut self.bevy_world.resource_mut::<Kicks>().0);
        for (id, reason) in kicks {
            tracing::warn!("kicking player {} ({})", id, reason);
            if let Some((_, conn)) = player_connections.remove(&id) {
                blacklist(&conn.ip);
                conn.connection.close(VarInt::from_u32(41), reason.as_bytes());
            }
            player_system::disconnect(&mut self.bevy_world, id);
        }
    }

    /// Runs the game loop forever.
    pub fn run(
        world: Self,
        mut input_rx: mpsc::Receiver<(u32, InternalGameMessages)>,
        input_map: &InputMap,
        player_connections: &IDToConnection,
    ) {
        let tick = Duration::from_millis(u64::from(TICK_MS));
        let mut w = world;

        loop {
            let start = std::time::Instant::now();
            w.bevy_world.resource_mut::<TickCount>().0 += 1;

            for _ in 0..MAX_MESSAGES_PER_TICK {
                match input_rx.try_recv() {
                    Ok((id, msg)) => w.handle_message(id, msg, input_map),
                    Err(_) => break,
                }
            }

            // Copy movement and aim input into the world.
            let entities: Vec<(Entity, u32)> = w
                .bevy_world
                .query::<(Entity, &PlayerEntity)>()
                .iter(&w.bevy_world)
                .map(|(e, p)| (e, p.0))
                .collect();
            for (entity, id) in entities {
                if let Some(input) = input_map.get(&id) {
                    if let Some(mut md) = w.bevy_world.get_mut::<MoveDir>(entity) {
                        md.0 = input.get_move();
                    }
                    if let Some(mut ad) = w.bevy_world.get_mut::<AimDir>(entity) {
                        ad.0 = input.get_aim();
                    }
                }
            }

            w.schedule.run(&mut w.bevy_world);

            let bevy = &mut w.bevy_world;
            player_system::process_deaths(bevy);
            replication::upgrades::flush_dirty(bevy);
            replication::hits::replicate_attacks(bevy);
            replication::replicate_view(bevy);
            w.process_kicks(player_connections);
            replication::flush(&mut w.bevy_world, player_connections);

            let elapsed = start.elapsed();
            if let Some(remaining) = tick.checked_sub(elapsed) {
                std::thread::sleep(remaining);
            } else {
                tracing::warn!("tick took {} ms (budget {} ms)", elapsed.as_millis(), TICK_MS);
            }
        }
    }
}

/// Developer chat commands, only when the server runs with `ROAMER_DEV=1`:
/// `/xp N`, `/res N` (all resources and ammo), `/hurt N`, `/wxp N` (held
/// weapon XP), `/tp X Y`, `/event N` (0 gold rush,
/// 1 boss hunt, 2 supply drop, 3 stampede). Returns true if it was one.
fn dev_command(world: &mut bevy_ecs::world::World, entity: Option<Entity>, message: &str) -> bool {
    use crate::structs::components::{Dirty, Resources};
    use crate::systems::social_system::{start_event, EventKind};

    if std::env::var("ROAMER_DEV").as_deref() != Ok("1") {
        return false;
    }
    let mut parts = message.split_whitespace();
    let cmd = parts.next().unwrap_or_default();
    let n: u32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    match (cmd, entity) {
        ("/xp", Some(e)) => progression_system::grant_xp(world, e, n),
        ("/res", Some(e)) => {
            if let Some(mut r) = world.get_mut::<Resources>(e) {
                *r = Resources {
                    wood:  r.wood + n,
                    stone: r.stone + n,
                    food:  r.food + n,
                    gold:  r.gold + n,
                    ammo:  r.ammo + n,
                };
            }
            world.resource_mut::<Dirty>().resources.insert(e);
        }
        ("/hurt", Some(e)) => {
            use crate::structs::components::{Damage, DamageEvents};
            world.resource_mut::<DamageEvents>().0.push(Damage {
                target: e,
                amount: f32::from(u16::try_from(n).unwrap_or(u16::MAX)),
                ..Damage::default()
            });
        }
        ("/wxp", Some(e)) => {
            let weapon = world.get::<crate::structs::components::Weapon>(e).map_or(0, |w| w.0);
            progression_system::grant_weapon_xp(world, e, weapon, n);
        }
        ("/tp", Some(e)) => {
            let y: u32 = message
                .split_whitespace()
                .nth(2)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            #[allow(clippy::cast_precision_loss)]
            world
                .entity_mut(e)
                .insert(crate::structs::components::Position(n as f32, y as f32));
        }
        ("/event", _) => start_event(
            world,
            match n {
                0 => EventKind::GoldRush,
                1 => EventKind::BossHunt,
                2 => EventKind::SupplyDrop,
                _ => EventKind::Stampede,
            },
        ),
        _ => return false,
    }
    true
}

/// Player id -> entity of living players.
#[derive(Debug, Resource, Default)]
pub struct PlayerMap {
    /// The mapping.
    pub map: HashMap<u32, Entity>,
}

/// Latest input from a player. Written by the connection task, read by the
/// game loop.
pub struct PlayerInput {
    /// Move direction as `f32` bits.
    pub move_dir: AtomicU32,
    /// Aim direction as `f32` bits.
    pub aim_dir:  AtomicU32,
    /// Whether the player is moving at all.
    pub has_move: AtomicBool,
}

impl PlayerInput {
    /// No input yet.
    pub const fn new() -> Self {
        Self {
            has_move: AtomicBool::new(false),
            move_dir: AtomicU32::new(0),
            aim_dir:  AtomicU32::new(0),
        }
    }

    /// Sets the move direction, `None` to stop.
    pub fn set_move(&self, dir: Option<f32>) {
        self.has_move.store(dir.is_some(), Ordering::Relaxed);
        self.move_dir.store(dir.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
    }

    /// Sets the aim direction.
    pub fn set_aim(&self, dir: f32) {
        self.aim_dir.store(dir.to_bits(), Ordering::Relaxed);
    }

    /// The move direction, `None` when not moving.
    pub fn get_move(&self) -> Option<f32> {
        self.has_move
            .load(Ordering::Relaxed)
            .then(|| f32::from_bits(self.move_dir.load(Ordering::Relaxed)))
    }

    /// The aim direction.
    pub fn get_aim(&self) -> f32 {
        f32::from_bits(self.aim_dir.load(Ordering::Relaxed))
    }
}

/// Player id -> latest input.
pub type InputMap = Arc<DashMap<u32, PlayerInput>>;
