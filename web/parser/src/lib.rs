//! WASM helpers for the web client: the encrypted session handshake, packet
//! encoding/decoding, and the proof-of-work solver.

use std::cell::UnsafeCell;

use borsh_derive::{BorshDeserialize, BorshSerialize};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use hkdf::Hkdf;
use pqc_kyber::{
    KYBER_CIPHERTEXTBYTES, KYBER_PUBLICKEYBYTES, KYBER_SECRETKEYBYTES, decapsulate, keypair,
};
use serde::{Serialize, de::DeserializeOwned};
use sha2::Sha256;
use shared::{
    PacketType,
    to_client::{
        AddObjectData, AddPlayerData, AddProjectileData, AgeData, AnimalsData, ArenaStateData,
        CaveCellsData, ChatData, ChestData, ClanStateData, DeathData, ExplosionData, HitEventTO,
        LeaderboardData, LoadoutData, MinimapData, NoticeData, ObjectHitAnimData, QuestData,
        RemoveObjectData, RemoveProjectileData, SetResourceData, ShopData, SpawnedData,
        UpdateHealthData, UpdateObjectHealthData, UpdatePlayerData, UpgradesData, WorldEventData,
    },
    to_server::{
        AimMessage, ArenaMessage, AttackMessage, CaveMessage, ChatMessage, ChestMessage,
        ChooseUpgradeMessage, ChooseWeapon, ClanMessage, MoveMessage, ShopMessage, SpawnMessage,
        TradeMessage, UseItemMessage,
    },
};
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey, StaticSecret};

/// First handshake message, sent by the client.
#[derive(BorshSerialize, BorshDeserialize)]
struct ClientHello {
    /// Client x25519 public key.
    x25519_pk: [u8; 32],
    /// Client Kyber public key.
    kyber_pk: Vec<u8>,
}

/// Handshake reply from the server.
#[derive(BorshSerialize, BorshDeserialize)]
struct ServerHello {
    /// Server x25519 public key.
    x25519_pk: [u8; 32],
    /// Kyber ciphertext encapsulated to the client's key.
    kyber_ct: Vec<u8>,
}

/// An established encrypted session.
#[wasm_bindgen]
pub struct SessionCrypto {
    /// Cipher keyed with the session key.
    cipher: ChaCha20Poly1305,
    /// Nonce for the next outgoing packet.
    send_nonce: u64,
    /// Nonce counter for incoming packets (debug only).
    recv_nonce: u64,
}

#[wasm_bindgen]
impl SessionCrypto {
    /// Builds a 12-byte nonce from a counter.
    fn make_nonce(counter: u64) -> Nonce {
        let mut bytes = [0u8; 12];
        bytes[4..].copy_from_slice(&counter.to_be_bytes());
        *Nonce::from_slice(&bytes)
    }

    /// Encrypts a packet. Output is `[nonce (8 bytes) | ciphertext]`.
    ///
    /// # Errors
    /// Returns an error if encryption fails.
    #[wasm_bindgen]
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, JsValue> {
        let nonce_value = self.send_nonce;
        self.send_nonce = self.send_nonce.wrapping_add(1);

        let nonce = Self::make_nonce(nonce_value);

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| JsValue::from_str(&format!("encryption failed: {e}")))?;

        let mut packet = nonce_value.to_be_bytes().to_vec();
        packet.extend(ciphertext);

