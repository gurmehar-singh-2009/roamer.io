// Doesn't cause any problems here.
#[allow(clippy::module_inception)]
mod config;

pub use config::{load_config, Config};
