//! Database backups: daily snapshots (keep 7), pre-migration, pre-restore and pre-edit copies (keep 3).

use std::io;
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime};
use rl_core::store::migrations::SCHEMA_VERSION;
use rusqlite::{params, Connection};
use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

pub const KEEP_DAILY: usize = 7;
pub const KEEP_OTHER: usize = 3;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("backup file error: {0}")]
    Io(#[from] io::Error),
    #[error("backup database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0} already exists")]
    Exists(PathBuf),
    #[error("the backup is damaged: {0}")]
    Corrupt(String),
    #[error("the backup has schema version {0}, which this build cannot open")]
    Incompatible(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Daily,
    PreMigration,
    PreRestore,
    PreEdit,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct BackupInfo {
    pub name: String,
    pub kind: BackupKind,
    pub size_bytes: u64,
    pub modified_ms: i64,
}

pub fn daily_name(day: NaiveDate) -> String {
    format!("rl-trainer-{}.db", day.format("%Y-%m-%d"))
}

pub fn pre_migration_name(from_version: i64, at: NaiveDateTime) -> String {
    format!(
        "pre-migration-{}-v{from_version}.db",
        at.format("%Y%m%d-%H%M%S")
    )
}

pub fn pre_restore_name(at: NaiveDateTime) -> String {
    format!("pre-restore-{}.db", at.format("%Y%m%d-%H%M%S"))
}

pub fn classify(name: &str) -> Option<BackupKind> {
    if !name.ends_with(".db") {
        return None;
    }
    if name.starts_with("rl-trainer-") {
        Some(BackupKind::Daily)
    } else if name.starts_with("pre-migration-") {
        Some(BackupKind::PreMigration)
    } else if name.starts_with("pre-restore-") {
        Some(BackupKind::PreRestore)
    } else if name.starts_with("pre-edit-") {
        Some(BackupKind::PreEdit)
    } else {
        None
    }
}

pub fn pre_edit_name(at: NaiveDateTime, action: &str) -> String {
    format!("pre-edit-{}-{action}.db", at.format("%Y%m%d-%H%M%S-%3f"))
}

/// A copy taken right before a merge, split or delete (`action`), then rotation.
pub fn before_edit(conn: &Connection, dir: &Path, action: &str) -> Result<PathBuf, BackupError> {
    std::fs::create_dir_all(dir)?;
    let mut at = chrono::Local::now().naive_local();
    let mut path = dir.join(pre_edit_name(at, action));
    // Two edits in the same millisecond still get two files.
    while path.exists() {
        at += chrono::Duration::milliseconds(1);
        path = dir.join(pre_edit_name(at, action));
    }
    vacuum_into(conn, &path)?;
    rotate(dir)?;
    Ok(path)
}

/// A consistent copy of a live (WAL) database without pausing writers.
pub fn vacuum_into(conn: &Connection, dest: &Path) -> Result<(), BackupError> {
    if dest.exists() {
        return Err(BackupError::Exists(dest.to_path_buf()));
    }
    conn.execute("VACUUM INTO ?1", params![dest.to_string_lossy()])?;
    Ok(())
}

pub fn ensure_daily(
    conn: &Connection,
    dir: &Path,
    today: NaiveDate,
) -> Result<Option<PathBuf>, BackupError> {
    let path = dir.join(daily_name(today));
    if path.exists() {
        return Ok(None);
    }
    vacuum_into(conn, &path)?;
    Ok(Some(path))
}

fn names_of(dir: &Path) -> Result<Vec<(String, BackupKind)>, BackupError> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some(kind) = classify(&name) {
            names.push((name, kind));
        }
    }
    // Every name embeds a sortable timestamp after its prefix.
    names.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(names)
}

