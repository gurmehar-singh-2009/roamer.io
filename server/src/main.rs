//! Roamer.io game server.

use std::{
    sync::{atomic::AtomicU32, Arc},
    time::Duration,
};

use bevy_ecs::{prelude::*, schedule::ScheduleBuildSettings};
use dashmap::DashMap;
use snafu::OptionExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc as god,
};
use wtransport::{endpoint::endpoint_side::Server, Endpoint, Identity, ServerConfig};

use crate::{
    config::{load_config, Config},
    errors::{GameError, InternalGameMessages, SystemPanicSnafu},
    net::{ip_lookup::normalize_ip, pow::Pow},
    structs::{
        bevy::{IDToConnection, InputMap, PlayerConnection, PlayerMap, World},
        components::TickCount,
        visibility::PlayerKnowledge,
    },
    systems::{map_system::init_map, FishSpatialIndex},
};

/// Static game data (weapons, items, hats, animals).
mod catalog;
/// Config file loading.
mod config;
/// Error types and internal game messages.
mod errors;
/// Networking: handshake, encryption, packet handling.
mod net;
/// Sending game state to clients.
mod replication;
/// Components, resources and the game world.
mod structs;
/// Bevy systems that run each tick.
mod systems;

use mimalloc::MiMalloc;

/// Systems that resolve collisions and attacks.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct CollisionSet;

/// Systems that rebuild the spatial indices at the start of a tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct SpatialIndexSet;

// Redefine the global allocator as MiMalloc, no clue how it will affect
// performance.
// Trade off -> more balanced CPU usage but higher RAM.
/// Global allocator.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Player id -> connection.
pub type ConnectionMap = Arc<DashMap<u32, PlayerConnection>>;
/// Channel from connection tasks to the game loop.
pub type GameChannel = tokio::sync::mpsc::Sender<(u32, InternalGameMessages)>;

/// Global config.
pub static CONFIG: std::sync::LazyLock<Config> =
    std::sync::LazyLock::new(|| load_config().expect("Failed to load config"));

#[tokio::main]
async fn main() -> Result<(), GameError> {
    // Log to a file.
    let log_path = std::env::var("LOG_FILE").unwrap_or_else(|_| "server.log".into());
    let log_file = std::fs::File::create(&log_path).expect("failed to create the log file");
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    // HTTPS requests (IP lookups) use rustls with the ring backend.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Load config.
    let config = load_config().expect("failed to load config");

    // Environment overrides let a second copy run next to the usual one.
    let game_port = env_port("GAME_PORT").unwrap_or(config.game.port);
    let idle_timeout_secs = config.game.idle_timeout_secs;

    // Create instance of World.
    let mut world = World {
        bevy_world: bevy_ecs::world::World::new(),
        schedule:   build_schedule(),
    };

    // Initialize PoW.
    let pow = Pow::new(4);
    spawn_pow_server(&pow);

    // Initialize one-time things.
    let mut rng = nanorand::WyRand::new();
    let map_config = load_config().expect("failed to load config");
    insert_resources(&mut world.bevy_world, config);
    init_map(&mut world.bevy_world, &mut rng, &map_config);
    systems::cave_system::generate_caves(&mut world.bevy_world, &mut rng);

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
    let cert = std::env::var("CERT_FILE").unwrap_or_else(|_| "cert.pem".into());
    let key = std::env::var("KEY_FILE").unwrap_or_else(|_| "key.pem".into());
    let identity = Identity::load_pemfiles(&cert, &key)
        .await
        .ok()
        .context(SystemPanicSnafu)?;
    // Listen on every interface, IPv6 and IPv4.
    let bind: std::net::SocketAddr = format!("[::]:{game_port}").parse().ok().context(SystemPanicSnafu)?;
    let server_config = ServerConfig::builder()
        .with_bind_address(bind)
        .with_identity(identity)
        .keep_alive_interval(Some(Duration::from_secs(5)))
        .max_idle_timeout(Some(Duration::from_secs(idle_timeout_secs)))
        .ok()
        .context(SystemPanicSnafu)?
        .build();

    let server = Endpoint::server(server_config).ok().context(SystemPanicSnafu)?;

    tracing::info!("Server listening on port {}", game_port);

    spawn_accept_loop(
        server,
        pow,
        player_connections.clone(),
        input_tx,
        input_map.clone(),
        ips,
    );

    // Spawn the game loop on a dedicated system thread.
    std::thread::spawn(move || {
        World::run(world, input_rx, &input_map, &player_connections);
    });

    // Return a pending future to keep the runtime alive.
    std::future::pending::<()>().await;
    Ok(())
}

