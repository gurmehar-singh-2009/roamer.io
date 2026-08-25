// TODO: Comment this.
// Also potentially rewrite how packets are structured.

use serde::Serialize;
use shared::structs::server::{ChatMessage, InternalChatMessage};
use shared::to_client::{
    AddAnimalData, HitEventTO, ObjectHitAnimData, SetResourceData, SetUpgradesData, SetWeaponsData, UpdateHealthData, UpdatePlayerData
};
use shared::to_server::{AimMessage, ChooseWeapon, ClientMessages, HitMessage, MoveMessage};
use shared::{
    PacketType,
    structs::server::{Aim, Move},
    to_server::SpawnMessage,
};
use wasm_bindgen::prelude::*;

use borsh_derive::{BorshDeserialize, BorshSerialize};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use hkdf::Hkdf;
use pqc_kyber::{
    KYBER_CIPHERTEXTBYTES, KYBER_PUBLICKEYBYTES, KYBER_SECRETKEYBYTES, decapsulate, keypair,
};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(BorshSerialize, BorshDeserialize)]
struct ClientHello {
    x25519_pk: [u8; 32],
    kyber_pk: Vec<u8>,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct ServerHello {
    x25519_pk: [u8; 32],
    kyber_ct: Vec<u8>,
}

#[wasm_bindgen]
pub struct SessionCrypto {
    cipher: ChaCha20Poly1305,
    send_nonce: u64,
    recv_nonce: u64,
}

#[wasm_bindgen]
impl SessionCrypto {
    fn make_nonce(counter: u64) -> Nonce {
        let mut bytes = [0u8; 12];
        bytes[4..].copy_from_slice(&counter.to_be_bytes());
        *Nonce::from_slice(&bytes)
    }

    #[wasm_bindgen]
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, JsValue> {
        let nonce_value = self.send_nonce;
        self.send_nonce = self.send_nonce.wrapping_add(1);

        let nonce = Self::make_nonce(nonce_value);

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| JsValue::from_str(&format!("encryption failed: {}", e)))?;

        let mut packet = nonce_value.to_be_bytes().to_vec();
        packet.extend(ciphertext);

        Ok(packet)
    }

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
            .map_err(|e| JsValue::from_str(&format!("decryption failed: {}", e)))
    }

    // debug stuff
    #[wasm_bindgen]
    pub fn get_send_nonce(&self) -> u64 {
        self.send_nonce
    }

    #[wasm_bindgen]
    pub fn get_recv_nonce(&self) -> u64 {
        self.recv_nonce
    }
}

#[wasm_bindgen]
pub struct HandshakeState {
    x25519_secret: Vec<u8>,
    kyber_secret: Vec<u8>,
}

#[wasm_bindgen]
impl HandshakeState {
    #[wasm_bindgen]
    pub fn create_client_hello() -> Result<HandshakeStateAndMessage, JsValue> {
        let mut rng = rand_core::OsRng;

        let x25519_secret = StaticSecret::random_from_rng(rng);
        let x25519_public = PublicKey::from(&x25519_secret);

        let keys = keypair(&mut rng)
            .map_err(|e| JsValue::from_str(&format!("kyber keygen failed: {:?}", e)))?;

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
            .map_err(|e| JsValue::from_str(&format!("serialization failed: {}", e)))?;

        let state = HandshakeState {
            x25519_secret: x25519_secret.to_bytes().to_vec(),
            kyber_secret: keys.secret.to_vec(),
        };

        Ok(HandshakeStateAndMessage {
            state,
            message: serialized,
        })
    }

