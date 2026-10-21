//! Headless application layer: everything rl-trainer.exe does except windows and the tray.

pub mod app;
pub mod backup;
#[cfg(test)]
mod coach_samples;
pub mod db;
pub mod desktop;
pub mod devtools;
pub mod elevated;
pub mod engine;
pub mod game_watch;
mod library;
pub mod pairing;
pub mod paths;
pub mod phone;
pub mod platform;
#[cfg(test)]
mod progress_samples;
pub mod replays;
pub mod report;
pub mod settings;
pub mod testing;
mod ts_export;
pub mod usage;

pub use app::{App, AppConfig, AppError, StartupReport};

/// Where releases (and so updates) are published.
pub const RELEASES_URL: &str = "https://github.com/Wapeto/rl-trainer/releases";
pub use rl_core::store::{ProfileRow, StoreError};

#[cfg(test)]
#[path = "../build_support.rs"]
mod build_support;
