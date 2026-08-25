use borsh_derive::{BorshDeserialize, BorshSerialize};
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

use crate::to_client::{AddObjectData, AddPlayerData, SetWeaponsData};

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct SpawnMessage {
    pub name: String,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct MoveMessage {
    pub dir: Option<f32>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct AimMessage {
    pub dir: Option<f32>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct HitMessage {}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct ChatMessage {
    pub message: String,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct ChooseWeapon {
    pub weapon: u8,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub enum ClientMessages {
    AddPlayer(AddPlayerData),
    SpawnMessage,
    MoveMessage,
    AimMessage,
    ChatMessage,
    HitMessage,
    ChooseWeapon,
    AddObject(AddObjectData),
    SetWeapons(SetWeaponsData),
}
