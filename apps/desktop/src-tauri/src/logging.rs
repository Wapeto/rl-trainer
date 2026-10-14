//! Daily rolling log files in %LOCALAPPDATA%\rl-trainer\logs (14 kept).

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub fn init(dir: &Path) -> Result<WorkerGuard, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("rl-trainer")
        .filename_suffix("log")
        .max_log_files(14)
        .build(dir)
        .map_err(|e| e.to_string())?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .map_err(|e| e.to_string())?;
    Ok(guard)
}
