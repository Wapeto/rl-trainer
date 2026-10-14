//! What the user sees when the app cannot start.

use std::path::Path;

use rl_app::AppError;

pub fn failure_text(err: &AppError, logs: &Path) -> String {
    format!(
        "RL Trainer could not start: {err}.

Your data is safe: nothing was changed.          Details are in the log files in {}.",
        logs.display()
    )
}

/// Startup errors happen before any window exists, and release builds have no console.
#[cfg(windows)]
pub fn show_failure(text: &str) {
    use windows::core::{w, HSTRING};
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let text = HSTRING::from(text);
    // SAFETY: both strings are valid, NUL-terminated and outlive the call; no owner window.
    unsafe {
        MessageBoxW(None, &text, w!("RL Trainer"), MB_OK | MB_ICONERROR);
    }
}

#[cfg(not(windows))]
pub fn show_failure(text: &str) {
    eprintln!("{text}");
}

/// Logs panics (release builds have no console) and then runs the default hook.
pub fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(%info, "rl-trainer panicked");
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_app::StoreError;

    fn newer_schema() -> AppError {
        AppError::Store(StoreError::NewerSchema {
            found: 3,
            supported: 2,
        })
    }

    #[test]
    fn a_newer_database_is_explained_with_where_to_look() {
        let logs = Path::new("C:/Users/me/AppData/Local/rl-trainer/logs");
        let text = failure_text(&newer_schema(), logs);
        assert!(text.starts_with("RL Trainer could not start"), "{text}");
        assert!(text.contains("newer version"), "{text}");
        assert!(text.contains("nothing was changed"), "{text}");
        assert!(text.contains("rl-trainer/logs"), "{text}");
    }
}
