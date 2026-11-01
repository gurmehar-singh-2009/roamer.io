// Doesn't cause any problems here.
#[allow(clippy::module_inception)]
/// Config file loading.
mod config;

pub use config::{load_config, Config};
