use dashmap::DashMap;
use wtransport::VarInt;

use crate::{
    net::{handle_msgs::handle_msg, perform_handshake},
    structs::bevy::{InputMap, PlayerConnection},
    ConnectionMap, GameChannel,
};

/// Exported fn that handles wtransport connection requests.
pub async fn handle_conn(
    connection: wtransport::Connection,
    player_id: u32,
    client_ip: String,
    ips: &DashMap<String, u8>,
    connection_map: ConnectionMap,
    game_tx: GameChannel,
    input_map: InputMap,
) {
    // We need to verify that the client is not a bot. The lookups are
    // blocking HTTP requests, so they run on a blocking thread.
    let lookup_ip = client_ip.clone();
    let is_bad_actor = tokio::task::spawn_blocking(move || is_bad_ip(&lookup_ip))
        .await
        .unwrap_or(false);

    if is_bad_actor {
        tracing::error!("Closed connection due to Anti bot service.");
        let _ = ips.remove(&client_ip);
        connection.close(VarInt::from_u32(41), b"noob");
        return;
    }

    // Accept bidirectional stream for the handshake.
    let (mut send_stream, mut recv_stream) = match connection.accept_bi().await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to open bidirectional stream for the handshake: {}", e);
            let _ = ips.remove(&client_ip);
            return;
        }
    };

    // Perform the handshake. Stores a `SessionCrypto` used to decrypt incoming
    // messages and encrypt.
    let crypto = match perform_handshake(&mut send_stream, &mut recv_stream).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("handshake failed for player {}: {}", player_id, e);
            let _ = ips.remove(&client_ip);
            connection.close(VarInt::from_u32(1), b"handshake_failed");
            return;
        }
    };

    // Drop streams after handshake.
    drop(send_stream);
    drop(recv_stream);

    // Store the new player entry.
    connection_map.insert(
        player_id,
        PlayerConnection::new(connection.clone(), crypto.clone(), client_ip.clone()),
    );
    tracing::info!("player {} connected - session encrypted", player_id);

    // let message_count = Arc::new(AtomicU8::new(0));
    // let message_count_watcher = Arc::clone(&message_count);

    // let anti_packet_spam_connection_clone = connection.clone();
    // tokio::spawn(async move {
    //     let mut clear_interval =
    // tokio::time::interval(std::time::Duration::from_secs(1));     loop {
    //         clear_interval.tick().await;
    //         let count = message_count_watcher.swap(0, Ordering::Relaxed);
    //         if count > 100 {
    //             let _ =
    // anti_packet_spam_connection_clone.close(VarInt::from_u32(41), b"noob");
    //             break;
    //         }
    //         println!("message_count: {}", count);
    //     }
    // });

    // Reliable messages (everything but movement and aim) arrive on
    // unidirectional streams as [length: u32 BE][encrypted packet] frames.
    let stream_task = {
        let connection = connection.clone();
        let crypto = crypto.clone();
        let game_tx = game_tx.clone();
        let input_map = input_map.clone();
        tokio::spawn(async move {
            while let Ok(stream) = connection.accept_uni().await {
                let crypto = crypto.clone();
                let game_tx = game_tx.clone();
                let input_map = input_map.clone();
                tokio::spawn(async move {
                    read_reliable(stream, &crypto, player_id, &game_tx, &input_map).await;
                });
            }
        })
    };

    // Message read loop.
    loop {
        // Process datagram messages.
        let datagram = match connection.receive_datagram().await {
            Ok(d) => d,
            Err(e) => {
                tracing::info!("player {} disconnected: {}", player_id, e);
                break;
            }
        };

        // message_count.fetch_add(1, Ordering::Relaxed);

        // Discard invalid messages (less than 8 bytes is not possible with our
        // message structure).
        if datagram.len() < 8 {
            continue;
        }

        // Track bytes received
        if let Some(conn) = connection_map.get(&player_id) {
            conn.bytes_recv
                .fetch_add(datagram.len() as u64, std::sync::atomic::Ordering::Relaxed);
        }

        // Decrypt the incoming message.
        let plaintext = match crypto.decrypt_datagram(&datagram) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Decryption failed for player {}: {:?}", player_id, e);
                continue;
            }
        };

        // Discard empty messages.
        if plaintext.is_empty() {
            continue;
        }

        // Extract the opcode from the plaintext.
        // Tells us which packet to parse the bytes as.
        let opcode = plaintext[0];

        handle_msg(opcode, &plaintext[1..], player_id, &game_tx, &input_map);
    }

    tracing::info!("player {} disconnected", player_id);

    stream_task.abort();

    // Forget the connection and tell the game loop, so the player is removed
    // from the world.
    connection_map.remove(&player_id);
    let _ = game_tx
        .send((player_id, crate::errors::InternalGameMessages::Disconnect))
        .await;

    // Remove the IP from the map when the connection is closed.
    let _ = ips.remove(&client_ip);
}

/// Asks the IP lookup services whether an IP is a crawler, datacenter, VPN,
/// proxy or Tor exit. Local and private addresses are never looked up.
fn is_bad_ip(ip: &str) -> bool {
    let is_local = match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        Ok(std::net::IpAddr::V6(v6)) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
        Err(_) => true,
    };
    if is_local {
        return false;
    }

    crate::net::ip_lookup::LookupProvider::all().iter().any(|provider| {
        crate::net::ip_lookup::lookup(ip, *provider).is_some_and(|c| {
            [
                c.connection.is_crawler,
                c.connection.is_datacenter,
                c.connection.is_vpn,
                c.connection.is_proxy,
                c.connection.is_tor,
            ]
            .into_iter()
            .flatten()
            .any(|v| v)
        })
    })
}

/// Largest reliable frame accepted.
const MAX_FRAME: usize = 64 * 1024;

/// Reads framed, encrypted messages from a client's reliable stream.
async fn read_reliable(
    mut stream: wtransport::RecvStream,
    crypto: &crate::net::SessionCrypto,
    player_id: u32,
    game_tx: &GameChannel,
    input_map: &InputMap,
) {
    loop {
        let mut len = [0u8; 4];
        if stream.read_exact(&mut len).await.is_err() {
            return;
        }
        let Ok(len) = usize::try_from(u32::from_be_bytes(len)) else {
            return;
        };
        if len > MAX_FRAME {
            return;
        }
        let mut packet = vec![0u8; len];
        if stream.read_exact(&mut packet).await.is_err() {
            return;
        }
        let Ok(plaintext) = crypto.decrypt_datagram(&packet) else {
            continue;
        };
        if let Some((&opcode, body)) = plaintext.split_first() {
            handle_msg(opcode, body, player_id, game_tx, input_map);
        }
    }
}