pub fn rotate(dir: &Path) -> Result<Vec<PathBuf>, BackupError> {
    let mut removed = Vec::new();
    for (kind, keep) in [
        (BackupKind::Daily, KEEP_DAILY),
        (BackupKind::PreMigration, KEEP_OTHER),
        (BackupKind::PreRestore, KEEP_OTHER),
        (BackupKind::PreEdit, KEEP_OTHER),
    ] {
        for (name, _) in names_of(dir)?
            .into_iter()
            .filter(|(_, k)| *k == kind)
            .skip(keep)
        {
            let path = dir.join(name);
            std::fs::remove_file(&path)?;
            removed.push(path);
        }
    }
    Ok(removed)
}

pub fn list(dir: &Path) -> Result<Vec<BackupInfo>, BackupError> {
    names_of(dir)?
        .into_iter()
        .map(|(name, kind)| {
            let meta = std::fs::metadata(dir.join(&name))?;
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
            Ok(BackupInfo {
                name,
                kind,
                size_bytes: meta.len(),
                modified_ms,
            })
        })
        .collect()
}

/// Checks a backup before it replaces the live database. Returns its schema version.
pub fn validate(path: &Path) -> Result<i64, BackupError> {
    let conn = crate::db::open_read(path)?;
    let check: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| BackupError::Corrupt(e.to_string()))?;
    if check != "ok" {
        return Err(BackupError::Corrupt(check));
    }
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| BackupError::Corrupt(e.to_string()))?;
    if !(1..=SCHEMA_VERSION).contains(&version) {
        return Err(BackupError::Incompatible(version));
    }
    Ok(version)
}

