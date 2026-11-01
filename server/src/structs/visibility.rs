//! What each player has been told about.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Resource;
use shared::to_client::ObjectTO;

/// Tracks which objects each player has already been sent. Kept across
/// deaths (the client keeps its objects too) and dropped on disconnect.
#[derive(Resource, Default)]
pub struct PlayerKnowledge {
    /// Object entity indices each player has been sent.
    pub known_objects: HashMap<u32, HashSet<u32>>,
}

impl PlayerKnowledge {
    /// Starts tracking a player (keeps what they already know).
    pub fn add_player(&mut self, player_id: u32) {
        self.known_objects.entry(player_id).or_default();
    }

    /// Forgets a destroyed object for every player and returns the players
    /// that knew about it (they need to be told to remove it).
    pub fn forget_object(&mut self, object_id: u32) -> Vec<u32> {
        self.known_objects
            .iter_mut()
            .filter_map(|(player_id, known)| known.remove(&object_id).then_some(*player_id))
            .collect()
    }

    /// Returns the players that know about an object.
    pub fn players_knowing(&self, object_id: u32) -> impl Iterator<Item = u32> + '_ {
        self.known_objects
            .iter()
            .filter(move |(_, known)| known.contains(&object_id))
            .map(|(player_id, _)| *player_id)
    }

    /// Stops tracking a player.
    pub fn remove_player(&mut self, player_id: u32) {
        self.known_objects.remove(&player_id);
    }

    /// Returns the objects the player can see that they haven't been sent
    /// yet, and marks them as sent.
    pub fn drain_new_objects<'a>(
        &mut self,
        player_id: u32,
        objects: impl Iterator<Item = &'a ObjectTO>,
        visible: impl Fn(&ObjectTO) -> bool,
    ) -> Vec<ObjectTO> {
        let Some(known) = self.known_objects.get_mut(&player_id) else {
            return Vec::new();
        };
        let mut new_objects = Vec::new();
        for obj in objects {
            if !known.contains(&obj.id) && visible(obj) {
                known.insert(obj.id);
                new_objects.push(obj.clone());
            }
        }
        new_objects
    }
}