/// Systems that move things.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct MoveSet;

/// Builds the bevy schedule that runs every tick.
fn build_schedule() -> Schedule {
    use systems::{
        attack_system::attack_system,
        building_system::{building_tick_system, release_trapped, spike_system},
        damage_system::damage_system,
        health_system::status_system,
        map_system::upkeep_system,
        social_system::social_tick_system,
        weapon_system::projectile_system,
    };

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
        (systems::movement_system, systems::fish_system)
            .in_set(MoveSet)
            .after(SpatialIndexSet),
    );
    // Spikes need the velocity from movement, before collisions zero it.
    schedule.add_systems(
        (systems::land_animal_system, building_tick_system, spike_system)
            .chain()
            .after(MoveSet)
            .before(CollisionSet),
    );
    // Underground players skip normal collisions and slide along cave walls.
    schedule.add_systems(
        (
            systems::collision_resolution_system,
            systems::cave_system::cave_collision_system,
        )
            .chain()
            .in_set(CollisionSet),
    );
    schedule.add_systems(
        (
            release_trapped,
            attack_system,
            projectile_system,
            status_system,
            damage_system,
            upkeep_system,
            social_tick_system,
        )
            .chain()
            .after(CollisionSet),
    );

    schedule
}

/// Inserts the global ECS resources.
fn insert_resources(world: &mut bevy_ecs::world::World, config: Config) {
    use crate::{
        structs::{
            components::{AttackAnims, DamageEvents, Deaths, Dirty},
            outbox::Outbox,
        },
        systems::{
            accounts::Accounts,
            anticheat_system::{CheatHistory, Kicks},
            arena_system::ArenaRooms,
            building_system::{GoldRemainder, SpikeCooldowns},
            player_system::Sessions,
            social_system::{Clans, WorldEvents},
        },
    };

    world.insert_resource(PlayerMap::default());
    world.insert_resource(TickCount::default());
    world.insert_resource(Outbox::default());
    world.insert_resource(DamageEvents::default());
    world.insert_resource(Deaths::default());
    world.insert_resource(AttackAnims::default());
    world.insert_resource(Dirty::default());
    world.insert_resource(systems::cave_system::Caves::default());
    world.insert_resource(FishSpatialIndex::default());
    world.insert_resource(crate::structs::spatial_index::GlobalSpatialIndex::default());
    world.insert_resource(config);
    world.insert_resource(PlayerKnowledge::default());
    world.insert_resource(Sessions::default());
    world.insert_resource(Accounts::load(
        std::env::var("ACCOUNTS_FILE").unwrap_or_else(|_| "accounts.json".into()),
    ));
    world.insert_resource(Clans::default());
    world.insert_resource(WorldEvents::default());
    world.insert_resource(ArenaRooms::default());
    world.insert_resource(Kicks::default());
    world.insert_resource(CheatHistory::default());
    world.insert_resource(SpikeCooldowns::default());
    world.insert_resource(GoldRemainder::default());
}