fn sidecar(db: &Path, suffix: &str) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Puts `backup` in place of `db`. Every connection to `db` must be closed first.
///
/// The old WAL files go first (a WAL left next to the restored file would be replayed into it),
/// then the backup is copied beside the database and renamed over it, so a failure at any step
/// leaves either the old database or the restored one, never a half-written file.
pub fn replace_database(db: &Path, backup: &Path) -> Result<(), BackupError> {
    for suffix in ["-wal", "-shm"] {
        let path = sidecar(db, suffix);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    let staged = sidecar(db, ".restore-tmp");
    std::fs::copy(backup, &staged)?;
    if let Err(err) = std::fs::rename(&staged, db) {
        let _ = std::fs::remove_file(&staged);
        return Err(err.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rl_core::store::open_db;

    fn date(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    fn sample_db(path: &Path) -> Connection {
        let conn = open_db(path).unwrap();
        conn.execute("INSERT INTO session (started_at) VALUES (1)", [])
            .unwrap();
        conn
    }

    #[test]
    fn names_classify_and_sort_chronologically() {
        let at = date(7).and_hms_opt(18, 30, 0).unwrap();
        assert_eq!(daily_name(date(7)), "rl-trainer-2026-10-07.db");
        assert_eq!(
            pre_migration_name(1, at),
            "pre-migration-20261007-183000-v1.db"
        );
        assert_eq!(pre_restore_name(at), "pre-restore-20261007-183000.db");
        assert_eq!(
            classify("rl-trainer-2026-10-07.db"),
            Some(BackupKind::Daily)
        );
        assert_eq!(
            classify("pre-migration-20261007-183000-v1.db"),
            Some(BackupKind::PreMigration)
        );
        assert_eq!(
            classify("pre-restore-20261007-183000.db"),
            Some(BackupKind::PreRestore)
        );
        assert_eq!(classify("notes.txt"), None);
    }

    #[test]
    fn daily_backup_is_written_once_per_day_and_restores_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let conn = sample_db(&dir.path().join("live.db"));
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        let first = ensure_daily(&conn, &backups, date(7)).unwrap().unwrap();
        assert!(ensure_daily(&conn, &backups, date(7)).unwrap().is_none());
        assert_eq!(
            validate(&first).unwrap(),
            rl_core::store::migrations::SCHEMA_VERSION
        );
        assert!(matches!(
            vacuum_into(&conn, &first),
            Err(BackupError::Exists(_))
        ));
    }

    #[test]
    fn rotation_keeps_seven_daily_and_three_of_each_other_kind() {
        let dir = tempfile::tempdir().unwrap();
        for d in 1..=9 {
            std::fs::write(dir.path().join(daily_name(date(d))), b"x").unwrap();
        }
        for s in 0..5 {
            let at = date(7).and_hms_opt(10, 0, s).unwrap();
            std::fs::write(dir.path().join(pre_restore_name(at)), b"x").unwrap();
        }
        std::fs::write(dir.path().join("keep-me.txt"), b"x").unwrap();
        let removed = rotate(dir.path()).unwrap();
        assert_eq!(removed.len(), 4);
        let names: Vec<String> = list(dir.path())
            .unwrap()
            .into_iter()
            .map(|b| b.name)
            .collect();
        assert_eq!(
            names
                .iter()
                .filter(|n| n.starts_with("rl-trainer-"))
                .count(),
            7
        );
        assert!(
            !names.contains(&daily_name(date(1))),
            "oldest daily removed"
        );
        assert_eq!(
            names
                .iter()
                .filter(|n| n.starts_with("pre-restore-"))
                .count(),
            3
        );
        assert!(dir.path().join("keep-me.txt").exists());
    }

    #[test]
    fn validation_rejects_garbage_and_newer_schemas() {
        let dir = tempfile::tempdir().unwrap();
        let garbage = dir.path().join("rl-trainer-2026-10-01.db");
        std::fs::write(&garbage, b"definitely not sqlite").unwrap();
        assert!(matches!(validate(&garbage), Err(BackupError::Corrupt(_))));
        let newer = dir.path().join("rl-trainer-2026-10-02.db");
        {
            let conn = Connection::open(&newer).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(
            validate(&newer),
            Err(BackupError::Incompatible(99))
        ));
    }

    #[test]
    fn a_replace_that_fails_leaves_the_live_database_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("live.db");
        let backup = dir.path().join("b.db");
        {
            let conn = sample_db(&db);
            vacuum_into(&conn, &backup).unwrap();
            conn.execute("INSERT INTO session (started_at) VALUES (2)", [])
                .unwrap();
        }
        // A WAL that cannot be removed (here: a directory) must stop the swap before it starts.
        std::fs::create_dir(dir.path().join("live.db-wal")).unwrap();
        assert!(replace_database(&db, &backup).is_err());
        std::fs::remove_dir(dir.path().join("live.db-wal")).unwrap();
        let conn = open_db(&db).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2, "the live data is still in place");
    }

    #[test]
    fn replacing_the_database_removes_stale_wal_files() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("live.db");
        let backup = dir.path().join("b.db");
        {
            let conn = sample_db(&db);
            vacuum_into(&conn, &backup).unwrap();
            conn.execute("INSERT INTO session (started_at) VALUES (2)", [])
                .unwrap();
        }
        std::fs::write(dir.path().join("live.db-wal"), b"stale").unwrap();
        std::fs::write(dir.path().join("live.db-shm"), b"stale").unwrap();
        replace_database(&db, &backup).unwrap();
        assert!(!dir.path().join("live.db-wal").exists());
        let conn = open_db(&db).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn before_edit_copies_have_unique_names_and_only_three_stay() {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(dir.path().join("live.db")).unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
        let backups = dir.path().join("backups");
        let made: Vec<PathBuf> = (0..5)
            .map(|_| before_edit(&conn, &backups, "merge").unwrap())
            .collect();
        let names: std::collections::HashSet<_> = made.iter().collect();
        assert_eq!(
            names.len(),
            5,
            "no two edits share a name, even in the same millisecond"
        );
        let kept = list(&backups).unwrap();
        assert_eq!(kept.len(), 3);
        assert!(kept.iter().all(|b| b.kind == BackupKind::PreEdit));
        assert!(kept.iter().all(|b| b.name.ends_with("-merge.db")));
        assert_eq!(
            classify("pre-edit-20261008-213000-123-split.db"),
            Some(BackupKind::PreEdit)
        );
    }
}