        Ok(packet)
    }

    /// Decrypts a `[nonce (8 bytes) | ciphertext]` packet.
    ///
    /// # Errors
    /// Returns an error if the packet is too short or fails to decrypt.
    #[wasm_bindgen]
    pub fn decrypt(&self, packet: &[u8]) -> Result<Vec<u8>, JsValue> {
        if packet.len() < 8 {
            return Err(JsValue::from_str("packet too small"));
        }

        let (nonce_bytes, ciphertext) = packet.split_at(8);

        let nonce_value = u64::from_be_bytes(
            nonce_bytes
                .try_into()
                .map_err(|_| JsValue::from_str("invalid nonce"))?,
        );

        let nonce = Self::make_nonce(nonce_value);

        self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| JsValue::from_str(&format!("decryption failed: {e}")))
    }

    /// Nonce of the next outgoing packet (debug).
    // wasm-bindgen does not support `const fn`.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    #[wasm_bindgen]
    pub fn get_send_nonce(&self) -> u64 {
        self.send_nonce
    }

    /// Incoming nonce counter (debug).
    // wasm-bindgen does not support `const fn`.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    #[wasm_bindgen]
    pub fn get_recv_nonce(&self) -> u64 {
        self.recv_nonce
    }
}

/// Client secrets kept between sending `ClientHello` and receiving
/// `ServerHello`.
#[wasm_bindgen]
pub struct HandshakeState {
    /// x25519 secret key bytes.
    x25519_secret: Vec<u8>,
    /// Kyber secret key bytes.
    kyber_secret: Vec<u8>,
}

#[wasm_bindgen]
impl HandshakeState {
    /// Generates fresh keys and the serialized `ClientHello` message.
    ///
    /// # Errors
    /// Returns an error if key generation or serialization fails.
    #[wasm_bindgen]
    pub fn create_client_hello() -> Result<HandshakeStateAndMessage, JsValue> {
        let mut rng = rand_core::OsRng;

        let x25519_secret = StaticSecret::random_from_rng(rng);
        let x25519_public = PublicKey::from(&x25519_secret);

        let keys = keypair(&mut rng)
            .map_err(|e| JsValue::from_str(&format!("kyber keygen failed: {e:?}")))?;

        if keys.public.len() != KYBER_PUBLICKEYBYTES {
            return Err(JsValue::from_str("invalid kyber public key size"));
        }
        if keys.secret.len() != KYBER_SECRETKEYBYTES {
            return Err(JsValue::from_str("invalid kyber secret key size"));
        }

        let client_hello = ClientHello {
            x25519_pk: *x25519_public.as_bytes(),
            kyber_pk: keys.public.to_vec(),
        };

        let serialized = borsh::to_vec(&client_hello)
            .map_err(|e| JsValue::from_str(&format!("serialization failed: {e}")))?;

        let state = Self {
            x25519_secret: x25519_secret.to_bytes().to_vec(),
            kyber_secret: keys.secret.to_vec(),
        };

        Ok(HandshakeStateAndMessage {
            state,
            message: serialized,
        })
    }

    /// Finishes the handshake with the server's reply and derives the session
    /// key.
    ///
    /// # Errors
    /// Returns an error if the reply is malformed or key derivation fails.
    #[wasm_bindgen]
    pub fn complete_handshake(self, server_hello_bytes: &[u8]) -> Result<SessionCrypto, JsValue> {
        let server_hello: ServerHello = borsh::from_slice(server_hello_bytes)
            .map_err(|e| JsValue::from_str(&format!("failed to parse ServerHello: {e}")))?;

        if server_hello.kyber_ct.len() != KYBER_CIPHERTEXTBYTES {
            return Err(JsValue::from_str(&format!(
                "invalid kyber ciphertext size: expected {KYBER_CIPHERTEXTBYTES}, got {}",
                server_hello.kyber_ct.len()
            )));
        }

        let x25519_secret_bytes: [u8; 32] = self
            .x25519_secret
            .try_into()
            .map_err(|_| JsValue::from_str("invalid x25519 secret key length"))?;
        let x25519_secret = StaticSecret::from(x25519_secret_bytes);

        // x25519 ecdh
        let server_x25519_pk = PublicKey::from(server_hello.x25519_pk);
        let x25519_shared = x25519_secret.diffie_hellman(&server_x25519_pk);

        let kyber_shared = decapsulate(&server_hello.kyber_ct, &self.kyber_secret)
            .map_err(|e| JsValue::from_str(&format!("kyber decapsulation failed: {e:?}")))?;

        let mut combined_secret = Vec::with_capacity(32 + kyber_shared.len());
        combined_secret.extend_from_slice(x25519_shared.as_bytes());
        combined_secret.extend_from_slice(&kyber_shared);

        let hkdf = Hkdf::<Sha256>::new(None, &combined_secret);
        let mut session_key = [0u8; 32];
        hkdf.expand(b"agartha", &mut session_key)
            .map_err(|_| JsValue::from_str("hkdf expansion failed"))?;

        let key = Key::from_slice(&session_key);
        let cipher = ChaCha20Poly1305::new(key);

        Ok(SessionCrypto {
            cipher,
            send_nonce: 0,
            recv_nonce: 0,
        })
    }
}