/// Starts the proof-of-work challenge HTTP endpoint and the task that sweeps
/// old challenges.
fn spawn_pow_server(pow: &Arc<Pow>) {
    // Sweep PoW challenges periodically.
    let pow_sweep = pow.clone();
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
        // Bind an endpoint for PoW challenges (port 3001 unless POW_PORT is
        // set), on every interface. TODO: Add better security related to
        // this endpoint.
        let port = env_port("POW_PORT").unwrap_or(3001);
        let addr = format!("[::]:{port}");
        let listener = TcpListener::bind(addr).await.expect("failed to bind PoW endpoint");

        loop {
            // Accept a connection.
            let Ok((socket, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(handle_pow_request(socket, pow_http.clone()));
        }
    });
}

/// Answers one HTTP request on the proof-of-work endpoint.
async fn handle_pow_request(mut socket: TcpStream, pow: Arc<Pow>) {
    // Initialize a buffer for reading the request.
    let mut buf = [0u8; 512];
    let Ok(n) = socket.read(&mut buf).await else {
        return;
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

    // Return the challenge as a plain text response, to the page's origin if
    // it's allowed. CLIENT_ORIGIN is a comma-separated list (or "*").
    let allowed = std::env::var("CLIENT_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into());
    let url = allowed_origin(req, &allowed);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: \
         {url}\r\n\r\n{challenge}",
        challenge.len(),
    );

    // Send the response to the client.
    let _ = socket.write_all(response.as_bytes()).await;
}

/// The `Access-Control-Allow-Origin` value for a request: its own `Origin`
/// header when that's in the comma-separated `allowed` list (or the list is
/// "*"), otherwise the first allowed origin.
fn allowed_origin(req: &str, allowed: &str) -> String {
    let origin = req
        .lines()
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("origin")
                .then(|| value.trim().to_string())
        })
        .unwrap_or_default();
    let list: Vec<&str> = allowed.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if !origin.is_empty() && (list.contains(&"*") || list.contains(&origin.as_str())) {
        return origin;
    }
    list.first().map_or_else(|| "*".to_string(), |s| (*s).to_string())
}

/// Spawns the task that accepts incoming WebTransport sessions.
fn spawn_accept_loop(
    server: Endpoint<Server>,
    pow: Arc<Pow>,
    connections: IDToConnection,
    tx: GameChannel,
    input_map: InputMap,
    ips: Arc<DashMap<String, u8>>,
) {
    // Stores the next user id.
    let next_id = AtomicU32::new(0);

    tokio::spawn(async move {
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
            let client_ip = incoming_session
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.split(',').next())
                .map_or_else(
                    || incoming_session.remote_address().ip().to_string(),
                    |v| v.trim().to_string(),
                );
            let client_ip = normalize_ip(&client_ip);

            // Players caught scripting are blacklisted for a day.
            if systems::anticheat_system::is_blacklisted(&client_ip) {
                incoming_session.forbidden().await;
                continue;
            }

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
                    input_map,
                )
                .await;
            });
        }
    });
}

/// Reads a port number from an environment variable.
fn env_port(name: &str) -> Option<u16> {
    std::env::var(name).ok()?.parse().ok()
}

/// Returns the value of `key` in a URL query string.
fn extract_query_param(path: &str, key: &str) -> Option<String> {
    path.split('?')
        .nth(1)?
        .split('&')
        .find(|p| p.starts_with(&format!("{key}=")))?
        .split('=')
        .nth(1)
        .map(std::string::ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::allowed_origin;

    #[test]
    fn origins_are_matched_against_the_list() {
        let req = "GET /challenge HTTP/1.1\r\nHost: x\r\nOrigin: http://game.example:8081\r\n\r\n";
        assert_eq!(allowed_origin(req, "*"), "http://game.example:8081");
        assert_eq!(
            allowed_origin(req, "http://localhost:5173, http://game.example:8081"),
            "http://game.example:8081"
        );
        assert_eq!(allowed_origin(req, "http://localhost:5173"), "http://localhost:5173");
        assert_eq!(allowed_origin("GET /challenge HTTP/1.1\r\n\r\n", ""), "*");
    }
}
