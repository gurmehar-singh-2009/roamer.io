use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Resource;

use crate::CONFIG;

#[derive(Clone)]
pub struct AnimalSnapshot {
    pub id:          u32,
    pub x:           f32,
    pub y:           f32,
    pub animal_type: u8,
}

#[derive(Clone)]
pub struct ObjSnapshot {
    pub id:       u32,
    pub x:        f32,
    pub y:        f32,
    pub dir:      f32,
    pub scale:    f32,
    pub type_obj: u8,
}

#[derive(Resource, Default)]
pub struct PlayerKnowledge {
    /// Tracks the IDs of animals each player has already been sent
    pub known_animals: HashMap<u32, HashSet<u32>>,
    /// Tracks the IDs of static objects each player has already been sent
    pub known_objects: HashMap<u32, HashSet<u32>>,
}

impl PlayerKnowledge {
    pub fn add_player(&mut self, player_id: u32) {
        self.known_animals.insert(player_id, HashSet::new());
        self.known_objects.insert(player_id, HashSet::new());
    }

    pub fn remove_player(&mut self, player_id: u32) {
        self.known_animals.remove(&player_id);
        self.known_objects.remove(&player_id);
    }

    /// Filters an iterator of objects, returning only the ones within the
    /// player's view radius that the player DOES NOT already know about.
    pub fn drain_new_objects<'a>(
        &mut self,
        player_id: u32,
        objects: impl Iterator<Item = &'a ObjSnapshot>,
        px: f32,
        py: f32,
    ) -> Vec<ObjSnapshot> {
        let mut new_objects = Vec::new();

        if let Some(known) = self.known_objects.get_mut(&player_id) {
            for obj in objects {
                let dx = obj.x - px;
                let dy = obj.y - py;

                if dx * dx + dy * dy <= CONFIG.players.view_radius_sq
                    && known.insert(obj.id) {
                        new_objects.push(obj.clone());
                    }
            }
        }

        new_objects
    }
}
