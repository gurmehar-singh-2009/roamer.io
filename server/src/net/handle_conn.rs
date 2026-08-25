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
    // We need to verify that the client is not a bot.
    let mut is_bad_actor = false;

    for provider in crate::net::ip_lookup::LookupProvider::all() {
        if let Some(c) = &crate::net::ip_lookup::lookup(&client_ip, *provider) {
            is_bad_actor |= [
                c.connection.is_crawler,
                c.connection.is_datacenter,
                c.connection.is_vpn,
                c.connection.is_proxy,
                c.connection.is_tor,
            ]
            .into_iter()
            .flatten()
            .any(|v| v);

            if is_bad_actor {
                break;
            }
        }
    }

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
    connection_map.insert(player_id, PlayerConnection::new(connection.clone(), crypto.clone()));
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

        // Discard invalid messages (less than 8 bytes is not possible with our message
        // structure).
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
    
    // Remove the IP from the map when the connection is closed.
    let _ = ips.remove(&client_ip);
}
