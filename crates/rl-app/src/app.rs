//! The application facade used by the Tauri shell, the phone server and `rl-replay serve`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard};
use std::time::Duration;

use chrono::Local;
use rl_core::ingest::now_ms;
use rl_core::live::SessionSummary;
use rl_core::store::{
    clear_active_profile, edits, list_profiles, set_active_profile, MmrInput, PackStats,
    ProfileRow, StoreError,
};
use rusqlite::Connection;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::backup::{self, BackupError, BackupInfo};
use crate::db::{open_app_db, open_read, open_write, DbError};
use crate::elevated;
use crate::engine::{
    spawn_engine, AppSnapshot, CommandError, EngineConfig, EngineHandle, EngineTask,
};
use crate::game_watch::ini::{self, StatsApiStatus};
use crate::game_watch::locator::{locate, Install, InstallHints};
use crate::game_watch::{self, GameWatchEvent, POLL_EVERY};
use crate::pairing::{pairing_info, PairingInfo};
use crate::paths::AppPaths;
use crate::platform::{LanAddresses, Platform, PlatformError};
use crate::settings::{self, Settings, SettingsError, SettingsPatch};

const BACKUP_EVERY: Duration = Duration::from_secs(60 * 60);

/// How often the replay folders are checked for new files (spec §4.1).
pub const REPLAY_POLL_EVERY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub paths: AppPaths,
    pub dev_db: Option<PathBuf>,
    pub stats_addr_override: Option<String>,
    pub watch_game: bool,
    pub poll_every: Duration,
    /// Replay folder override (tests, `rl-replay serve --replay-dir`); otherwise the setting,
    /// then the game's folders under Documents.
    pub replay_dir: Option<PathBuf>,
    pub replay_poll_every: Duration,
    /// Where reports are sent; the build's relay address unless a test sets one.
    pub report_url: Option<String>,
}

impl AppConfig {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            dev_db: None,
            stats_addr_override: None,
            watch_game: true,
            poll_every: POLL_EVERY,
            replay_dir: None,
            replay_poll_every: REPLAY_POLL_EVERY,
            report_url: crate::report::relay_url().map(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, ts_rs::TS)]
pub struct StartupReport {
    pub imported_dev_db: bool,
    pub pre_migration_backup: Option<String>,
    pub sessions_recovered: usize,
    pub daily_backup_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Backup(#[from] BackupError),
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error("the administrator command failed (exit code {0})")]
    Elevated(i32),
    #[error("Rocket League's install folder was not found; set it in Settings")]
    InstallNotFound,
    #[error("there is no backup named {0}")]
    UnknownBackup(String),
    #[error("phone access: {0}")]
    Phone(String),
    #[error("a background task failed: {0}")]
    Join(String),
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The patch that puts back, from `before`, every field `patch` touched.
fn undo_patch(patch: &SettingsPatch, before: &Settings) -> SettingsPatch {
    SettingsPatch {
        phone_port: patch.phone_port.map(|_| before.phone_port),
        session_timeout_min: patch
            .session_timeout_min
            .map(|_| before.session_timeout_min),
        hard_hit_kmh: patch.hard_hit_kmh.map(|_| before.hard_hit_kmh),
        install_dir: patch
            .install_dir
            .as_ref()
            .map(|_| before.install_dir.clone()),
        replay_dir: patch.replay_dir.as_ref().map(|_| before.replay_dir.clone()),
        me_hint: patch
            .me_hint
            .as_ref()
            .map(|_| before.me_hint.clone().unwrap_or_default()),
        setup_dismissed: patch.setup_dismissed.map(|_| before.setup_dismissed),
        usage_ping: patch.usage_ping.map(|_| before.usage_ping),
    }
}

/// Keeps a background task so `shutdown` can stop it.
pub(crate) fn push_background(app: &App, task: JoinHandle<()>) {
    lock(&app.inner.background).push(task);
}

/// A connection that keeps a restore from swapping the file underneath it.
pub(crate) struct GatedConn<'a> {
    conn: Connection,
    _gate: RwLockReadGuard<'a, ()>,
}

