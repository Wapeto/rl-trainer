#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rl_app::engine::AppSnapshot;
use rl_app::game_watch::GameProcess;
use rl_app::paths::AppPaths;
use rl_app::testing::FakePlatform;
use rl_app::{App, AppConfig};
use tempfile::TempDir;

pub fn ini_text(rate: u32, port: u16) -> String {
    format!("[TAGame.MatchStatsExporter_TA]\r\nPort={port}\r\nWebPort=49124\r\nPacketSendRate={rate}\r\n")
}

/// Creates `<dir>/rocketleague` with the ini and returns the game's exe path.
pub fn install(dir: &Path, ini: &str) -> PathBuf {
    let root = dir.join("rocketleague");
    let config = root.join("TAGame").join("Config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("DefaultStatsAPI.ini"), ini).unwrap();
    root.join("Binaries").join("Win64").join("RocketLeague.exe")
}

pub fn ini_path(dir: &Path) -> PathBuf {
    dir.join("rocketleague")
        .join("TAGame")
        .join("Config")
        .join("DefaultStatsAPI.ini")
}

pub fn game(exe: &Path) -> Option<GameProcess> {
    Some(GameProcess {
        exe: Some(exe.to_path_buf()),
    })
}

pub fn port_of(addr: &str) -> u16 {
    addr.rsplit(':').next().unwrap().parse().unwrap()
}

pub fn config(dir: &TempDir, stats_addr: Option<String>) -> AppConfig {
    AppConfig {
        stats_addr_override: stats_addr,
        poll_every: Duration::from_millis(20),
        ..AppConfig::new(AppPaths::under(&dir.path().join("app")))
    }
}

pub async fn start(dir: &TempDir, platform: Arc<FakePlatform>, stats_addr: Option<String>) -> App {
    App::start(config(dir, stats_addr), platform).await.unwrap()
}

pub async fn wait_for(app: &App, what: &str, f: impl FnMut(&AppSnapshot) -> bool) -> AppSnapshot {
    let mut rx = app.snapshots();
    let snapshot = tokio::time::timeout(Duration::from_secs(10), rx.wait_for(f))
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
        .unwrap()
        .clone();
    snapshot
}
