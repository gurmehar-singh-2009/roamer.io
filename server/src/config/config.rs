use bevy_ecs::resource::Resource;
use config::File;
use serde::Deserialize;

#[derive(Debug, Deserialize, Resource)]
/// Struct that represents the top level config.
pub struct Config {
    pub map:      MapConfig,
    pub players:  PlayerConfig,
    pub entities: EntityConfig,
    pub animals:  AnimalConfig,
    pub game:     GameConfig,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the game server configuration.
pub struct GameConfig {
    pub hkdf_key:          String,
    pub port:              u16,
    pub idle_timeout_mins: u64,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the map configuration entries.
pub struct MapConfig {
    pub size: u16,

    pub snow_start: u16,
    pub snow_end:   u16,

    pub grassland_start: u16,
    pub grassland_end:   u16,

    pub desert_start: u16,
    pub desert_end:   u16,

    pub lava_start: u16,
    pub lava_end:   u16,

    pub ocean_start_x: u16,
    pub ocean_end_x:   u16,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the player configuration entries.
pub struct PlayerConfig {
    pub update_dt:           f32,
    pub player_max_speed:    f32,
    pub player_acceleration: f32,
    pub player_friction:     f32,
    pub view_radius:         f32,
    pub view_radius_sq:      f32,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the game object configuration entries.
pub struct EntityConfig {
    pub trees_per_chunk:  u8,
    pub bushes_per_chunk: u8,
    pub stones_per_chunk: u8,
    pub gold_per_chunk:   u8,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the animal configuration entries.
pub struct AnimalConfig {
    pub max_fish_alive:         u16,
    pub max_wolf:               u8,
    pub fish_turn_factor:       f32,
    pub fish_visual_range:      f32,
    pub fish_visual_range_sq:   f32,
    pub fish_protect_range_sq:  f32,
    pub fish_separation_factor: f32,
    pub fish_alignment_factor:  f32,
    pub fish_cohesion_factor:   f32,
    pub fish_wander_strength:   f32,
}

/// Loads the config file and attempts to parse it.
pub fn load_config() -> Result<Config, config::ConfigError> {
    let config = config::Config::builder()
        .add_source(File::with_name("src/config/config.toml"))
        .build()?;
    config.try_deserialize()
}