impl std::ops::Deref for GatedConn<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.conn
    }
}

fn joined(err: tokio::task::JoinError) -> AppError {
    AppError::Join(err.to_string())
}

pub(crate) struct Inner {
    pub(crate) paths: AppPaths,
    pub(crate) platform: Arc<dyn Platform>,
    pub(crate) report_url: Option<String>,
    pub(crate) snapshots: Arc<watch::Sender<AppSnapshot>>,
    engine: tokio::sync::Mutex<Option<EngineTask>>,
    stats_addr_override: Option<String>,
    install: Mutex<Option<Install>>,
    game_running: AtomicBool,
    report: StartupReport,
    stop: watch::Sender<bool>,
    background: Mutex<Vec<JoinHandle<()>>>,
    /// Ad-hoc connections hold it shared; a restore holds it exclusively while it swaps files.
    db_gate: Arc<RwLock<()>>,
    phone: tokio::sync::Mutex<Option<crate::phone::PhoneServer>>,
    /// The phone server listens for the LAN (`enable_phone`), not on a given address.
    phone_lan: AtomicBool,
    token: watch::Sender<String>,
    phone_clients: std::sync::atomic::AtomicU32,
    pub(crate) replay_dir_override: Option<PathBuf>,
}

#[derive(Clone)]
pub struct App {
    pub(crate) inner: Arc<Inner>,
}

fn engine_config(settings: &Settings, stats_addr_override: &Option<String>) -> EngineConfig {
    EngineConfig {
        stats_addr_override: stats_addr_override.clone(),
        ..EngineConfig::new(settings.tracker_config())
    }
}

fn daily_backup(conn: &Connection, dir: &std::path::Path) -> Result<(), BackupError> {
    if let Some(path) = backup::ensure_daily(conn, dir, Local::now().date_naive())? {
        tracing::info!(path = %path.display(), "daily backup written");
    }
    backup::rotate(dir)?;
    Ok(())
}

/// Saves a pre-restore copy, puts `source` in place and reopens. The second value is the
/// swap's own outcome: when it failed, the connection is to whatever database is in place.
fn swap_and_reopen(
    paths: &AppPaths,
    source: &std::path::Path,
) -> Result<(Connection, Result<(), AppError>), AppError> {
    let copy = paths
        .backups
        .join(backup::pre_restore_name(Local::now().naive_local()));
    let swapped = (|| -> Result<(), AppError> {
        backup::vacuum_into(&open_read(&paths.db)?, &copy)?;
        backup::replace_database(&paths.db, source)?;
        Ok(())
    })();
    match open_app_db(paths, None) {
        Ok((conn, _)) => {
            if let Err(err) = backup::rotate(&paths.backups) {
                tracing::warn!(error = %err, "could not rotate backups after the restore");
            }
            Ok((conn, swapped))
        }
        // The restored file passed validation but does not open: put the previous one back.
        Err(err) if swapped.is_ok() => {
            tracing::error!(error = %err, "the restored database does not open; rolling back");
            backup::replace_database(&paths.db, &copy)?;
            let (conn, _) = open_app_db(paths, None)?;
            Ok((conn, Err(err.into())))
        }
        Err(err) => Err(err.into()),
    }
}

