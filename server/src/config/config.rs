use bevy_ecs::resource::Resource;
use config::File;
use serde::Deserialize;

#[derive(Debug, Deserialize, Resource)]
/// Struct that represents the top level config.
pub struct Config {
    /// Map size and biome bounds.
    pub map:      MapConfig,
    /// Player movement and view settings.
    pub players:  PlayerConfig,
    /// Natural object spawn counts.
    pub entities: EntityConfig,
    /// Animal counts and fish flocking settings.
    pub animals:  AnimalConfig,
    /// Network and session settings.
    pub game:     GameConfig,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the game server configuration.
pub struct GameConfig {
    /// Info string used when deriving session keys.
    pub hkdf_key:          String,
    /// WebTransport port.
    pub port:              u16,
    /// Seconds before a silent connection is dropped.
    pub idle_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the map configuration entries.
pub struct MapConfig {
    /// Width and height of the square map.
    pub size: u16,

    /// Top of the snow biome (y).
    pub snow_start: u16,
    /// Bottom of the snow biome (y).
    pub snow_end:   u16,

    /// Top of the grassland biome (y).
    pub grassland_start: u16,
    /// Bottom of the grassland biome (y).
    pub grassland_end:   u16,

    /// Top of the desert biome (y).
    pub desert_start: u16,
    /// Bottom of the desert biome (y).
    pub desert_end:   u16,

    /// Top of the lava biome (y).
    pub lava_start: u16,
    /// Bottom of the lava biome (y).
    pub lava_end:   u16,

    /// Left edge of the ocean (x).
    pub ocean_start_x: u16,
    /// Right edge of the ocean (x).
    pub ocean_end_x:   u16,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the player configuration entries.
pub struct PlayerConfig {
    /// Movement time step per tick.
    pub update_dt:           f32,
    /// Top walking speed.
    pub player_max_speed:    f32,
    /// Acceleration while moving.
    pub player_acceleration: f32,
    /// Deceleration when not moving.
    pub player_friction:     f32,
    /// How far a player can see.
    pub view_radius:         f32,
    /// `view_radius` squared.
    pub view_radius_sq:      f32,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the game object configuration entries.
// Field names match the keys in config.toml.
#[allow(clippy::struct_field_names)]
pub struct EntityConfig {
    /// Trees per map chunk.
    pub trees_per_chunk:  u8,
    /// Bushes per map chunk.
    pub bushes_per_chunk: u8,
    /// Stones per map chunk.
    pub stones_per_chunk: u8,
    /// Gold per map chunk.
    pub gold_per_chunk:   u8,
}

#[derive(Debug, Deserialize)]
/// Struct that represents the animal configuration entries.
pub struct AnimalConfig {
    /// Number of fish spawned.
    pub max_fish_alive:         u16,
    /// Number of wolves spawned.
    pub max_wolf:               u8,
    /// How hard fish turn back at the ocean edge.
    pub fish_turn_factor:       f32,
    /// How far a fish looks for neighbours.
    pub fish_visual_range:      f32,
    /// `fish_visual_range` squared.
    pub fish_visual_range_sq:   f32,
    /// Squared distance under which fish push apart.
    pub fish_protect_range_sq:  f32,
    /// Strength of the push apart.
    pub fish_separation_factor: f32,
    /// How much fish match their neighbours' velocity.
    pub fish_alignment_factor:  f32,
    /// How much fish steer to their neighbours' center.
    pub fish_cohesion_factor:   f32,
    /// How much fish wander randomly.
    pub fish_wander_strength:   f32,
}

/// Loads the config file and attempts to parse it.
pub fn load_config() -> Result<Config, config::ConfigError> {
    let config = config::Config::builder()
        // ROAMER_CONFIG points a release build at its config file.
        .add_source(File::with_name(
            &std::env::var("ROAMER_CONFIG").unwrap_or_else(|_| "src/config/config.toml".into()),
        ))
        .build()?;
    config.try_deserialize()
}