/// Handshake state plus the `ClientHello` bytes to send.
#[wasm_bindgen]
pub struct HandshakeStateAndMessage {
    /// Secrets needed to finish the handshake.
    state: HandshakeState,
    /// Serialized `ClientHello`.
    message: Vec<u8>,
}

#[wasm_bindgen]
impl HandshakeStateAndMessage {
    /// The serialized `ClientHello` to send to the server.
    #[must_use]
    #[wasm_bindgen]
    pub fn message(&self) -> Vec<u8> {
        self.message.clone()
    }

    /// Finishes the handshake. See [`HandshakeState::complete_handshake`].
    ///
    /// # Errors
    /// Returns an error if the reply is malformed or key derivation fails.
    #[wasm_bindgen]
    pub fn complete_handshake(self, server_hello_bytes: &[u8]) -> Result<SessionCrypto, JsValue> {
        self.state.complete_handshake(server_hello_bytes)
    }
}

/// A decoded packet as handed to JS: `{ code, data }`.
#[derive(Serialize)]
struct DecodedPacket<T> {
    /// Packet opcode.
    code: u8,
    /// Packet body.
    data: T,
}

/// Size of the shared JS <-> WASM packet buffer.
const BUFFER_SIZE: usize = 512 * 1024;

/// Packet buffer that JS writes into directly.
struct SyncBuffer(UnsafeCell<[u8; BUFFER_SIZE]>);

// SAFETY: WASM runs single-threaded, so there is no concurrent access.
unsafe impl Sync for SyncBuffer {}

/// The shared packet buffer. Layout: `[length (u32 LE) | packet bytes]`.
static BUFFER: SyncBuffer = SyncBuffer(UnsafeCell::new([0u8; BUFFER_SIZE]));

/// Offset of the packet bytes in `BUFFER` (after the length prefix).
const DATA_OFFSET: usize = 4;

/// Pointer to the shared packet buffer, for JS to write into.
#[must_use]
#[wasm_bindgen]
pub fn get_buffer_ptr() -> *mut u8 {
    BUFFER.0.get().cast::<u8>()
}

/// Size of the shared packet buffer.
// wasm-bindgen does not support `const fn`.
#[allow(clippy::missing_const_for_fn)]
#[must_use]
#[wasm_bindgen]
pub fn get_buffer_size() -> usize {
    BUFFER_SIZE
}

/// Decodes a borsh body into `$T` and converts it into a JS `{ code, data }`
/// object.
macro_rules! decode_packet {
    ($code:expr, $bytes:expr, $T:ty) => {{
        let data = borsh::from_slice::<$T>($bytes)
            .map_err(|e| JsValue::from_str(&format!("error decoding {}: {}", stringify!($T), e)))?;
        to_js(&DecodedPacket { code: $code, data })
    }};
}

/// Converts a value into a JS value.
fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decodes the packet currently in the shared buffer.
///
/// # Errors
/// Returns an error if the packet is empty, has an unknown opcode, or fails to
/// decode.
#[wasm_bindgen]
pub fn decode_bytes() -> Result<JsValue, JsValue> {
    // SAFETY: JS writes the length prefix and packet into BUFFER right before
    // calling this, and WASM is single-threaded.
    let bytes = unsafe {
        let buf = &*BUFFER.0.get();
        let length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        let end = DATA_OFFSET.saturating_add(length).min(BUFFER_SIZE);
        &buf[DATA_OFFSET..end]
    };

    decode_bytes_inner(bytes)
}

