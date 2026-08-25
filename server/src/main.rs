use std::{
    sync::{atomic::AtomicU32, Arc},
    time::Duration,
};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct CollisionSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct SpatialIndexSet;

use bevy_ecs::{prelude::*, schedule::ScheduleBuildSettings};
use dashmap::DashMap;
use snafu::OptionExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpListener, sync::mpsc as god};
use wtransport::*;

use crate::{
    config::{load_config, Config},
    errors::{GameError, InternalGameMessages, SystemPanicSnafu},
    net::{ip_lookup::normalize_ip, pow::Pow},
    structs::{
        bevy::{IDToConnection, InputMap, PlayerConnection, PlayerMap, World},
        components::{HealthUpdates, HitEvents, UpgradeEvents},
        visibility::PlayerKnowledge,
    },
    systems::{init_animals, init_map, FishSpatialIndex},
};

mod config;
mod errors;
mod net;
mod replication;
mod structs;
mod systems;

use mimalloc::MiMalloc;

// Redefine the global allocator as MiMalloc, no clue how it will affect
// performance.
// Trade off -> more balanced CPU usage but higher RAM.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

// Types.
pub type ConnectionMap = Arc<DashMap<u32, PlayerConnection>>;
pub type GameChannel = tokio::sync::mpsc::Sender<(u32, InternalGameMessages)>;

// Global config.
pub static CONFIG: once_cell::sync::Lazy<Config> =
    once_cell::sync::Lazy::new(|| load_config().expect("Failed to load config"));

