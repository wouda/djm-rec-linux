//! djm-common: Shared types, configuration, protocols, and mixer profiles for djm-box.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod config;
pub mod protocol;
pub mod profile;
