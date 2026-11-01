#[cfg(feature = "web")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "web")]
#[allow(unused_imports)]
use wasm_bindgen::prelude::*;

use borsh_derive::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "web")]
use crate::structs::client::{JsGameObject, JsPlayer};

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(bevy_ecs::component::Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct Player {
    pub name: String,
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub move_dir: Option<f32>,
    pub aim: f32,
    pub vx: f32,
    pub vy: f32,
    pub attacked: bool,
    pub health: f32,
    pub weapon_index: Option<u8>,
}

impl Player {
    #[must_use]
    pub const fn new(name: String, id: u32) -> Self {
        Self {
            name,
            id,
            x: 10000.,
            y: 10000.,
            vx: 0.0, 
            vy: 0.0,
            move_dir: None,
            attacked: false,
            aim: 0.0,
            health: 100.0,
            weapon_index: Some(0)
        }
    }
}

#[cfg(feature = "web")]
impl From<JsPlayer> for Player {
    fn from(value: JsPlayer) -> Self {
        Self {
            name: value.get_name(),
            id: value.get_id(),
            x: value.get_x(),
            y: value.get_y(),
            move_dir: value.get_move_dir(),
            aim: value.get_aim(),
            vx: 0.,
            vy: 0.,
            attacked: false,
            health: 100.0,
            weapon_index: value.get_weapon_index(),
        }
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "server", derive(bevy_ecs::component::Component))]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct GameObject {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[cfg(feature = "web")]
impl From<JsGameObject> for GameObject {
    fn from(value: JsGameObject) -> Self {
        Self {
            id: value.get_id(),
            x: value.get_x(),
            y: value.get_y(),
            scale: value.get_scale(),
        }
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct Move {
    pub dir: Option<f32>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct Aim {
    pub dir: Option<f32>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct SetWeapons {
    pub weapons: Vec<u8>,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct HitEvent {}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct ChatMessage {
    pub message: String
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct InternalChatMessage {
    pub id: u32,
    pub message: String
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
#[cfg_attr(feature = "web", derive(Serialize, Deserialize))]
pub struct ChooseWeapon {
    pub weapon: u8,
}
