//! Where the app keeps its database, backups and logs.

use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub backups: PathBuf,
    pub logs: PathBuf,
}

impl AppPaths {
    /// The installed app's layout: `<root>\rl-trainer.db`, `backups\`, `logs\`.
    pub fn under(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            db: root.join("rl-trainer.db"),
            backups: root.join("backups"),
            logs: root.join("logs"),
        }
    }

    /// Layout for a database chosen on the command line (`rl-replay serve`).
    pub fn for_db(db: &Path) -> Self {
        let root = db.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        let stem = db
            .file_stem()
            .map_or_else(|| "rl-trainer".into(), |s| s.to_string_lossy().into_owned());
        Self {
            backups: root.join(format!("{stem}-backups")),
            logs: root.join(format!("{stem}-logs")),
            db: db.to_path_buf(),
            root,
        }
    }

    /// `%LOCALAPPDATA%\rl-trainer`.
    pub fn default_root() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("rl-trainer"))
    }

    /// The Plan 1 live-test database, imported once on first start.
    pub fn dev_db() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("rl-trainer-dev.db"))
    }

    pub fn ensure_dirs(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.backups)?;
        std::fs::create_dir_all(&self.logs)?;
        if let Some(parent) = self.db.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_layout_lives_under_one_root() {
        let p = AppPaths::under(Path::new("C:/data/rl-trainer"));
        assert_eq!(p.db, Path::new("C:/data/rl-trainer/rl-trainer.db"));
        assert_eq!(p.backups, Path::new("C:/data/rl-trainer/backups"));
        assert_eq!(p.logs, Path::new("C:/data/rl-trainer/logs"));
    }

    #[test]
    fn a_dev_database_gets_sibling_folders() {
        let p = AppPaths::for_db(Path::new("D:/tmp/dev.db"));
        assert_eq!(p.db, Path::new("D:/tmp/dev.db"));
        assert_eq!(p.backups, Path::new("D:/tmp/dev-backups"));
        assert_eq!(p.logs, Path::new("D:/tmp/dev-logs"));
    }
}
