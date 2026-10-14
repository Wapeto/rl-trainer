//! How rl-trainer.exe was launched: elevated helper, autostart (minimised) or normal.

pub const MINIMIZED_FLAG: &str = "--minimized";
/// Set by a restart: the new process gets the old one's arguments, an autostart's `--minimized`
/// included, but the user asked for it from the window and expects the window back.
pub const SHOW_WINDOW_ENV: &str = "RL_TRAINER_SHOW_WINDOW";

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// `rl-trainer.exe --elevated <command> ...`: run the command and exit, no window.
    Elevated(Vec<String>),
    Normal {
        minimized: bool,
    },
}

pub fn mode(args: &[String], show_window: bool) -> Mode {
    match args.split_first() {
        Some((first, rest)) if first == "--elevated" => Mode::Elevated(rest.to_vec()),
        _ => Mode::Normal {
            minimized: !show_window && args.iter().any(|a| a == MINIMIZED_FLAG),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn a_restart_shows_the_window_even_after_an_autostart() {
        assert_eq!(
            mode(&args(&["--minimized"]), true),
            Mode::Normal { minimized: false }
        );
    }

    #[test]
    fn recognises_each_launch_mode() {
        assert_eq!(mode(&args(&[]), false), Mode::Normal { minimized: false });
        assert_eq!(
            mode(&args(&["--minimized"]), false),
            Mode::Normal { minimized: true }
        );
        assert_eq!(
            mode(
                &args(&["--elevated", "fix-stats-api", "--ini", "C:/x"]),
                false
            ),
            Mode::Elevated(args(&["fix-stats-api", "--ini", "C:/x"]))
        );
        assert_eq!(
            mode(&args(&["--minimized", "--elevated"]), false),
            Mode::Normal { minimized: true },
            "--elevated only counts as the first argument"
        );
    }
}
