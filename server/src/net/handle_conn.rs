use std::time::Duration;

use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::{
    net::{handle_msgs::handle_msg, perform_handshake, pow::Pow},
    structs::bevy::{InputMap, PlayerConnection},
    ConnectionMap, GameChannel,
};

/// Longest a client may take to send its half of the handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest a client may take to solve the proof-of-work challenge.
const POW_TIMEOUT: Duration = Duration::from_secs(30);

/// The next binary message, or None if the client hangs up or takes longer
/// than `limit`.
async fn next_binary(ws: &mut WebSocketStream<TcpStream>, limit: Duration) -> Option<Vec<u8>> {
    tokio::time::timeout(limit, async {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Binary(bytes) = msg {
                return Some(bytes.to_vec());
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// Makes the client solve a proof-of-work challenge: the server sends the
/// challenge (text, as a binary message) and the client answers with the
/// nonce. True if it was solved.
async fn proof_of_work(ws: &mut WebSocketStream<TcpStream>, pow: &Pow) -> bool {
    let challenge = pow.generate_challenge();
    if ws
        .send(Message::Binary(challenge.clone().into_bytes().into()))
        .await
        .is_err()
    {
        return false;
    }
    let Some(answer) = next_binary(ws, POW_TIMEOUT).await else {
        return false;
    };
    let nonce = String::from_utf8_lossy(&answer);
    pow.verify(&challenge, &nonce)
}

/// Runs one player's WebSocket connection until it closes.
#[allow(clippy::too_many_arguments, reason = "each is a separate shared map or channel")]
pub async fn handle_conn(
    mut ws: WebSocketStream<TcpStream>,
    pow: &Pow,
    player_id: u32,
    client_ip: String,
    ips: &DashMap<String, u8>,
    connection_map: ConnectionMap,
    game_tx: GameChannel,
    input_map: InputMap,
) {
    // The client proves some work before anything else is done for it.
    if !proof_of_work(&mut ws, pow).await {
        tracing::info!("player {} failed the proof of work", player_id);
        let _ = ips.remove(&client_ip);
        let _ = ws.close(None).await;
        return;
    }

    // We need to verify that the client is not a bot. The lookups are
    // blocking HTTP requests, so they run on a blocking thread.
    let lookup_ip = client_ip.clone();
    let is_bad_actor = tokio::task::spawn_blocking(move || is_bad_ip(&lookup_ip))
        .await
        .unwrap_or(false);

    if is_bad_actor {
        tracing::error!("Closed connection due to Anti bot service.");
        let _ = ips.remove(&client_ip);
        let _ = ws.close(None).await;
        return;
    }

    // The first binary message is the client's half of the handshake.
    let Some(hello) = next_binary(&mut ws, HANDSHAKE_TIMEOUT).await else {
        tracing::error!("player {} sent no handshake", player_id);
        let _ = ips.remove(&client_ip);
        return;
    };

    // Perform the handshake. Stores a `SessionCrypto` used to decrypt incoming
    // messages and encrypt.
    let crypto = match perform_handshake(&hello) {
        Ok((crypto, reply)) => {
            if ws.send(Message::Binary(reply.into())).await.is_err() {
                let _ = ips.remove(&client_ip);
                return;
            }
            crypto
        }
        Err(e) => {
            tracing::error!("handshake failed for player {}: {}", player_id, e);
            let _ = ips.remove(&client_ip);
            let _ = ws.close(None).await;
            return;
        }
    };

    // The writer task gets the sending half; this task keeps reading.
    let (sink, mut stream) = ws.split();
    let connection = PlayerConnection::new(sink, crypto.clone(), client_ip.clone());
    let mut closed = connection.closed();
    connection_map.insert(player_id, connection);
    tracing::info!("player {} connected - session encrypted", player_id);

    // Anything at all (including the answers to our pings) counts as alive.
    let idle = Duration::from_secs(crate::CONFIG.game.idle_timeout_secs);

    loop {
        let msg = tokio::select! {
            msg = tokio::time::timeout(idle, stream.next()) => msg,
            _ = closed.changed() => break,
        };
        let packet = match msg {
            Ok(Some(Ok(Message::Binary(packet)))) => packet,
            Ok(Some(Ok(Message::Close(_)) | Err(_)) | None) => break,
            Ok(Some(Ok(_))) => continue,
            Err(_) => {
                tracing::info!("player {} timed out", player_id);
                break;
            }
        };

        // Track bytes received
        if let Some(conn) = connection_map.get(&player_id) {
            conn.bytes_recv
                .fetch_add(packet.len() as u64, std::sync::atomic::Ordering::Relaxed);
        }

        // Decrypt the incoming message.
        let plaintext = match crypto.decrypt_datagram(&packet) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Decryption failed for player {}: {:?}", player_id, e);
                continue;
            }
        };

        // The first byte is the opcode: which packet to parse the rest as.
        if let Some((&opcode, body)) = plaintext.split_first() {
            handle_msg(opcode, body, player_id, &game_tx, &input_map);
        }
    }

    tracing::info!("player {} disconnected", player_id);

    // Forget the connection (which stops its writer) and tell the game loop,
    // so the player is removed from the world.
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