impl App {
    pub async fn start(config: AppConfig, platform: Arc<dyn Platform>) -> Result<App, AppError> {
        let paths = config.paths.clone();
        let dev_db = config.dev_db.clone();
        let (conn, open) =
            tokio::task::spawn_blocking(move || open_app_db(&paths, dev_db.as_deref()))
                .await
                .map_err(joined)??;
        if open.imported_dev_db {
            settings::set_dev_db_imported_at(&conn, now_ms())?;
        }
        let current = settings::load(&conn)?;
        let token = settings::token(&conn)?;
        let mut report = StartupReport {
            imported_dev_db: open.imported_dev_db,
            pre_migration_backup: open.pre_migration_backup.map(|p| p.display().to_string()),
            sessions_recovered: open.recovery.sessions_closed,
            daily_backup_error: None,
        };
        // Reads and writes files: off the async thread, like the hourly check.
        let backups = config.paths.backups.clone();
        let (conn, backed_up) = tokio::task::spawn_blocking(move || {
            let result = daily_backup(&conn, &backups);
            (conn, result)
        })
        .await
        .map_err(joined)?;
        if let Err(err) = backed_up {
            tracing::error!(error = %err, "daily backup failed");
            report.daily_backup_error = Some(err.to_string());
        }
        let snapshots = Arc::new(watch::channel(AppSnapshot::default()).0);
        let engine = spawn_engine(
            conn,
            engine_config(&current, &config.stats_addr_override),
            snapshots.clone(),
        )?;
        let app = App {
            inner: Arc::new(Inner {
                paths: config.paths.clone(),
                platform,
                report_url: config.report_url.clone(),
                snapshots,
                engine: tokio::sync::Mutex::new(Some(engine)),
                stats_addr_override: config.stats_addr_override.clone(),
                install: Mutex::new(None),
                game_running: AtomicBool::new(false),
                report,
                stop: watch::channel(false).0,
                background: Mutex::new(Vec::new()),
                db_gate: Arc::new(RwLock::new(())),
                phone: tokio::sync::Mutex::new(None),
                phone_lan: AtomicBool::new(false),
                token: watch::channel(token).0,
                phone_clients: std::sync::atomic::AtomicU32::new(0),
                replay_dir_override: config.replay_dir.clone(),
            }),
        };
        if config.watch_game {
            app.spawn_game_watch(config.poll_every);
        }
        app.spawn_hourly_backups();
        if current.phone_enabled {
            let bind =
                std::net::SocketAddr::new(app.inner.platform.lan_bind_ip(), current.phone_port);
            match app.start_phone(bind).await {
                Ok(()) => app.inner.phone_lan.store(true, Ordering::SeqCst),
                Err(err) => tracing::error!(error = %err, "phone access could not be restored"),
            }
        }
        app.spawn_replay_poller(config.replay_poll_every);
        // The game may already be set up: show the Stats API state before it starts.
        app.recheck_stats_api().await;
        app.spawn_usage_ping();
        Ok(app)
    }

    pub(crate) fn stop_receiver(&self) -> watch::Receiver<bool> {
        self.inner.stop.subscribe()
    }

