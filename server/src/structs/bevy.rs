use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64},
        Arc,
    },
    time::Duration,
};

use bevy_ecs::prelude::*;
use bytes::Bytes;
use dashmap::DashMap;
use shared::{
    objects::{GameObjects, StaticGameObjects},
    to_client::{
        AddAnimalData, AnimalTO, PlayerTO,
        SetWeaponsData,
    },
    to_server::ClientMessages,
};
use wtransport::Connection;

use crate::{
    broadcast,
    errors::InternalGameMessages,
    net::{serialization::encode, SessionCrypto},
    replication::{
        health::replicate_health, hits::replicate_hits, objects::replicate_object, players::replicate_players,
        upgrades::replicate_upgrades,
    },
    structs::{
        components::{
            AimDir, AnimalType, AttackState, Experience, Health, HitEvents, Level, MoveDir, Name, ObjectEntity,
            PlayerBundle, PlayerEntity, Position, ReloadState, Resources, UpgradeEvents, Velocity,
        },
        weapons::{get_weapon_attack_speed, weapon_upgrade_hierachy, Weapon},
    },
    systems::NonReactiveCollider,
    CONFIG,
};
use crate::{
    structs::visibility::{AnimalSnapshot, ObjSnapshot, PlayerKnowledge},
    systems::Collider,
};

#[derive(Clone)]
/// Struct that represents a player connection.
pub struct PlayerConnection {
    pub connection: Connection,
    pub crypto:     SessionCrypto,
    pub bytes_sent: Arc<AtomicU64>,
    pub bytes_recv: Arc<AtomicU64>,
}