    #[wasm_bindgen]
    pub fn complete_handshake(self, server_hello_bytes: &[u8]) -> Result<SessionCrypto, JsValue> {
        let server_hello: ServerHello = borsh::from_slice(server_hello_bytes)
            .map_err(|e| JsValue::from_str(&format!("failed to parse ServerHello: {}", e)))?;

        if server_hello.kyber_ct.len() != KYBER_CIPHERTEXTBYTES {
            return Err(JsValue::from_str(&format!(
                "invalid kyber ciphertext size: expected {}, got {}",
                KYBER_CIPHERTEXTBYTES,
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
            .map_err(|e| JsValue::from_str(&format!("kyber decapsulation failed: {:?}", e)))?;

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

#[wasm_bindgen]
pub struct HandshakeStateAndMessage {
    state: HandshakeState,
    message: Vec<u8>,
}

#[wasm_bindgen]
impl HandshakeStateAndMessage {
    #[wasm_bindgen]
    pub fn message(&self) -> Vec<u8> {
        self.message.clone()
    }

    #[wasm_bindgen]
    pub fn complete_handshake(self, server_hello_bytes: &[u8]) -> Result<SessionCrypto, JsValue> {
        self.state.complete_handshake(server_hello_bytes)
    }
}

#[derive(Serialize)]
struct DecodedPacket<T> {
    code: u8,
    data: T,
}

use std::cell::UnsafeCell;

const BUFFER_SIZE: usize = 512 * 1024;

struct SyncBuffer(UnsafeCell<[u8; BUFFER_SIZE]>);
unsafe impl Sync for SyncBuffer {}

static BUFFER: SyncBuffer = SyncBuffer(UnsafeCell::new([0u8; BUFFER_SIZE]));

const DATA_OFFSET: usize = 4;

#[wasm_bindgen]
pub fn get_buffer_ptr() -> *mut u8 {
    unsafe { (*BUFFER.0.get()).as_mut_ptr() }
}

#[wasm_bindgen]
pub fn get_buffer_size() -> usize {
    BUFFER_SIZE
}

macro_rules! decode_packet {
    ($code:expr, $bytes:expr, $T:ty) => {{
        let data = borsh::from_slice::<$T>($bytes)
            .map_err(|e| JsValue::from_str(&format!("error decoding {}: {}", stringify!($T), e)))?;
        let packet = DecodedPacket { code: $code, data };
        Ok(serde_wasm_bindgen::to_value(&packet).map_err(|e| JsValue::from_str(&e.to_string()))?)
    }};
}

#[wasm_bindgen]
pub fn decode_bytes() -> Result<JsValue, JsValue> {
    let bytes = unsafe {
        let ptr = BUFFER.0.get();
        let buf = &*ptr;
        let length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        std::slice::from_raw_parts(buf.as_ptr().add(DATA_OFFSET), length)
    };

    decode_bytes_inner(bytes)
}
fn decode_bytes_inner(bytes: &[u8]) -> Result<JsValue, JsValue> {
    if bytes.is_empty() {
        return Err(JsValue::from_str("empty"));
    }

    let opcode = bytes.first();

    match opcode {
        Some(code) => match PacketType::from_u8(*code) {
            Some(PacketType::Spawn) => {
                let data = borsh::from_slice::<ClientMessages>(&bytes[1..])
                    .map_err(|e| JsValue::from_str(&format!("error decoding player {}", e)))?;

                let data = match data {
                    ClientMessages::AddPlayer(d) => d,
                    _ => {
                        return Err(JsValue::from_str("invalid spawn packet"));
                    }
                };

                let packet = DecodedPacket { code: *code, data };
                Ok(serde_wasm_bindgen::to_value(&packet)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?)
            }
            Some(PacketType::AddObject) => {
                let data = borsh::from_slice::<ClientMessages>(&bytes[1..])
                    .map_err(|e| JsValue::from_str(&format!("error decoding addobject {}", e)))?;

                let data = match data {
                    ClientMessages::AddObject(d) => d,
                    _ => {
                        return Err(JsValue::from_str("invalid object packet"));
                    }
                };

                let packet = DecodedPacket { code: *code, data };

                Ok(serde_wasm_bindgen::to_value(&packet)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?)
            }
            Some(PacketType::Move) => decode_packet!(*code, &bytes[1..], Move),
            Some(PacketType::Aim) => decode_packet!(*code, &bytes[1..], Aim),
            Some(PacketType::HitEvent) => decode_packet!(*code, &bytes[1..], HitEventTO),
            Some(PacketType::UpdateHealth) => decode_packet!(*code, &bytes[1..], UpdateHealthData),
            Some(PacketType::UpdatePlayers) => decode_packet!(*code, &bytes[1..], UpdatePlayerData),
            Some(PacketType::AddAnimal) => decode_packet!(*code, &bytes[1..], AddAnimalData),
            Some(PacketType::SetWeapons) => decode_packet!(*code, &bytes[1..], SetWeaponsData),
            Some(PacketType::ObjectHitAnim) => {
                decode_packet!(*code, &bytes[1..], ObjectHitAnimData)
            }
            Some(PacketType::InternalChatMessage) => decode_packet!(*code, &bytes[1..], InternalChatMessage),
            Some(PacketType::SetResource) => decode_packet!(*code, &bytes[1..], SetResourceData),
            Some(PacketType::SetUpgrades) => decode_packet!(*code, &bytes[1..], SetUpgradesData),
            None => Err(JsValue::from_str("unknown opcode1")),

            _ => Err(JsValue::from_str("not a valid packet"))
        },
        None => Err(JsValue::from_str("no opcode found")),
    }
}

#[wasm_bindgen]
pub fn encode_into_bytes(packet: JsValue, opcode: u8) -> Result<Box<[u8]>, JsValue> {
    let mut buf = vec![opcode];

    match opcode {
        1 => {
            let spawn: SpawnMessage = serde_wasm_bindgen::from_value(packet)
                .map_err(|x| JsValue::from_str(&x.to_string()))?;
            if let Err(e) = borsh::to_writer(&mut buf, &spawn) {
                return Err(JsValue::from_str(&format!(
                    "error encoding spawn msg {}",
                    e
                )));
            }
        }
        2 => {
            let js_move: MoveMessage = serde_wasm_bindgen::from_value(packet)
                .map_err(|x| JsValue::from_str(&x.to_string()))?;
            if let Err(e) = borsh::to_writer(&mut buf, &js_move) {
                return Err(JsValue::from_str(&format!("error encoding move {}", e)));
            }
        }
        5 => {
            let js_aim: AimMessage = serde_wasm_bindgen::from_value(packet)
                .map_err(|x| JsValue::from_str(&x.to_string()))?;

            if let Err(e) = borsh::to_writer(&mut buf, &js_aim) {
                return Err(JsValue::from_str(&format!("error encoding move {}", e)));
            }
        }
        6 => {
            let js_hit: HitMessage = serde_wasm_bindgen::from_value(packet)
                .map_err(|x| JsValue::from_str(&x.to_string()))?;

            if let Err(e) = borsh::to_writer(&mut buf, &js_hit) {
                return Err(JsValue::from_str(&format!("error encoding move {}", e)));
            }
        }
        12 => {
            let js_msg: ChatMessage = serde_wasm_bindgen::from_value(packet)
                .map_err(|x| JsValue::from_str(&x.to_string()))?;

            if let Err(e) = borsh::to_writer(&mut buf, &js_msg) {
                return Err(JsValue::from_str(&format!("error encoding chat msg {}", e)));
            }
        }
        15 => {
            let js_weapon: ChooseWeapon = serde_wasm_bindgen::from_value(packet)
                .map_err(|x| JsValue::from_str(&x.to_string()))?;

            if let Err(e) = borsh::to_writer(&mut buf, &js_weapon) {
                return Err(JsValue::from_str(&format!("error encoding choose weapon {}", e)));
            }
        }
        _ => return Err(JsValue::from_str("unknown opcode")),
    }

    Ok(buf.into_boxed_slice())
}

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

fn write_u64(buf: &mut Vec<u8>, mut n: u64) {
    if n == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    while n > 0 {
        buf.push(b'0' + (n % 10) as u8);
        n /= 10;
    }
    buf[start..].reverse();
}

fn leading_zero_bits(hash: &[u8]) -> u32 {
    let mut count = 0;
    for byte in hash {
        count += byte.leading_zeros();
        if *byte != 0 { break; }
    }
    count
}