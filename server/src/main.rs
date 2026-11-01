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
use tokio_tungstenite::tungstenite::{
    handshake::server::{ErrorResponse, Request, Response},
    http::StatusCode,
    protocol::WebSocketConfig,
};

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

    // Players connect with WebSockets, on every interface, IPv6 and IPv4.
    let server = TcpListener::bind(format!("[::]:{game_port}"))
        .await
        .ok()
        .context(SystemPanicSnafu)?;

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

/// Largest message a client may send.
const MAX_CLIENT_MESSAGE: usize = 64 * 1024;

/// Spawns the task that accepts incoming WebSocket connections.
fn spawn_accept_loop(
    server: TcpListener,
    pow: Arc<Pow>,
    connections: IDToConnection,
    tx: GameChannel,
    input_map: InputMap,
    ips: Arc<DashMap<String, u8>>,
) {
    // Stores the next user id.
    let next_id = Arc::new(AtomicU32::new(0));

    tokio::spawn(async move {
        loop {
            let (socket, peer) = match server.accept().await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to accept connection: {}", e);
                    continue;
                }
            };
            let _ = socket.set_nodelay(true);

            let pow = pow.clone();
            let connection_map = connections.clone();
            let game_tx = tx.clone();
            let input_map = input_map.clone();
            let ips = ips.clone();
            let next_id = next_id.clone();

            tokio::spawn(async move {
                // Filled in while the upgrade request is checked.
                let mut client_ip = String::new();
                #[allow(clippy::result_large_err, reason = "tungstenite sets the callback's types")]
                let check = |req: &Request, res: Response| -> Result<Response, ErrorResponse> {
                    client_ip = normalize_ip(&client_address(req, peer.ip()));

                    // Players caught scripting are blacklisted for a day.
                    if systems::anticheat_system::is_blacklisted(&client_ip) {
                        return Err(forbidden());
                    }

                    // Verify PoW.
                    let query = req.uri().query().map_or_else(String::new, |q| format!("?{q}"));
                    let nonce = extract_query_param(&query, "pow").unwrap_or_default();
                    let challenge_key = extract_query_param(&query, "challenge").unwrap_or_default();
                    if !pow.verify(&challenge_key, &nonce) {
                        return Err(forbidden());
                    }
                    Ok(res)
                };
                let config = WebSocketConfig::default()
                    .max_message_size(Some(MAX_CLIENT_MESSAGE))
                    .max_frame_size(Some(MAX_CLIENT_MESSAGE));
                let ws = match tokio_tungstenite::accept_hdr_async_with_config(socket, check, Some(config)).await {
                    Ok(ws) => ws,
                    Err(e) => {
                        tracing::info!("refused a connection from {}: {}", peer, e);
                        return;
                    }
                };

                let player_id = next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                ips.insert(client_ip.clone(), 1);
                net::handle_conn(ws, player_id, client_ip, &ips, connection_map, game_tx, input_map).await;
            });
        }
    });
}

/// The player's IP. Behind a proxy on this machine (the release's web client,
/// Caddy, nginx, Vite) it's the first `X-Forwarded-For` entry; from anywhere
/// else that header could be faked, so the socket's address is used.
fn client_address(req: &Request, peer: std::net::IpAddr) -> String {
    let forwarded = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    match forwarded {
        Some(ip) if peer.is_loopback() || peer.to_canonical().is_loopback() => ip.to_string(),
        _ => peer.to_canonical().to_string(),
    }
}

/// A 403 answer to a refused upgrade request.
fn forbidden() -> ErrorResponse {
    let mut res = ErrorResponse::new(None);
    *res.status_mut() = StatusCode::FORBIDDEN;
    res
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
