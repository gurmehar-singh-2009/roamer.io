#[cfg(feature = "web")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "web")]
use wasm_bindgen::prelude::*;

#[allow(unused_imports)]
use borsh_derive::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "web")]
// use crate::structs::server::GameObject;
use crate::structs::server::{Aim, ChooseWeapon, GameObject, Move, Player, SetWeapons};

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize, BorshDeserialize)]
pub struct JsPlayer {
    /// Display name.
    name: String,
    /// Player id.
    id: u32,
    /// X position.
    x: f32,
    /// Y position.
    y: f32,
    /// Movement direction in radians, `None` when standing still.
    move_dir: Option<f32>,
    /// Aim direction in radians.
    aim: f32,
    /// Held weapon.
    weapon_index: Option<u8>,
}

#[cfg(feature = "web")]
impl JsPlayer {
    #[must_use]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub const fn get_id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn get_x(&self) -> f32 {
        self.x
    }

    #[must_use]
    pub const fn get_y(&self) -> f32 {
        self.y
    }

    #[must_use]
    pub const fn get_move_dir(&self) -> Option<f32> {
        self.move_dir
    }

    #[must_use]
    pub const fn get_aim(&self) -> f32 {
        self.aim
    }

    #[must_use]
    pub const fn get_weapon_index(&self) -> Option<u8> {
        self.weapon_index
    }
}

#[cfg(feature = "web")]
impl From<Player> for JsPlayer {
    fn from(value: Player) -> Self {
        Self {
            name: value.name,
            id: value.id,
            x: value.x,
            y: value.y,
            move_dir: value.move_dir,
            aim: value.aim,
            weapon_index: value.weapon_index,
        }
    }
}

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize, BorshDeserialize)]
pub struct JsGameObject {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[cfg(feature = "web")]
impl JsGameObject {
    #[must_use]
    pub const fn get_id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn get_x(&self) -> f32 {
        self.x
    }

    #[must_use]
    pub const fn get_y(&self) -> f32 {
        self.y
    }

    #[must_use]
    pub const fn get_scale(&self) -> f32 {
        self.scale
    }
}

#[cfg(feature = "web")]
impl From<GameObject> for JsGameObject {
    fn from(value: GameObject) -> Self {
        Self {
            id: value.id,
            x: value.x,
            y: value.y,
            scale: value.scale,
        }
    }
}

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize)]
pub struct JsHitEvent {
    /// Id of the entity that attacked.
    entity: u32,
}

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize)]
pub struct JsMove {
    /// Movement direction in radians, `None` when standing still.
    dir: Option<f32>,
}

#[cfg(feature = "web")]
impl From<Move> for JsMove {
    fn from(value: Move) -> Self {
        Self { dir: value.dir }
    }
}

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize)]
pub struct JsAim {
    /// Aim direction in radians.
    dir: Option<f32>,
}

#[cfg(feature = "web")]
impl From<Aim> for JsAim {
    fn from(value: Aim) -> Self {
        Self { dir: value.dir }
    }
}

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize)]
pub struct JsSetWeapons {
    /// Weapon ids.
    weapons: Vec<u8>,
}

#[cfg(feature = "web")]
impl From<SetWeapons> for JsSetWeapons {
    fn from(value: SetWeapons) -> Self {
        Self { weapons: value.weapons }
    }
}

#[cfg(feature = "web")]
impl JsSetWeapons {
    #[must_use]
    pub fn get_set_weapons(&self) -> Vec<u8> {
        self.weapons.clone()
    }
}

#[cfg(feature = "web")]
#[wasm_bindgen]
#[derive(BorshSerialize, Deserialize, Serialize)]
pub struct JsChooseWeapon {
    /// Chosen weapon id.
    weapon: u8,
}

#[cfg(feature = "web")]
impl From<ChooseWeapon> for JsChooseWeapon {
    fn from(value: ChooseWeapon) -> Self {
        Self { weapon: value.weapon }
    }
}

#[cfg(feature = "web")]
impl JsChooseWeapon {
    #[must_use]
    pub const fn get_weapon(&self) -> u8 {
        self.weapon
    }
}