#[tokio::main]
async fn main() -> Result<(), GameError> {
    // Log to a file.
    let log_file = std::fs::File::create("server.log").expect("failed to create server.log");
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    // Load config.
    let config = load_config().expect("failed to load config");

    let game_port = config.game.port;
    let idle_timeout_mins = config.game.idle_timeout_mins;

    // Create a bevy schedule for the systems.
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::MultiThreaded);
    schedule.set_build_settings(ScheduleBuildSettings::default());

    schedule.add_systems(
        (
            systems::build_spatial_index_system,
            systems::build_fish_spatial_index_system,
        )
            .in_set(SpatialIndexSet),
    );
    schedule.add_systems(
        (systems::movement_system, systems::animal_ai_system)
            .after(SpatialIndexSet)
            .before(CollisionSet),
    );
    schedule.add_systems((systems::collision_resolution_system, systems::attack_system).in_set(CollisionSet));
    schedule.add_systems(systems::health_system.after(CollisionSet));

    // Create instance of World.
    let mut world = World {
        bevy_world: bevy_ecs::world::World::new(),
        schedule,
    };

    // Create random number generator.
    let mut rng = nanorand::WyRand::new();

    // Initialize PoW.
    let pow = Pow::new(4);
    let pow_sweep = pow.clone();

    // Sweep PoW challenges periodically.
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            pow_sweep.sweep();
        }
    });

    // PoW HTTP endpoint.
    let pow_http = pow.clone();
    tokio::spawn(async move {
        // Bind an endpoint to port 3001 for PoW challenges.
        // TODO: Add better security related to this endpoint.
        let listener = TcpListener::bind("0.0.0.0:3001").await.unwrap();

        loop {
            // Accept a connection.
            let Ok((mut socket, _)) = listener.accept().await else {
                continue;
            };
            let pow = pow_http.clone();

            tokio::spawn(async move {
                // Initialize a buffer for reading the request.
                let mut buf = [0u8; 512];
                let n = match socket.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => return,
                };

                // Parse the request.
                let req = std::str::from_utf8(&buf[..n]).unwrap_or("");

                // Check if the request is a valid PoW challenge request.
                // If not, return a 404 Not Found response.
                if !req.starts_with("GET /challenge") {
                    let _ = socket.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
                    return;
                }

                // Generate a PoW challenge.
                let challenge = pow.generate_challenge();
                tracing::info!("generated challenge: {}, map size: {}", challenge, pow.challenges.len());

                // Return the challenge as a plain text response.
                let url = "http://localhost:5173";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: {}\r\n\r\n{}",
                    challenge.len(),
                    url,
                    challenge,
                );

                // Send the response to the client.
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });

    // Initialize one-time things.
    init_map(&mut world.bevy_world, &mut rng, &config);
    init_animals(&mut world.bevy_world, &mut rng);

    // Insert resources.
    world.bevy_world.insert_resource(PlayerMap::default());
    world.bevy_world.insert_resource(HitEvents::default());
    world.bevy_world.insert_resource(HealthUpdates::default());
    world.bevy_world.insert_resource(UpgradeEvents::default());
    world.bevy_world.insert_resource(FishSpatialIndex::default());
    world
        .bevy_world
        .insert_resource(crate::structs::spatial_index::GlobalSpatialIndex::default());
    world.bevy_world.insert_resource(config);
    world.bevy_world.insert_resource(PlayerKnowledge::default());

    // Configure rayon's thread pool.
    rayon::ThreadPoolBuilder::new()
        .stack_size(32 * 1024 * 1024)
        .build_global()
        .ok()
        .context(SystemPanicSnafu)?;

    // Create maps for player connections, input maps, and IP addresses.
    let player_connections: IDToConnection = Arc::new(DashMap::new());
    let input_map: InputMap = Arc::new(DashMap::new());
    let ips: Arc<DashMap<String, u8>> = Arc::new(DashMap::new());

    // Create an internal channel for game messages.
    let (input_tx, input_rx) = god::channel::<(u32, InternalGameMessages)>(1024);

    // Initialize WebTransport server.
    let identity = Identity::load_pemfiles("cert.pem", "key.pem")
        .await
        .ok()
        .context(SystemPanicSnafu)?;
    let server_config = ServerConfig::builder()
        .with_bind_default(game_port)
        .with_identity(identity)
        .max_idle_timeout(Some(Duration::from_mins(idle_timeout_mins)))
        .unwrap()
        .build();

    let server = Endpoint::server(server_config).ok().context(SystemPanicSnafu)?;

    tracing::info!("Server listening on port {}", game_port);

    // Stores the next user id.
    let next_id = AtomicU32::new(0);

    // Spawn a tokio task to handle incoming connection requests.
    tokio::spawn({
        // Clone these 'Arc's.
        let connections = player_connections.clone();
        let tx = input_tx.clone();
        let input_map = input_map.clone();
        let ips = ips.clone();

        async move {
            loop {
                let incoming_session = match server.accept().await.await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to accept session: {}", e);
                        continue;
                    }
                };

                // Get the client IP, normalize it.
                // Need to check if they are botting.
                let mut client_ip = incoming_session
                    .headers()
                    .get("x-forwarded-for")
                    .and_then(|v| v.split(',').next())
                    .map(|v| v.trim().to_string())
                    .unwrap_or_else(|| incoming_session.remote_address().ip().to_string());
                client_ip = normalize_ip(&client_ip);

                // if ips.contains_key(&client_ip) {
                //     // Player already connected from this IP, reject.
                //     continue;
                // }

                // Verify PoW
                let path = incoming_session.path();
                let nonce = extract_query_param(path, "pow").unwrap_or_default();
                let challenge_key = extract_query_param(path, "challenge").unwrap_or_default();

                if !pow.verify(&challenge_key, &nonce) {
                    incoming_session.forbidden().await;

                    continue;
                }

                let player_id = next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let connection_map = connections.clone();
                let game_tx = tx.clone();
                let input_map = input_map.clone();
                let ips = ips.clone();

                ips.insert(client_ip.clone(), 1);

                tokio::spawn(async move {
                    let connection = match incoming_session.accept().await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("Failed to accept connection: {}", e);
                            return;
                        }
                    };

                    net::handle_conn(
                        connection,
                        player_id,
                        client_ip,
                        &ips,
                        connection_map,
                        game_tx,
                        input_map.clone(),
                    )
                    .await;
                });
            }
        }
    });

    // Get a 'Handle' for tokio's runtime.
    let rt_handle = tokio::runtime::Handle::current();

    // Clone these 'Arc's.
    let game_conns = player_connections.clone();
    let game_inputs = input_map.clone();

    // Spawn the game loop on a dedicated system thread.
    std::thread::spawn(move || {
        World::run(world, input_rx, game_inputs, game_conns, rt_handle);
    });

    // Return a pending future to keep the runtime alive.
    let _: () = std::future::pending::<()>().await;
    Ok(())
}

fn extract_query_param(path: &str, key: &str) -> Option<String> {
    path.split('?')
        .nth(1)?
        .split('&')
        .find(|p| p.starts_with(&format!("{}=", key)))?
        .split('=')
        .nth(1)
        .map(|v| v.to_string())
}