impl PlayerConnection {
    /// Initialize a new player connection. Takes a wtransport Connection and
    /// the SessionCrypto.
    pub fn new(connection: Connection, crypto: SessionCrypto) -> Self {
        Self {
            connection,
            crypto,
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_recv: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Sends the encoded data (using borsh) through the Socket using datagrams.
    pub async fn send_encrypted(&self, plaintext: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Encrypt the packet (SessionCrypto handles nonce incrementing atomically).
        let packet = self
            .crypto
            .encrypt(plaintext)
            .map_err(|e| format!("encrypt failed: {e}"))?;

        // Update the bytes sent counter.
        self.bytes_sent
            .fetch_add(packet.len() as u64, std::sync::atomic::Ordering::Relaxed);

        self.connection
            .send_datagram(Bytes::from(packet))
            .map_err(|e| format!("send_datagram failed: {e}"))?;

        Ok(())
    }

    /// Sends the encoded data (using borsh) through the Socket using the TCP
    /// protocol.
    pub async fn send_reliable(&self, plaintext: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let packet = self
            .crypto
            .encrypt(plaintext)
            .map_err(|e| format!("encrypt failed: {e}"))?;

        self.bytes_sent
            .fetch_add(packet.len() as u64, std::sync::atomic::Ordering::Relaxed);

        let mut stream = self.connection.open_uni().await?.await?;

        let len_bytes = (packet.len() as u32).to_be_bytes();
        stream.write_all(&len_bytes).await?;
        stream.write_all(&packet).await?;
        stream.finish().await?;

        Ok(())
    }
}

/// World struct.
/// Contains the bevy ECS world, the config, and the schedule.
pub struct World {
    pub bevy_world: bevy_ecs::world::World,
    pub schedule:   bevy_ecs::schedule::Schedule,
}

pub type IDToConnection = Arc<DashMap<u32, PlayerConnection>>;

impl World {
    /// Broadcasts the data to all the active connections.
    pub async fn broadcast(plaintext: &[u8], connections: &IDToConnection) {
        // Get the values (the 'PlayerConnection') from the IDToConnection DashMap.
        let conns: Vec<PlayerConnection> = connections.iter().map(|e| e.value().clone()).collect();

        // Loop through each connection and attempt to send.
        for conn in conns {
            let data = Bytes::copy_from_slice(plaintext);
            tokio::spawn(async move {
                // Currently discards any errors. Might change later.
                let _ = conn.send_encrypted(&data).await;
            });
        }
    }

    /// Broadcasts the data to all active connections, except those exempt.
    pub async fn broadcast_with_exceptions(exceptions: &[u32], plaintext: &[u8], connections: &IDToConnection) {
        let conns: Vec<PlayerConnection> = connections
            .iter()
            .filter(|e| !exceptions.contains(e.key()))
            .map(|e| e.value().clone())
            .collect();

        for conn in conns {
            let data = Bytes::copy_from_slice(plaintext);
            tokio::spawn(async move {
                let _ = conn.send_encrypted(&data).await;
            });
        }
    }

    /// Uses TCP to "guarantee" the message goes through.
    /// Broadcasts the data to all active connections.
    #[allow(dead_code)]
    pub async fn broadcast_reliable(plaintext: &[u8], connections: &IDToConnection) {
        let conns: Vec<PlayerConnection> = connections.iter().map(|e| e.value().clone()).collect();

        for conn in conns {
            let data = plaintext.to_vec();
            tokio::spawn(async move {
                let _ = conn.send_reliable(&data).await;
            });
        }
    }

    /// Sends the data through TCP to the connection mapped with the id.
    pub async fn send_reliable_to(id: u32, plaintext: &[u8], connections: &IDToConnection) {
        if let Some(conn) = connections.get(&id).map(|e| e.value().clone()) {
            let data = plaintext.to_vec();
            tokio::spawn(async move {
                if let Err(e) = conn.send_reliable(&data).await {
                    tracing::error!("reliable send failed for player {}: {}", id, e);
                }
            });
        }
    }

    /// Sends the data to the connection mapped with the id.
    pub async fn send_to(id: u32, plaintext: &[u8], connections: &IDToConnection) {
        if let Some(conn) = connections.get(&id).map(|e| e.value().clone()) {
            let data = Bytes::copy_from_slice(plaintext);
            tokio::spawn(async move {
                let _ = conn.send_encrypted(&data).await;
            });
        }
    }

    /// Process internal events.
    fn handle_internal_game_msgs(
        &mut self,
        msg: InternalGameMessages,
        input_map: &InputMap,
        id: u32,
        player_connections: &IDToConnection,
        rt_handle: &tokio::runtime::Handle,
    ) {
        // Store a mutable reference to the bevy world.
        let bevy = &mut self.bevy_world;

        // Process the message.
        match msg {
            InternalGameMessages::AddPlayer(p) => {
                // Guard against duplicate spawns (e.g. a client re-sending Spawn rapidly).
                // Without this, each duplicate would overwrite PlayerMap but leave the
                // previous entity orphaned in the ECS world.
                if bevy.resource::<PlayerMap>().map.contains_key(&id) {
                    return;
                }

                // Insert a new "PlayerInput".
                input_map.insert(id, PlayerInput::new());

                bevy.resource_mut::<PlayerKnowledge>().add_player(id);

                // Tell the new player about all existing players.
                {
                    let entities: Vec<(u32, Entity)> = {
                        let player_map = bevy.resource::<PlayerMap>();
                        player_map.map.iter().map(|e| (*e.0, *e.1)).collect()
                    };

                    for (existing_id, entity) in entities {
                        if let (Some(name), Some(pos), Some(aim), Some(health), Some(weapon_index)) = (
                            bevy.get::<Name>(entity),
                            bevy.get::<Position>(entity),
                            bevy.get::<AimDir>(entity),
                            bevy.get::<Health>(entity),
                            bevy.get::<Weapon>(entity),
                        ) {
                            let msg = crate::net::serialization::encode(
                                1,
                                ClientMessages::AddPlayer(shared::to_client::AddPlayerData {
                                    is_mine: false,
                                    data:    PlayerTO {
                                        id:           existing_id,
                                        name:         name.0.clone(),
                                        x:            pos.0,
                                        y:            pos.1,
                                        aim:          aim.0,
                                        health:       health.0,
                                        weapon_index: Some(*weapon_index as u8),
                                    },
                                }),
                            )
                            .unwrap();

                            broadcast!(to, rt_handle, player_connections, id, msg);
                        }
                    }
                }

                // Spawn the new player entity.
                use crate::systems::{Collider, ReactiveCollider};
                let entity = bevy
                    .spawn(PlayerBundle(
                        PlayerEntity,
                        Name(p.name.clone()),
                        Position(p.x, p.y),
                        Velocity(p.vx, p.vy),
                        MoveDir(None),
                        AimDir(0.0),
                        crate::structs::weapons::Weapon::Fists,
                        ReloadState(0, get_weapon_attack_speed(&crate::structs::weapons::Weapon::Fists)),
                        Health(100., 100.),
                        AttackState(false),
                        Resources(0, 0, 0, 0, 0),
                        Experience(0, 30),
                        Level(0),
                        Collider::circle(35.),
                        ReactiveCollider,
                    ))
                    .id();

                // Insert it into our player map.
                bevy.resource_mut::<PlayerMap>().map.insert(id, entity);

                // Tell the player about themself.
                let spawn_self = crate::net::serialization::encode(
                    1,
                    ClientMessages::AddPlayer(shared::to_client::AddPlayerData {
                        is_mine: true,
                        data:    PlayerTO {
                            id,
                            name: p.name.clone(),
                            x: p.x,
                            y: p.y,
                            health: p.health,
                            aim: 0.0,
                            weapon_index: Some(0),
                        },
                    }),
                )
                .unwrap();

                // Tell the player about their weapons.
                // TODO: Redundent - REMOVE LATER!!
                let set_weapons = crate::net::serialization::encode(9, SetWeaponsData { weapons: vec![0] }).unwrap();

                broadcast!(reliable_to, rt_handle, player_connections, id, spawn_self);
                broadcast!(reliable_to, rt_handle, player_connections, id, set_weapons);

                // Tell all existing players about the new player.
                let spawn_other = crate::net::serialization::encode(
                    1,
                    ClientMessages::AddPlayer(shared::to_client::AddPlayerData {
                        is_mine: false,
                        data:    PlayerTO {
                            id,
                            name: p.name.clone(),
                            x: p.x,
                            y: p.y,
                            aim: 0.0,
                            health: p.health,
                            weapon_index: Some(0),
                        },
                    }),
                )
                .unwrap();

                broadcast!(except, rt_handle, player_connections, [id], spawn_other);
            }

            InternalGameMessages::Disconnect => {
                // Remove their entry.
                input_map.remove(&id);

                // Despawn their bevy linked entity.
                // TODO: Send a despawn packet to all players, telling them to remove the
                // player.
                if let Some(e) = bevy.resource_mut::<PlayerMap>().map.remove(&id) {
                    if let Some(p) = bevy.get::<Name>(e) {
                        tracing::info!("player {} ({}) gone", p.0, id);
                    }
                    bevy.despawn(e);
                }

                bevy.resource_mut::<PlayerKnowledge>().remove_player(id);
            }

            InternalGameMessages::PlayerHit(_) => {
                if let Some(&e) = bevy.resource::<PlayerMap>().map.get(&id) {
                    if let Some(mut hit) = bevy.get_mut::<AttackState>(e) {
                        hit.0 = !hit.0;
                    }
                }
            }

            InternalGameMessages::ChatMessage(data) => {
                let msg = encode(13, data).unwrap();

                // Unstable datagrams.
                broadcast!(rt_handle, player_connections, msg)
            }

            InternalGameMessages::SetWeapon(weapon) => {
                let entity = bevy.resource::<PlayerMap>().map.get(&id).copied();

                if let Some(e) = entity {
                    let level_val = bevy.get::<Level>(e).map(|l| l.0);

                    if let Some(level_val) = level_val {
                        let upgr = weapon_upgrade_hierachy(level_val);

                        if let Some(&new_weapon) = (weapon.weapon as usize).checked_sub(1).and_then(|i| upgr.get(i)) {
                            if let Some(mut weapon_state) = bevy.get_mut::<Weapon>(e) {
                                *weapon_state = new_weapon;
                            }
                            if let Some(mut reload) = bevy.get_mut::<ReloadState>(e) {
                                *reload = ReloadState(0, get_weapon_attack_speed(&new_weapon));
                            }
                            let set_weapons = crate::net::serialization::encode(
                                9,
                                SetWeaponsData {
                                    weapons: vec![new_weapon as u8],
                                },
                            )
                            .unwrap();
                            broadcast!(reliable_to, rt_handle, player_connections, id, set_weapons);
                        } else {
                            println!("HERE REMOVE PLAYER!!");
                        }
                    }
                }
            }

            _ => {}
        }
    }

    /// Broadcast the game state.
    fn broadcast_state(&mut self, rt_handle: &tokio::runtime::Handle, player_connections: &IDToConnection) {
        let bevy = &mut self.bevy_world;

        let player_positions: Vec<(u32, f32, f32)> = {
            let player_map = bevy.resource::<PlayerMap>();
            player_map
                .map
                .iter()
                .filter_map(|(id, &entity)| bevy.get::<Position>(entity).map(|pos| (*id, pos.0, pos.1)))
                .collect()
        };

        let upgrade_events = std::mem::take(&mut bevy.resource_mut::<UpgradeEvents>().0);
        if !upgrade_events.is_empty() {
            replicate_upgrades(upgrade_events, bevy, rt_handle, player_connections);
        }

        let hits = std::mem::take(&mut bevy.resource_mut::<HitEvents>().0);
        if !hits.is_empty() {
            replicate_hits(&hits, bevy, &player_positions, rt_handle, player_connections);
        }

        // Broadcast accumulated health updates produced by systems this tick.
        let updates = std::mem::take(&mut bevy.resource_mut::<crate::structs::components::HealthUpdates>().0);
        if !updates.is_empty() {
            replicate_health(bevy, updates, &player_positions, rt_handle, player_connections);
        }

        let player_map = bevy.resource::<PlayerMap>();
        replicate_players(bevy, player_map, rt_handle, player_connections);

        // Collect a lightweight snapshot of every animal.
        // Fish are ECS entities (AnimalType::Fish) so they appear here already;
        // the old FishSpatialIndex snapshot loop duplicated them and is removed.
        let all_animals: Vec<AnimalSnapshot> = {
            let mut q = bevy.query::<(Entity, &Position, &AnimalType)>();
            q.iter(bevy)
                .map(|(entity, pos, atype)| AnimalSnapshot {
                    id:          entity.index(),
                    x:           pos.0,
                    y:           pos.1,
                    animal_type: *atype as u8,
                })
                .collect()
        };

        // Collect a lightweight snapshot of every static object.
        let all_objects: Vec<ObjSnapshot> = {
            let mut q = bevy.query::<(
                Entity,
                &ObjectEntity,
                &GameObjects,
                &Position,
                &AimDir,
                &Collider,
                &NonReactiveCollider,
            )>();
            q.iter(bevy)
                .map(|(entity, _, game_obj, pos, aim, col, _)| {
                    let type_obj = match game_obj {
                        GameObjects::StaticGameObjects(StaticGameObjects::Tree) => 0,
                        GameObjects::StaticGameObjects(StaticGameObjects::Stone) => 1,
                        GameObjects::StaticGameObjects(StaticGameObjects::Bush) => 2,
                        _ => 67,
                    };
                    ObjSnapshot {
                        id: entity.index(),
                        x: pos.0,
                        y: pos.1,
                        dir: aim.0,
                        scale: col.rad,
                        type_obj,
                    }
                })
                .collect()
        };

        for (player_id, px, py) in &player_positions {
            let nearby_animals: Vec<AnimalTO> = all_animals
                .iter()
                .filter_map(|a| {
                    let dx = a.x - px;
                    let dy = a.y - py;
                    if dx * dx + dy * dy <= CONFIG.players.view_radius_sq {
                        Some(AnimalTO {
                            id:          a.id,
                            x:           a.x,
                            y:           a.y,
                            animal_type: a.animal_type,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            if !nearby_animals.is_empty() {
                let msg = crate::net::serialization::encode(
                    4,
                    AddAnimalData {
                        animals: nearby_animals,
                    },
                )
                .unwrap();

                broadcast!(reliable_to, rt_handle, player_connections, *player_id, msg);
            }
            // if !nearby_animals.is_empty() {
            //     let msg = crate::net::serialization::encode(
            //         4,
            //         AddAnimalData {
            //             animals: nearby_animals,
            //         },
            //     )
            //     .unwrap();

            //     broadcast!(to, rt_handle, player_connections, *player_id, msg);
            // }
            //

            replicate_object(&all_objects, player_id, (px, py), bevy, rt_handle, player_connections);
        }
    }

    pub fn run(
        world: World,
        mut input_rx: tokio::sync::mpsc::Receiver<(u32, InternalGameMessages)>,
        input_map: InputMap,
        player_connections: IDToConnection,
        rt_handle: tokio::runtime::Handle,
    ) {
        let tick = Duration::from_millis(67);
        let mut w = world;

        loop {
            let start = std::time::Instant::now();

            {
                // Cap how many queued messages are processed per tick so a flood of
                // incoming packets (e.g. a client spamming HitEvent/ChatMessage) cannot
                // starve physics, AI, and state broadcast within the same tick.
                let mut msgs_this_tick = 0;
                while msgs_this_tick < 100 {
                    match input_rx.try_recv() {
                        Ok((id, msg)) => {
                            w.handle_internal_game_msgs(msg, &input_map, id, &player_connections, &rt_handle);
                            msgs_this_tick += 1;
                        }
                        Err(_) => break,
                    }
                }

                {
                    // let bevy = &mut w.bevy_world;
                    let World {
                        ref mut bevy_world,
                        ref mut schedule,
                        ..
                    } = w;

                    for entry in input_map.iter() {
                        let id = *entry.key();
                        if let Some(&entity) = bevy_world.resource::<PlayerMap>().map.get(&id) {
                            if let Some(mut md) = bevy_world.get_mut::<MoveDir>(entity) {
                                md.0 = entry.get_move();
                            }
                            if let Some(mut ad) = bevy_world.get_mut::<AimDir>(entity) {
                                ad.0 = entry.get_aim();
                            }
                        }
                    }

                    schedule.run(bevy_world);

                    w.broadcast_state(&rt_handle, &player_connections);
                }
            }

            if let Some(remaining) = tick.checked_sub(start.elapsed()) {
                std::thread::sleep(remaining);
            }
        }
    }
}

#[derive(Debug, Resource, Default)]
pub struct PlayerMap {
    pub map: HashMap<u32, Entity>,
}

pub struct PlayerInput {
    pub move_dir: AtomicU32,
    pub aim_dir:  AtomicU32,

    pub has_move: AtomicBool,
}

use std::sync::atomic::Ordering;
impl PlayerInput {
    pub fn new() -> Self {
        Self {
            has_move: AtomicBool::new(false),
            move_dir: AtomicU32::new(0),
            aim_dir:  AtomicU32::new(0),
        }
    }
    pub fn set_move(&self, dir: Option<f32>) {
        self.has_move.store(dir.is_some(), Ordering::Relaxed);
        self.move_dir.store(dir.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
    }
    pub fn set_aim(&self, dir: f32) {
        self.aim_dir.store(dir.to_bits(), Ordering::Relaxed);
    }
    pub fn get_move(&self) -> Option<f32> {
        self.has_move
            .load(Ordering::Relaxed)
            .then(|| f32::from_bits(self.move_dir.load(Ordering::Relaxed)))
    }
    pub fn get_aim(&self) -> f32 {
        f32::from_bits(self.aim_dir.load(Ordering::Relaxed))
    }
}

pub type InputMap = Arc<DashMap<u32, PlayerInput>>;