/// Decodes a packet: one opcode byte followed by a borsh body.
fn decode_bytes_inner(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let Some((&code, body)) = bytes.split_first() else {
        return Err(JsValue::from_str("empty"));
    };

    match PacketType::from_u8(code) {
        Some(PacketType::AddPlayer) => decode_packet!(code, body, AddPlayerData),
        Some(PacketType::UpdatePlayers) => decode_packet!(code, body, UpdatePlayerData),
        Some(PacketType::Animals) => decode_packet!(code, body, AnimalsData),
        Some(PacketType::HitEvent) => decode_packet!(code, body, HitEventTO),
        Some(PacketType::AddObject) => decode_packet!(code, body, AddObjectData),
        Some(PacketType::UpdateHealth) => decode_packet!(code, body, UpdateHealthData),
        Some(PacketType::Loadout) => decode_packet!(code, body, LoadoutData),
        Some(PacketType::ObjectHitAnim) => decode_packet!(code, body, ObjectHitAnimData),
        Some(PacketType::SetResource) => decode_packet!(code, body, SetResourceData),
        Some(PacketType::Death) => decode_packet!(code, body, DeathData),
        Some(PacketType::Chat) => decode_packet!(code, body, ChatData),
        Some(PacketType::Upgrades) => decode_packet!(code, body, UpgradesData),
        Some(PacketType::Leaderboard) => decode_packet!(code, body, LeaderboardData),
        Some(PacketType::UpdateObjectHealth) => decode_packet!(code, body, UpdateObjectHealthData),
        Some(PacketType::RemoveObject) => decode_packet!(code, body, RemoveObjectData),
        Some(PacketType::AddProjectile) => decode_packet!(code, body, AddProjectileData),
        Some(PacketType::RemoveProjectile) => decode_packet!(code, body, RemoveProjectileData),
        Some(PacketType::Age) => decode_packet!(code, body, AgeData),
        Some(PacketType::Notice) => decode_packet!(code, body, NoticeData),
        Some(PacketType::Explosion) => decode_packet!(code, body, ExplosionData),
        Some(PacketType::ClanState) => decode_packet!(code, body, ClanStateData),
        Some(PacketType::ShopState) => decode_packet!(code, body, ShopData),
        Some(PacketType::Quest) => decode_packet!(code, body, QuestData),
        Some(PacketType::WorldEvent) => decode_packet!(code, body, WorldEventData),
        Some(PacketType::ChestState) => decode_packet!(code, body, ChestData),
        Some(PacketType::Minimap) => decode_packet!(code, body, MinimapData),
        Some(PacketType::Spawned) => decode_packet!(code, body, SpawnedData),
        Some(PacketType::ArenaState) => decode_packet!(code, body, ArenaStateData),
        Some(PacketType::CaveCells) => decode_packet!(code, body, CaveCellsData),
        Some(PacketType::RemovePlayer) => {
            let id = body
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| JsValue::from_str("bad remove packet"))?;
            to_js(&DecodedPacket {
                code,
                data: RemovedPlayer { id },
            })
        }
        None => Err(JsValue::from_str("unknown opcode")),
        Some(_) => Err(JsValue::from_str("not a client packet")),
    }
}

/// Body of a remove-player packet.
#[derive(Serialize)]
struct RemovedPlayer {
    /// Player id.
    id: u32,
}

/// Converts a JS value into `T` and appends its borsh encoding to `buf`.
fn encode_as<T: DeserializeOwned + borsh::BorshSerialize>(
    packet: JsValue,
    buf: &mut Vec<u8>,
    name: &str,
) -> Result<(), JsValue> {
    let value: T =
        serde_wasm_bindgen::from_value(packet).map_err(|x| JsValue::from_str(&x.to_string()))?;

    borsh::to_writer(buf, &value)
        .map_err(|e| JsValue::from_str(&format!("error encoding {name} {e}")))
}

