//! System tray: status line, open, fix the Stats API, quit.

use rl_app::engine::AppSnapshot;
use rl_app::game_watch::ini::StatsApiStatus;
use rl_app::App;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "main";

struct TrayItems {
    status: MenuItem<Wry>,
    fix: MenuItem<Wry>,
}

pub fn needs_fix(s: &AppSnapshot) -> bool {
    matches!(s.stats_api, StatsApiStatus::Disabled { .. })
}

pub fn status_line(s: &AppSnapshot) -> String {
    let game = if s.game_running {
        "running"
    } else {
        "not running"
    };
    let api = match &s.stats_api {
        StatsApiStatus::Unknown => "not checked",
        StatsApiStatus::Ok { .. } => "OK",
        StatsApiStatus::Disabled { .. } => "DISABLED",
        StatsApiStatus::FileMissing { .. } | StatsApiStatus::Unreadable { .. } => "ini problem",
        StatsApiStatus::InstallNotFound => "install not found",
    };
    format!(
        "Game: {game} · Stats API: {api} · Phones: {}",
        s.phone_clients
    )
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn fix(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(core) = handle.try_state::<App>() {
            let core = core.inner().clone();
            if let Err(err) = core.fix_stats_api().await {
                tracing::warn!(error = %err, "Stats API fix from the tray failed");
                show_main(&handle);
            }
        }
    });
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "Starting…", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open RL Trainer", true, None::<&str>)?;
    let fix_item = MenuItem::with_id(app, "fix", "Fix Stats API…", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&status, &separator, &open, &fix_item, &quit_item])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("RL Trainer")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "fix" => fix(app),
            "quit" => crate::lifecycle::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(TrayItems {
        status,
        fix: fix_item,
    });
    Ok(())
}

pub fn update(app: &AppHandle, s: &AppSnapshot) {
    let line = status_line(s);
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.status.set_text(&line);
        let _ = items.fix.set_enabled(needs_fix(s));
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("RL Trainer — {line}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_app::game_watch::ini::StatsApiStatus;
    use std::path::PathBuf;

    #[test]
    fn status_line_summarises_game_stats_api_and_phones() {
        let mut s = AppSnapshot::default();
        assert_eq!(
            status_line(&s),
            "Game: not running · Stats API: not checked · Phones: 0"
        );
        s.game_running = true;
        s.stats_api = StatsApiStatus::Ok {
            rate: 10.0,
            port: 49123,
            web_port: None,
        };
        s.phone_clients = 1;
        assert_eq!(status_line(&s), "Game: running · Stats API: OK · Phones: 1");
        assert!(!needs_fix(&s));
        s.stats_api = StatsApiStatus::Disabled {
            ini: PathBuf::from("C:/x.ini"),
        };
        assert_eq!(
            status_line(&s),
            "Game: running · Stats API: DISABLED · Phones: 1"
        );
        assert!(needs_fix(&s));
    }
}
