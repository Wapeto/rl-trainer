//! The desktop page calls `desktop(command, args)`, which runs `rl_app::desktop::dispatch` — the
//! same dispatcher the development server uses. Autostart needs the Tauri handle, so it stays
//! here. Errors are returned as readable strings.

use rl_app::App;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::UpdaterExt;

type CmdResult<T> = Result<T, String>;

#[tauri::command(rename_all = "snake_case")]
pub async fn desktop(app: State<'_, App>, command: String, args: Value) -> CmdResult<Value> {
    rl_app::desktop::dispatch(&app, &command, args).await
}

/// Settings → App → Quit: the same as the tray's Quit.
#[tauri::command(rename_all = "snake_case")]
pub fn quit_app(handle: AppHandle) {
    crate::lifecycle::quit(&handle);
}

/// Settings → App → Restart. A debug build rebuilds from its sources first, and stays up when
/// that fails.
#[tauri::command(rename_all = "snake_case")]
pub async fn restart_app(handle: AppHandle) -> CmdResult<()> {
    if let Some(root) = crate::dev_rebuild::workspace() {
        tauri::async_runtime::spawn_blocking(move || crate::dev_rebuild::rebuild(&root))
            .await
            .map_err(|e| e.to_string())??;
    }
    crate::lifecycle::restart(&handle);
    Ok(())
}

#[derive(Serialize)]
pub struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

/// The newer release, if any. A debug build never checks (there is nothing to install over).
#[tauri::command(rename_all = "snake_case")]
pub async fn check_update(handle: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    if cfg!(debug_assertions) {
        return Ok(None);
    }
    let update = handle
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version,
        notes: u.body,
    }))
}

/// Downloads and installs the newest release. On Windows the installer closes the app and
/// starts it again (passive mode), so recording stops cleanly between download and install.
#[tauri::command(rename_all = "snake_case")]
pub async fn install_update(handle: AppHandle, app: State<'_, App>) -> CmdResult<()> {
    let update = handle
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("There is no update to install")?;
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.inner().clone().shutdown().await; // closes the open session first
    if let Err(err) = update.install(bytes) {
        // Recording is stopped: start again on the current version.
        tracing::error!("update install failed: {err}");
        crate::lifecycle::restart(&handle);
        return Err(err.to_string());
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_autostart(handle: AppHandle) -> CmdResult<bool> {
    if cfg!(debug_assertions) {
        return Ok(false);
    }
    handle.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_autostart(handle: AppHandle, enabled: bool) -> CmdResult<()> {
    // A dev build would register its own path and start a stale binary at login.
    if cfg!(debug_assertions) {
        return Err("Start with Windows is off in dev builds".into());
    }
    let manager = handle.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_loads_the_built_desktop_page() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["build"]["frontendDist"], "../../../ui/dist/desktop");
        let window = &conf["app"]["windows"][0];
        assert_ne!(
            conf["plugins"]["updater"]["endpoints"][0],
            serde_json::Value::Null
        );
        assert_eq!(
            (window["minWidth"].as_i64(), window["minHeight"].as_i64()),
            (Some(1100), Some(700))
        );
    }
}