/// Encodes a JS packet for the server: one opcode byte followed by a borsh
/// body.
///
/// # Errors
/// Returns an error if the opcode is unknown or the packet doesn't match it.
#[wasm_bindgen]
pub fn encode_into_bytes(packet: JsValue, opcode: u8) -> Result<Box<[u8]>, JsValue> {
    let mut buf = vec![opcode];

    match PacketType::from_u8(opcode) {
        Some(PacketType::Spawn) => encode_as::<SpawnMessage>(packet, &mut buf, "spawn")?,
        Some(PacketType::Move) => encode_as::<MoveMessage>(packet, &mut buf, "move")?,
        Some(PacketType::Aim) => encode_as::<AimMessage>(packet, &mut buf, "aim")?,
        Some(PacketType::Attack) => encode_as::<AttackMessage>(packet, &mut buf, "attack")?,
        Some(PacketType::ChatMessage) => encode_as::<ChatMessage>(packet, &mut buf, "chat")?,
        Some(PacketType::ChooseWeapon) => {
            encode_as::<ChooseWeapon>(packet, &mut buf, "choose weapon")?;
        }
        Some(PacketType::UseItem) => encode_as::<UseItemMessage>(packet, &mut buf, "use item")?,
        Some(PacketType::ChooseUpgrade) => {
            encode_as::<ChooseUpgradeMessage>(packet, &mut buf, "upgrade")?;
        }
        Some(PacketType::Shop) => encode_as::<ShopMessage>(packet, &mut buf, "shop")?,
        Some(PacketType::Clan) => encode_as::<ClanMessage>(packet, &mut buf, "clan")?,
        Some(PacketType::Chest) => encode_as::<ChestMessage>(packet, &mut buf, "chest")?,
        Some(PacketType::Trade) => encode_as::<TradeMessage>(packet, &mut buf, "trade")?,
        Some(PacketType::Arena) => encode_as::<ArenaMessage>(packet, &mut buf, "arena")?,
        Some(PacketType::Cave) => encode_as::<CaveMessage>(packet, &mut buf, "cave")?,
        _ => return Err(JsValue::from_str("unknown opcode")),
    }

    Ok(buf.into_boxed_slice())
}

/// Finds a nonce such that `sha256(challenge + nonce)` starts with at least
/// `difficulty` zero bits. Returns the nonce as a decimal string.
#[must_use]
#[wasm_bindgen]
pub fn solve_pow(challenge: &str, difficulty: u32) -> String {
    use sha2::Digest;

    let challenge_bytes = challenge.as_bytes();
    let mut buf = Vec::with_capacity(challenge_bytes.len() + 20);
    buf.extend_from_slice(challenge_bytes);

    let prefix_len = challenge_bytes.len();
    let mut nonce: u64 = 0;

    loop {
        buf.truncate(prefix_len);
        write_u64(&mut buf, nonce);

        let hash = sha2::Sha256::digest(&buf);
        if leading_zero_bits(&hash) >= difficulty {
            return nonce.to_string();
        }
        nonce = nonce.wrapping_add(1);
    }
}

/// Appends the decimal digits of `n` to `buf`.
fn write_u64(buf: &mut Vec<u8>, mut n: u64) {
    if n == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    while n > 0 {
        // `n % 10` is always a single digit.
        #[allow(clippy::cast_possible_truncation)]
        buf.push(b'0' + (n % 10) as u8);
        n /= 10;
    }
    buf[start..].reverse();
}

/// Counts the leading zero bits of a hash.
fn leading_zero_bits(hash: &[u8]) -> u32 {
    let mut count = 0;
    for byte in hash {
        count += byte.leading_zeros();
        if *byte != 0 {
            break;
        }
    }
    count
}
