//! Decodes client packets into game messages.

use shared::{
    to_server::{
        AimMessage, ArenaMessage, AttackMessage, ChatMessage, ChestMessage, ChooseUpgradeMessage, ChooseWeapon,
        ClanMessage, MoveMessage, ShopMessage, SpawnMessage, TradeMessage, UseItemMessage,
    },
    PacketType,
};

use crate::{errors::InternalGameMessages, net::serialization::decode, structs::bevy::InputMap, GameChannel};

/// Max name length.
const MAX_NAME: usize = 16;
/// Max chat message length.
const MAX_CHAT: usize = 30;

/// Censors offensive words.
fn censor(text: &str) -> String {
    rustrict::Censor::from_str(text)
        .with_censor_first_character_threshold(rustrict::Type::OFFENSIVE & rustrict::Type::SEXUAL)
        .with_ignore_false_positives(false)
        .with_censor_replacement('*')
        .censor()
}

/// Handles one decrypted client packet.
pub fn handle_msg(opcode: u8, data: &[u8], player_id: u32, game_tx: &GameChannel, input_map: &InputMap) {
    let send = |msg| {
        let _ = game_tx.try_send((player_id, msg));
    };

    match PacketType::from_u8(opcode) {
        Some(PacketType::Spawn) => {
            if let Ok(data) = decode::<SpawnMessage>(data) {
                let name: String = censor(data.name.trim()).chars().take(MAX_NAME).collect();
                let name = if name.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    name
                };
                let token: String = data.token.chars().filter(char::is_ascii_hexdigit).take(64).collect();
                send(InternalGameMessages::Spawn { name, token });
            }
        }
        Some(PacketType::Move) => {
            if let (Ok(data), Some(input)) = (decode::<MoveMessage>(data), input_map.get(&player_id)) {
                input.set_move(data.dir.filter(|d| d.is_finite()));
            }
        }
        Some(PacketType::Aim) => {
            if let (Ok(data), Some(input)) = (decode::<AimMessage>(data), input_map.get(&player_id)) {
                input.set_aim(data.dir.filter(|d| d.is_finite()).unwrap_or(0.0));
            }
        }
        Some(PacketType::Attack) => {
            if let Ok(data) = decode::<AttackMessage>(data) {
                send(InternalGameMessages::Attack(data.attacking));
            }
        }
        Some(PacketType::ChatMessage) => {
            if let Ok(data) = decode::<ChatMessage>(data) {
                // Dev commands (ROAMER_DEV=1 only) skip the filter, which can
                // mangle numbers.
                let dev_command = data.message.starts_with('/') && std::env::var("ROAMER_DEV").as_deref() == Ok("1");
                let text = if dev_command {
                    data.message
                } else {
                    censor(&data.message)
                };
                let message: String = text.chars().take(MAX_CHAT).collect();
                if !message.trim().is_empty() {
                    send(InternalGameMessages::ChatMessage(message));
                }
            }
        }
        Some(PacketType::ChooseWeapon) => {
            if let Ok(data) = decode::<ChooseWeapon>(data) {
                send(InternalGameMessages::SetWeapon(data.weapon));
            }
        }
        Some(PacketType::UseItem) => {
            if let Ok(data) = decode::<UseItemMessage>(data) {
                send(InternalGameMessages::UseItem(data.item));
            }
        }
        Some(PacketType::ChooseUpgrade) => {
            if let Ok(data) = decode::<ChooseUpgradeMessage>(data) {
                send(InternalGameMessages::ChooseUpgrade {
                    is_item: data.is_item,
                    id:      data.id,
                });
            }
        }
        Some(PacketType::Shop) => {
            if let Ok(data) = decode::<ShopMessage>(data) {
                send(InternalGameMessages::Shop(data));
            }
        }
        Some(PacketType::Clan) => {
            if let Ok(mut data) = decode::<ClanMessage>(data) {
                data.name = censor(&data.name);
                send(InternalGameMessages::Clan(data));
            }
        }
        Some(PacketType::Chest) => {
            if let Ok(data) = decode::<ChestMessage>(data) {
                send(InternalGameMessages::Chest(data));
            }
        }
        Some(PacketType::Cave) => send(InternalGameMessages::Cave),
        Some(PacketType::Trade) => {
            if let Ok(data) = decode::<TradeMessage>(data) {
                send(InternalGameMessages::Trade(data));
            }
        }
        Some(PacketType::Arena) => {
            if let Ok(data) = decode::<ArenaMessage>(data) {
                send(InternalGameMessages::Arena(data));
            }
        }
        _ => {}
    }
}
