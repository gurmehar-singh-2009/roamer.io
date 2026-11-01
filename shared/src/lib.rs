//! Network messages shared by the server and the web client (through the WASM
//! parser). Every packet is one opcode byte followed by a borsh body.
//!
//! `src/objects/` and `src/structs/` are leftovers from the old protocol and
//! are no longer compiled.

pub mod to_client;
pub mod to_server;

/// Declares a network message struct with the serialization derives both
/// sides need.
macro_rules! message {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$fmeta:meta])* pub $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        // Messages with only integer fields could also be `Eq`; not worth a
        // second macro arm.
        #[allow(clippy::derive_partial_eq_without_eq)]
        #[derive(Debug, Clone, Default, PartialEq, borsh_derive::BorshSerialize, borsh_derive::BorshDeserialize)]
        #[cfg_attr(feature = "web", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty),*
        }
    };
}
pub(crate) use message;

/// Every packet opcode. Client and server packets share one number space.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    // --- client -> server ---
    /// `to_server::SpawnMessage`
    Spawn = 1,
    /// `to_server::MoveMessage`
    Move = 2,
    /// `to_server::AimMessage`
    Aim = 5,
    /// `to_server::AttackMessage`
    Attack = 6,
    /// `to_server::ChatMessage`
    ChatMessage = 12,
    /// `to_server::ChooseWeapon`
    ChooseWeapon = 15,
    /// `to_server::UseItemMessage`
    UseItem = 17,
    /// `to_server::ChooseUpgradeMessage`
    ChooseUpgrade = 31,
    /// `to_server::ShopMessage`
    Shop = 32,
    /// `to_server::ClanMessage`
    Clan = 33,
    /// `to_server::ChestMessage`
    Chest = 34,
    /// `to_server::TradeMessage`
    Trade = 35,
    /// `to_server::ArenaMessage`
    Arena = 36,
    /// `to_server::CaveMessage`
    Cave = 37,

    // --- server -> client ---
    /// `to_client::AddPlayerData`
    AddPlayer = 101,
    /// Raw little-endian player id.
    RemovePlayer = 102,
    /// `to_client::UpdatePlayerData`
    UpdatePlayers = 103,
    /// `to_client::AnimalsData`
    Animals = 104,
    /// `to_client::HitEventTO`
    HitEvent = 106,
    /// `to_client::AddObjectData`
    AddObject = 107,
    /// `to_client::UpdateHealthData`
    UpdateHealth = 108,
    /// `to_client::LoadoutData`
    Loadout = 109,
    /// `to_client::ObjectHitAnimData`
    ObjectHitAnim = 110,
    /// `to_client::SetResourceData`
    SetResource = 111,
    /// `to_client::DeathData`
    Death = 112,
    /// `to_client::ChatData`
    Chat = 113,
    /// `to_client::UpgradesData`
    Upgrades = 114,
    /// `to_client::LeaderboardData`
    Leaderboard = 116,
    /// `to_client::UpdateObjectHealthData`
    UpdateObjectHealth = 119,
    /// `to_client::RemoveObjectData`
    RemoveObject = 120,
    /// `to_client::AddProjectileData`
    AddProjectile = 121,
    /// `to_client::RemoveProjectileData`
    RemoveProjectile = 122,
    /// `to_client::AgeData`
    Age = 123,
    /// `to_client::NoticeData`
    Notice = 124,
    /// `to_client::ExplosionData`
    Explosion = 125,
    /// `to_client::ClanStateData`
    ClanState = 126,
    /// `to_client::ShopData`
    ShopState = 127,
    /// `to_client::QuestData`
    Quest = 128,
    /// `to_client::WorldEventData`
    WorldEvent = 129,
    /// `to_client::ChestData`
    ChestState = 130,
    /// `to_client::MinimapData`
    Minimap = 131,
    /// `to_client::SpawnedData`
    Spawned = 132,
    /// `to_client::ArenaStateData`
    ArenaState = 133,
    /// `to_client::CaveCellsData`
    CaveCells = 134,
}

impl PacketType {
    /// Every opcode.
    pub const ALL: [Self; 44] = [
        Self::Spawn,
        Self::Move,
        Self::Aim,
        Self::Attack,
        Self::ChatMessage,
        Self::ChooseWeapon,
        Self::UseItem,
        Self::ChooseUpgrade,
        Self::Shop,
        Self::Clan,
        Self::Chest,
        Self::Trade,
        Self::Arena,
        Self::Cave,
        Self::AddPlayer,
        Self::RemovePlayer,
        Self::UpdatePlayers,
        Self::Animals,
        Self::HitEvent,
        Self::AddObject,
        Self::UpdateHealth,
        Self::Loadout,
        Self::ObjectHitAnim,
        Self::SetResource,
        Self::Death,
        Self::Chat,
        Self::Upgrades,
        Self::Leaderboard,
        Self::UpdateObjectHealth,
        Self::RemoveObject,
        Self::AddProjectile,
        Self::RemoveProjectile,
        Self::Age,
        Self::Notice,
        Self::Explosion,
        Self::ClanState,
        Self::ShopState,
        Self::Quest,
        Self::WorldEvent,
        Self::ChestState,
        Self::Minimap,
        Self::Spawned,
        Self::ArenaState,
        Self::CaveCells,
    ];

    /// The packet type with this opcode.
    #[must_use]
    pub fn from_u8(val: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|p| *p as u8 == val)
    }
}

#[cfg(test)]
mod tests {
    use super::PacketType;

    #[test]
    fn opcodes_round_trip_and_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for p in PacketType::ALL {
            assert!(seen.insert(p as u8), "duplicate opcode {}", p as u8);
            assert_eq!(PacketType::from_u8(p as u8), Some(p));
        }
    }
}