    fn gate(&self) -> RwLockReadGuard<'_, ()> {
        self.inner
            .db_gate
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn read_db(&self) -> Result<GatedConn<'_>, AppError> {
        let gate = self.gate();
        Ok(GatedConn {
            conn: open_read(&self.inner.paths.db)?,
            _gate: gate,
        })
    }

    pub(crate) fn write_db(&self) -> Result<GatedConn<'_>, AppError> {
        let gate = self.gate();
        Ok(GatedConn {
            conn: open_write(&self.inner.paths.db)?,
            _gate: gate,
        })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.inner.paths
    }

    pub fn snapshot(&self) -> AppSnapshot {
        self.inner.snapshots.borrow().clone()
    }

    pub fn snapshots(&self) -> watch::Receiver<AppSnapshot> {
        self.inner.snapshots.subscribe()
    }

    pub fn startup_report(&self) -> StartupReport {
        self.inner.report.clone()
    }

    pub(crate) async fn engine(&self) -> Result<EngineHandle, AppError> {
        self.inner
            .engine
            .lock()
            .await
            .as_ref()
            .map(|task| task.handle.clone())
            .ok_or(AppError::Command(CommandError::Stopped))
    }

    fn spawn_game_watch(&self, every: Duration) {
        let (tx, mut rx) = mpsc::channel(8);
        let probe = self.inner.platform.process_probe();
        let watcher = tokio::spawn(game_watch::watch(
            probe,
            every,
            tx,
            self.inner.stop.subscribe(),
        ));
        let app = self.clone();
        let forward = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                app.on_game_event(event).await;
            }
        });
        lock(&self.inner.background).extend([watcher, forward]);
    }

    fn spawn_hourly_backups(&self) {
        let paths = self.inner.paths.clone();
        let gate = self.inner.db_gate.clone();
        let mut stop = self.inner.stop.subscribe();
        let task = tokio::spawn(async move {
            let mut every = tokio::time::interval(BACKUP_EVERY);
            every.tick().await; // startup already took today's backup
            loop {
                tokio::select! {
                    _ = every.tick() => {}
                    _ = stop.changed() => return,
                }
                let paths = paths.clone();
                let gate = gate.clone();
                let result = tokio::task::spawn_blocking(move || -> Result<(), AppError> {
                    let _shared = gate.read().unwrap_or_else(PoisonError::into_inner);
                    let conn = open_read(&paths.db)?;
                    Ok(daily_backup(&conn, &paths.backups)?)
                })
                .await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => tracing::error!(error = %err, "hourly backup check failed"),
                    Err(err) => tracing::error!(error = %err, "backup task failed"),
                }
            }
        });
        lock(&self.inner.background).push(task);
    }

    async fn on_game_event(&self, event: GameWatchEvent) {
        // Record the game's state first: during a restore there is no engine, and the restore
        // replays a start from this flag when it finishes.
        let started = matches!(event, GameWatchEvent::Started { .. });
        self.inner.game_running.store(started, Ordering::SeqCst);
        let engine = match self.engine().await {
            Ok(engine) => engine,
            Err(err) => {
                tracing::error!(error = %err, "game event while the recorder is stopped");
                return;
            }
        };
        if let GameWatchEvent::Started { exe } = &event {
            let status = self.refresh_stats_api(exe.clone());
            if let Err(err) = engine.stats_api(status).await {
                tracing::error!(error = %err, "could not report the Stats API status");
            }
        }
        if let Err(err) = engine.game_event(event).await {
            tracing::error!(error = %err, "could not forward the game event");
        }
    }

    fn locate_install(&self, running_exe: Option<PathBuf>) -> Option<Install> {
        let manual = self.settings().ok().and_then(|s| s.install_dir);
        locate(&InstallHints {
            running_exe,
            epic_manifests: self.inner.platform.epic_manifests_dir(),
            steam_root: self.inner.platform.steam_root(),
            manual,
        })
    }

    fn refresh_stats_api(&self, running_exe: Option<PathBuf>) -> StatsApiStatus {
        let found = self.locate_install(running_exe);
        let mut install = lock(&self.inner.install);
        if found.is_some() {
            *install = found;
        }
        match install.as_ref() {
            Some(i) => ini::check(&i.stats_ini()),
            None => StatsApiStatus::InstallNotFound,
        }
    }

    pub async fn recheck_stats_api(&self) -> StatsApiStatus {
        let status = self.refresh_stats_api(None);
        match self.engine().await {
            Ok(engine) => {
                if let Err(err) = engine.stats_api(status.clone()).await {
                    tracing::error!(error = %err, "could not report the Stats API status");
                }
            }
            Err(err) => tracing::error!(error = %err, "recheck while the recorder is stopped"),
        }
        status
    }

    /// Asks for administrator rights (UAC) and turns the Stats API on. Never runs unasked.
    pub async fn fix_stats_api(&self) -> Result<StatsApiStatus, AppError> {
        let install = lock(&self.inner.install)
            .clone()
            .or_else(|| self.locate_install(None))
            .ok_or(AppError::InstallNotFound)?;
        let args = elevated::fix_stats_api_args(&install.stats_ini());
        let platform = self.inner.platform.clone();
        let code = tokio::task::spawn_blocking(move || platform.run_elevated(&args))
            .await
            .map_err(joined)??;
        if code != elevated::EXIT_OK {
            return Err(AppError::Elevated(code));
        }
        Ok(self.recheck_stats_api().await)
    }

    pub async fn set_block_pack(
        &self,
        block_id: i64,
        pack_id: Option<i64>,
    ) -> Result<(), AppError> {
        Ok(self
            .engine()
            .await?
            .set_block_pack(block_id, pack_id)
            .await?)
    }

    pub async fn set_attempt_skipped(
        &self,
        attempt_id: i64,
        skipped: bool,
    ) -> Result<(), AppError> {
        Ok(self
            .engine()
            .await?
            .set_attempt_skipped(attempt_id, skipped)
            .await?)
    }

    pub async fn add_mmr(
        &self,
        team_size: u8,
        mmr: i32,
        session_id: Option<i64>,
    ) -> Result<i64, AppError> {
        Ok(self
            .engine()
            .await?
            .add_mmr(team_size, mmr, session_id)
            .await?)
    }

    pub async fn set_session_notes(
        &self,
        session_id: i64,
        notes: String,
    ) -> Result<String, AppError> {
        Ok(self
            .engine()
            .await?
            .set_session_notes(session_id, notes)
            .await?)
    }

    /// Visible packs for the phone's picker, filtered by the shared Rust search.
    pub fn packs(&self, search: &str) -> Result<Vec<PackStats>, AppError> {
        let conn = self.read_db()?;
        Ok(edits::pack_stats(&conn)?
            .into_iter()
            .filter(|p| rl_core::store::packs::pack_matches(&p.pack, search))
            .collect())
    }

    /// Saves the end-of-session review (MMR per ranked mode, note) and marks the session reviewed.
    pub async fn review_session(
        &self,
        session_id: i64,
        mmr: Vec<MmrInput>,
        notes: Option<String>,
    ) -> Result<SessionSummary, AppError> {
        Ok(self
            .engine()
            .await?
            .review_session(session_id, mmr, notes)
            .await?)
    }

    pub fn settings(&self) -> Result<Settings, AppError> {
        Ok(settings::load(&*self.read_db()?)?)
    }

    /// Saves settings and applies them at once: limits go to the recorder, a new port restarts
    /// the phone server when it is on (on the same address kind), a new install folder re-checks
    /// the Stats API.
    pub async fn update_settings(&self, patch: &SettingsPatch) -> Result<Settings, AppError> {
        let before = self.settings()?;
        let after = settings::apply_patch(&*self.write_db()?, patch)?;
        if after.phone_port != before.phone_port {
            if let Err(err) = self.move_phone_port(after.phone_port).await {
                // Nothing of a failed change stays (spec §7.2 item 4).
                settings::apply_patch(&*self.write_db()?, &undo_patch(patch, &before))?;
                return Err(err);
            }
        }
        if after.session_timeout_min != before.session_timeout_min
            || after.hard_hit_kmh != before.hard_hit_kmh
        {
            let limits = after.tracker_config();
            self.engine()
                .await?
                .run(move |p| {
                    p.set_limits(limits.session_timeout_ms, limits.hard_hit_kmh);
                    Ok(())
                })
                .await?;
        }
        if after.install_dir != before.install_dir {
            self.recheck_stats_api().await;
        }
        if after.me_hint != before.me_hint {
            self.apply_me_hint(after.me_hint.as_deref())?;
        }
        Ok(after)
    }

    /// Restarts a running phone server on `new_port` (the setting already holds it). On failure
    /// it serves on the old address again — the firewall rule moved back too — and returns the
    /// first error; the caller puts the setting back.
    async fn move_phone_port(&self, new_port: u16) -> Result<(), AppError> {
        let Some(old) = self.phone_addr().await else {
            return Ok(());
        };
        let lan = self.inner.phone_lan.load(Ordering::SeqCst);
        let restarted = if lan {
            self.enable_phone().await.map(|_| ())
        } else {
            let new = std::net::SocketAddr::new(old.ip(), new_port);
            self.enable_phone_at(new).await.map(|_| ())
        };
        let Err(err) = restarted else {
            return Ok(());
        };
        if lan {
            // `enable_phone` reads the port from the settings: put the old one back first.
            let undo = SettingsPatch {
                phone_port: Some(old.port()),
                ..SettingsPatch::default()
            };
            settings::apply_patch(&*self.write_db()?, &undo)?;
            self.enable_phone().await?;
        } else {
            self.enable_phone_at(old).await?;
        }
        Err(err)
    }

    async fn phone_addr(&self) -> Option<std::net::SocketAddr> {
        self.inner.phone.lock().await.as_ref().map(|s| s.local_addr)
    }

    pub fn profiles(&self) -> Result<Vec<ProfileRow>, AppError> {
        Ok(list_profiles(&*self.read_db()?)?)
    }

    /// A seen profile that matches the hint becomes active, else none is, so detection runs on
    /// the next game. Clearing the hint keeps the active profile. Applies from the next start.
    fn apply_me_hint(&self, hint: Option<&str>) -> Result<(), AppError> {
        let Some(hint) = hint else {
            return Ok(());
        };
        let conn = self.write_db()?;
        let found = list_profiles(&conn)?
            .into_iter()
            .find(|p| rl_core::tracker::hint_matches(hint, &p.display_name, &p.primary_id));
        match found {
            Some(p) => {
                set_active_profile(&conn, &p.primary_id)?;
            }
            None => clear_active_profile(&conn)?,
        }
        Ok(())
    }

    /// Applies from the next app start (the recorder keeps the identity it started with).
    pub fn set_active_profile(&self, primary_id: &str) -> Result<bool, AppError> {
        Ok(set_active_profile(&*self.write_db()?, primary_id)?)
    }

    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, AppError> {
        Ok(backup::list(&self.inner.paths.backups)?)
    }

    /// Stops recording (closing the open session), saves a pre-restore copy, swaps the database
    /// and restarts recording on it. On failure, recording restarts on whatever is in place.
    pub async fn restore_backup(&self, name: &str) -> Result<(), AppError> {
        if !self.list_backups()?.iter().any(|b| b.name == name) {
            return Err(AppError::UnknownBackup(name.to_owned()));
        }
        let source = self.inner.paths.backups.join(name);
        backup::validate(&source)?;
        let mut slot = self.inner.engine.lock().await;
        if let Some(task) = slot.take() {
            task.handle.shutdown().await;
            if let Err(err) = task.join.await {
                tracing::error!(error = %err, "recorder ended abnormally before the restore");
            }
        }
        let paths = self.inner.paths.clone();
        let gate = self.inner.db_gate.clone();
        let (conn, swapped) = tokio::task::spawn_blocking(move || {
            let _exclusive = gate.write().unwrap_or_else(PoisonError::into_inner);
            swap_and_reopen(&paths, &source)
        })
        .await
        .map_err(joined)??;
        let current = settings::load(&conn).unwrap_or_else(|err| {
            tracing::error!(error = %err, "settings unreadable after the restore; using defaults");
            Settings::default()
        });
        let task = spawn_engine(
            conn,
            engine_config(&current, &self.inner.stats_addr_override),
            self.inner.snapshots.clone(),
        )?;
        let handle = task.handle.clone();
        *slot = Some(task);
        drop(slot);
        if self.inner.game_running.load(Ordering::SeqCst) {
            handle
                .game_event(GameWatchEvent::Started { exe: None })
                .await?;
        }
        swapped
    }

    pub fn token_receiver(&self) -> watch::Receiver<String> {
        self.inner.token.subscribe()
    }

    async fn start_phone(&self, bind: std::net::SocketAddr) -> Result<(), AppError> {
        let mut slot = self.inner.phone.lock().await;
        if let Some(old) = slot.take() {
            old.stop().await;
        }
        let server = crate::phone::start(self.clone(), bind, self.token_receiver())
            .await
            .map_err(|e| AppError::Phone(format!("cannot listen on port {}: {e}", bind.port())))?;
        *slot = Some(server);
        Ok(())
    }

    async fn ensure_firewall(&self, port: u16) -> Result<(), AppError> {
        let recorded = self.settings()?.firewall_port == Some(port);
        let exe = self.inner.platform.current_exe();
        if recorded && self.inner.platform.firewall_rule_allows(&exe) {
            return Ok(());
        }
        let args = elevated::firewall_args(port, &exe);
        let platform = self.inner.platform.clone();
        let code = tokio::task::spawn_blocking(move || platform.run_elevated(&args))
            .await
            .map_err(joined)??;
        if code != elevated::EXIT_OK {
            return Err(AppError::Elevated(code));
        }
        settings::set_firewall_port(&*self.write_db()?, port)?;
        Ok(())
    }

    /// Adds the private-network firewall rule if needed (one UAC prompt), then starts serving.
    pub async fn enable_phone(&self) -> Result<PairingInfo, AppError> {
        let port = self.settings()?.phone_port;
        self.ensure_firewall(port).await?;
        self.start_phone(std::net::SocketAddr::new(
            self.inner.platform.lan_bind_ip(),
            port,
        ))
        .await?;
        self.inner.phone_lan.store(true, Ordering::SeqCst);
        settings::set_phone_enabled(&*self.write_db()?, true)?;
        self.pairing()
            .await
            .ok_or_else(|| AppError::Phone("the server did not start".into()))
    }

    /// Serves without touching the firewall or settings (tests and `rl-replay serve`).
    pub async fn enable_phone_at(
        &self,
        bind: std::net::SocketAddr,
    ) -> Result<PairingInfo, AppError> {
        self.start_phone(bind).await?;
        self.inner.phone_lan.store(false, Ordering::SeqCst);
        self.pairing()
            .await
            .ok_or_else(|| AppError::Phone("the server did not start".into()))
    }

    pub async fn disable_phone(&self) -> Result<(), AppError> {
        self.stop_phone().await;
        settings::set_phone_enabled(&*self.write_db()?, false)?;
        Ok(())
    }

    async fn stop_phone(&self) {
        if let Some(server) = self.inner.phone.lock().await.take() {
            server.stop().await;
        }
    }

    /// Issues a new token; every paired device must scan the new QR code.
    pub async fn regenerate_token(&self) -> Result<Option<PairingInfo>, AppError> {
        let token = settings::regenerate_token(&*self.write_db()?)?;
        self.inner.token.send_replace(token);
        Ok(self.pairing().await)
    }

    pub async fn pairing(&self) -> Option<PairingInfo> {
        let port_and_ip = {
            let slot = self.inner.phone.lock().await;
            slot.as_ref()
                .map(|s| (s.local_addr.port(), s.local_addr.ip()))
        }?;
        let (port, ip) = port_and_ip;
        let lan = if ip.is_loopback() {
            LanAddresses {
                preferred: Some(ip),
                all: vec![ip],
            }
        } else {
            self.inner.platform.lan_addresses()
        };
        let token = self.inner.token.borrow().clone();
        pairing_info(&lan, port, &token)
            .map_err(|err| tracing::error!(%err, "could not build the pairing QR code"))
            .ok()
    }

    pub async fn phone_client_changed(&self, connected: bool) {
        use std::sync::atomic::Ordering;
        let count = if connected {
            self.inner.phone_clients.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            self.inner
                .phone_clients
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1)
        };
        if let Ok(engine) = self.engine().await {
            if let Err(err) = engine.phone_clients(count).await {
                tracing::warn!(error = %err, "could not update the phone count");
            }
        }
    }

    /// Closes the open session and stops every background task.
    pub async fn shutdown(&self) {
        let _ = self.inner.stop.send(true);
        self.stop_phone().await;
        if let Some(task) = self.inner.engine.lock().await.take() {
            task.handle.shutdown().await;
            if let Err(err) = task.join.await {
                tracing::error!(error = %err, "recorder ended abnormally");
            }
        }
        for task in lock(&self.inner.background).drain(..) {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restored_file_that_will_not_open_is_rolled_back() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::under(dir.path());
        {
            let (conn, _) = open_app_db(&paths, None).unwrap();
            conn.execute(
                "INSERT INTO session (started_at, ended_at) VALUES (1, 2)",
                [],
            )
            .unwrap();
        }
        let garbage = paths.backups.join("rl-trainer-2026-10-01.db");
        std::fs::write(&garbage, b"definitely not a database").unwrap();
        let (conn, swapped) = swap_and_reopen(&paths, &garbage).expect("recording can restart");
        assert!(swapped.is_err(), "the restore is reported as failed");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "the previous database is back in place");
    }
}
