#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dev_rebuild;
mod launch;
mod lifecycle;
mod logging;
mod startup;
mod tray;

use std::sync::Arc;

use rl_app::paths::AppPaths;
use rl_app::platform::WindowsPlatform;
use rl_app::{App, AppConfig};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let show_window = std::env::var_os(launch::SHOW_WINDOW_ENV).is_some();
    std::env::remove_var(launch::SHOW_WINDOW_ENV);
    match launch::mode(&args, show_window) {
        launch::Mode::Elevated(command) => std::process::exit(rl_app::elevated::run(&command)),
        launch::Mode::Normal { minimized } => run(minimized),
    }
}

fn run(minimized: bool) {
    let Some(root) = AppPaths::default_root() else {
        eprintln!("rl-trainer: LOCALAPPDATA is not set");
        std::process::exit(1);
    };
    let paths = AppPaths::under(&root);
    let _log_guard = match logging::init(&paths.logs) {
        Ok(guard) => Some(guard),
        Err(err) => {
            eprintln!("rl-trainer: logging disabled: {err}");
            None
        }
    };
    startup::log_panics();
    let logs = paths.logs.clone();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![launch::MINIMIZED_FLAG]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::desktop,
            commands::get_autostart,
            commands::set_autostart,
            commands::quit_app,
            commands::restart_app,
            commands::check_update,
            commands::install_update,
        ])
        .on_window_event(|window, event| {
            // Closing the window keeps recording in the tray; Quit is in the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            let config = AppConfig {
                dev_db: AppPaths::dev_db(),
                ..AppConfig::new(paths.clone())
            };
            // No window exists yet, and the database open and the daily backup run on blocking
            // threads inside `App::start`: the main thread only waits here.
            let started =
                tauri::async_runtime::block_on(App::start(config, Arc::new(WindowsPlatform)));
            // A setup error would only panic inside Tauri, invisibly: explain it and exit instead.
            let core = match started {
                Ok(core) => core,
                Err(err) => {
                    tracing::error!(error = %err, "startup failed");
                    startup::show_failure(&startup::failure_text(&err, &logs));
                    std::process::exit(1);
                }
            };
            tracing::info!(report = ?core.startup_report(), "started");
            tray::create(app.handle())?;
            forward_live(app.handle().clone(), core.clone());
            app.manage(core);
            if minimized {
                if let Some(window) = app.get_webview_window("main") {
                    window.hide()?;
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(err) = result {
        tracing::error!(error = %err, "the app stopped with an error");
        eprintln!("rl-trainer: {err}");
        std::process::exit(1);
    }
}

/// Pushes every snapshot to the desktop page (`live` event) and the tray.
fn forward_live(handle: tauri::AppHandle, core: App) {
    tauri::async_runtime::spawn(async move {
        let mut rx = core.snapshots();
        loop {
            let snapshot = rx.borrow_and_update().clone();
            if let Err(err) = handle.emit("live", &snapshot) {
                tracing::warn!(error = %err, "could not send the live event");
            }
            tray::update(&handle, &snapshot);
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
}
