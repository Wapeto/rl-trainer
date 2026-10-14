//! Opening the app database: one-time dev import, pre-migration backup, migrations, recovery.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rl_core::store::{open_db, pending_upgrade, recover_open_sessions, RecoveryReport, StoreError};
use rusqlite::{Connection, OpenFlags};
use thiserror::Error;

use crate::backup::{self, BackupError};
use crate::paths::AppPaths;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("file error: {0}")]
    Io(#[from] io::Error),
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Backup(#[from] BackupError),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenReport {
    pub imported_dev_db: bool,
    pub pre_migration_backup: Option<PathBuf>,
    pub recovery: RecoveryReport,
}

const BUSY: Duration = Duration::from_secs(5);

/// Read-write connection to an existing file; never creates one.
pub fn open_existing(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(BUSY)?;
    Ok(conn)
}

pub fn open_read(db: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(BUSY)?;
    Ok(conn)
}

pub fn open_write(db: &Path) -> rusqlite::Result<Connection> {
    let conn = open_existing(db)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

/// Copies the dev database into place when the app has none yet. `VACUUM INTO` reads a
/// consistent snapshot (WAL included) and writes nothing to the source.
pub fn import_dev_db(src: &Path, dst: &Path) -> Result<bool, DbError> {
    if dst.exists() || !src.exists() {
        return Ok(false);
    }
    let conn = open_existing(src)?;
    backup::vacuum_into(&conn, dst)?;
    tracing::info!(from = %src.display(), to = %dst.display(), "imported the dev database");
    Ok(true)
}

fn backup_before_upgrade(db: &Path, dir: &Path) -> Result<Option<PathBuf>, DbError> {
    if !db.exists() {
        return Ok(None);
    }
    let conn = open_existing(db)?;
    let Some(from) = pending_upgrade(&conn)? else {
        return Ok(None);
    };
    let path = dir.join(backup::pre_migration_name(
        from,
        chrono::Local::now().naive_local(),
    ));
    backup::vacuum_into(&conn, &path)?;
    tracing::info!(path = %path.display(), from, "backed up before migrating");
    Ok(Some(path))
}

pub fn open_app_db(
    paths: &AppPaths,
    dev_db: Option<&Path>,
) -> Result<(Connection, OpenReport), DbError> {
    paths.ensure_dirs()?;
    let imported_dev_db = match dev_db {
        Some(src) => import_dev_db(src, &paths.db)?,
        None => false,
    };
    let pre_migration_backup = backup_before_upgrade(&paths.db, &paths.backups)?;
    let mut conn = open_db(&paths.db)?;
    let recovery = recover_open_sessions(&mut conn)?;
    if recovery != RecoveryReport::default() {
        tracing::warn!(?recovery, "closed sessions left open by a crash");
    }
    Ok((
        conn,
        OpenReport {
            imported_dev_db,
            pre_migration_backup,
            recovery,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::store::migrations::migrate_to;

    #[test]
    fn dev_database_is_imported_once_and_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let dev = dir.path().join("rl-trainer-dev.db");
        {
            let conn = rl_core::store::open_db(&dev).unwrap();
            conn.execute(
                "INSERT INTO session (started_at, ended_at) VALUES (1, 2)",
                [],
            )
            .unwrap();
        }
        let before = std::fs::read(&dev).unwrap();
        let paths = AppPaths::under(&dir.path().join("app"));
        let (conn, report) = open_app_db(&paths, Some(&dev)).unwrap();
        assert!(report.imported_dev_db);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        drop(conn);
        assert_eq!(std::fs::read(&dev).unwrap(), before, "source unchanged");
        let (_, again) = open_app_db(&paths, Some(&dev)).unwrap();
        assert!(
            !again.imported_dev_db,
            "never overwrites an existing app database"
        );
    }

    #[test]
    fn populated_old_database_is_backed_up_before_migrating_and_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::under(dir.path());
        paths.ensure_dirs().unwrap();
        {
            let mut conn = Connection::open(&paths.db).unwrap();
            migrate_to(&mut conn, 1).unwrap();
            conn.execute_batch(
                "INSERT INTO session (id, started_at) VALUES (1, 100);
                 INSERT INTO block (session_id, kind, started_at) VALUES (1, 'freeplay', 200);",
            )
            .unwrap();
        }
        let (_, report) = open_app_db(&paths, None).unwrap();
        let backup = report.pre_migration_backup.expect("backup written");
        assert!(backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-v1.db"));
        assert_eq!(report.recovery.sessions_closed, 1);
        let fresh = AppPaths::under(&dir.path().join("fresh"));
        let (_, report) = open_app_db(&fresh, None).unwrap();
        assert!(report.pre_migration_backup.is_none() && !report.imported_dev_db);
    }
}
