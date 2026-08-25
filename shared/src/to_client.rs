use bevy_ecs::component::Component;
#[cfg(feature = "web")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "web")]
#[allow(unused_imports)]
use wasm_bindgen::prelude::*;

use borsh_derive::{BorshDeserialize, BorshSerialize};

use crate::structs::server::Player as ServerPlayer;

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct PlayerTO {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub aim: f32,
    pub health: f32,
    pub weapon_index: Option<u8>,
}

impl From<ServerPlayer> for PlayerTO {
    fn from(value: ServerPlayer) -> Self {
        Self {
            id: value.id,
            name: value.name,
            x: value.x,
            y: value.y,
            aim: value.aim,
            health: value.health,
            weapon_index: value.weapon_index,
        }
    }
}

// DO NOT RENAME OR REORDER THE FOLLOWING ENUM!
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
#[repr(u8)]
#[borsh(use_discriminant = true)]
pub enum AnimalType {
    Wolf = 0,
    Fish = 1,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct AnimalTO {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub animal_type: u8,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct HitEventTO {
    pub entity_id: u32,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct ObjectTO {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub dir: f32,
    pub scale: f32,
    pub type_obj: u8,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct AddPlayerData {
    pub is_mine: bool,
    pub data: PlayerTO,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct UpdatePlayerData {
    pub players: Vec<PlayerTO>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct AddObjectData {
    pub objects: Vec<ObjectTO>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct AddAnimalData {
    pub animals: Vec<AnimalTO>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct UpdateHealthData {
    pub id: u32,
    pub new_health: f32,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct ObjectHitAnimData {
    pub id: u32,
    pub dir: f32,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct SetResourceData {
    pub wood: u32,
    pub stone: u32,
    pub food: u32,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct SetWeaponsData {
    pub weapons: Vec<u8>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct SetUpgradesData {
    pub upgrades: Vec<u8>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
#[repr(u8)]
#[borsh(use_discriminant = true)]
pub enum Weapons {
    Fists = 0,
    Hammer = 1,
    Sword = 2,
    Hoe = 3,
    Knife = 4,
}