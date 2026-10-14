//! Quit and restart, from the tray menu or Settings: the open session is closed first.

use rl_app::App;
use tauri::{AppHandle, Manager};

use crate::launch::SHOW_WINDOW_ENV;

pub fn quit(app: &AppHandle) {
    leave(app, false);
}

pub fn restart(app: &AppHandle) {
    leave(app, true);
}

fn leave(app: &AppHandle, restart: bool) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(core) = handle.try_state::<App>() {
            let core = core.inner().clone();
            core.shutdown().await; // closes the open session first
        }
        if restart {
            // The new process inherits this; see `SHOW_WINDOW_ENV`.
            std::env::set_var(SHOW_WINDOW_ENV, "1");
            // Unlike `restart`, this runs the exit events, so the single-instance lock is freed
            // before the new process starts.
            handle.request_restart();
        } else {
            handle.exit(0);
        }
    });
}
