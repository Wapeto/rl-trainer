//! The engine actor: the only task that writes recording data. It owns the Pipeline, connects
//! to the Stats API only while the game runs, and publishes `AppSnapshot`s.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, TimeZone};
use rl_core::ingest::{self, now_ms, IngestMsg};
use rl_core::live::{LiveSnapshot, SessionSummary};
use rl_core::pipeline::Pipeline;
use rl_core::store::{EditError, MmrInput, StoreError};
use rl_core::tracker::TrackerConfig;
use rusqlite::Connection;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};
use ts_rs::TS;

use crate::game_watch::ini::{StatsApiStatus, DEFAULT_PORT};
use crate::game_watch::GameWatchEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct AppSnapshot {
    pub version: u64,
    /// +1 each time an engine starts (startup, restore): windows reload everything on a change.
    pub generation: u64,
    pub game_running: bool,
    pub ingest_connected: bool,
    pub stats_api: StatsApiStatus,
    pub phone_clients: u32,
    /// +1 per imported replay (spec §4): part of the windows' reload key.
    pub replays: u64,
    pub live: LiveSnapshot,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub tracker: TrackerConfig,
    /// Fixed Stats API address (tests, `rl-replay serve`); otherwise 127.0.0.1 and the ini port.
    pub stats_addr_override: Option<String>,
    pub tick_every: Duration,
    pub heartbeat_every: Duration,
    pub publish_every: Duration,
    /// Stats API messages waiting for the engine.
    pub ingest_buffer: usize,
}

impl EngineConfig {
    pub fn new(tracker: TrackerConfig) -> Self {
        Self {
            tracker,
            stats_addr_override: None,
            tick_every: Duration::from_secs(10),
            heartbeat_every: Duration::from_secs(30),
            publish_every: Duration::from_millis(500),
            ingest_buffer: 1024,
        }
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error("the recorder is not running")]
    Stopped,
}

type Reply<T> = oneshot::Sender<Result<T, EditError>>;

enum Command {
    Game(GameWatchEvent),
    StatsApi(StatsApiStatus),
    PhoneClients(u32),
    ReplaysImported,
    SetBlockPack {
        block_id: i64,
        pack_id: Option<i64>,
        reply: Reply<()>,
    },
    SetSkipped {
        attempt_id: i64,
        skipped: bool,
        reply: Reply<()>,
    },
    AddMmr {
        team_size: u8,
        mmr: i32,
        session_id: Option<i64>,
        reply: Reply<i64>,
    },
    SetNotes {
        session_id: i64,
        notes: String,
        reply: Reply<String>,
    },
    ReviewSession {
        session_id: i64,
        mmr: Vec<MmrInput>,
        notes: Option<String>,
        reply: Reply<SessionSummary>,
    },
    /// A desktop edit or setting change, run on the pipeline (the only writer).
    Run(Box<dyn FnOnce(&mut Pipeline) + Send>),
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<Command>,
}

impl EngineHandle {
    async fn send(&self, command: Command) -> Result<(), CommandError> {
        self.tx
            .send(command)
            .await
            .map_err(|_| CommandError::Stopped)
    }

    async fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Command) -> Result<T, CommandError> {
        let (reply, rx) = oneshot::channel();
        self.send(make(reply)).await?;
        Ok(rx.await.map_err(|_| CommandError::Stopped)??)
    }

    /// Runs `f` on the engine's pipeline and publishes a snapshot after it.
    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Pipeline) -> Result<T, EditError> + Send + 'static,
    ) -> Result<T, CommandError> {
        let (reply, rx) = oneshot::channel();
        self.send(Command::Run(Box::new(move |pipeline: &mut Pipeline| {
            let _ = reply.send(f(pipeline));
        })))
        .await?;
        Ok(rx.await.map_err(|_| CommandError::Stopped)??)
    }

    pub async fn game_event(&self, event: GameWatchEvent) -> Result<(), CommandError> {
        self.send(Command::Game(event)).await
    }

    pub async fn stats_api(&self, status: StatsApiStatus) -> Result<(), CommandError> {
        self.send(Command::StatsApi(status)).await
    }

    pub async fn phone_clients(&self, count: u32) -> Result<(), CommandError> {
        self.send(Command::PhoneClients(count)).await
    }

    pub async fn replays_imported(&self) -> Result<(), CommandError> {
        self.send(Command::ReplaysImported).await
    }

    pub async fn set_block_pack(
        &self,
        block_id: i64,
        pack_id: Option<i64>,
    ) -> Result<(), CommandError> {
        self.call(|reply| Command::SetBlockPack {
            block_id,
            pack_id,
            reply,
        })
        .await
    }

    pub async fn set_attempt_skipped(
        &self,
        attempt_id: i64,
        skipped: bool,
    ) -> Result<(), CommandError> {
        self.call(|reply| Command::SetSkipped {
            attempt_id,
            skipped,
            reply,
        })
        .await
    }

    pub async fn add_mmr(
        &self,
        team_size: u8,
        mmr: i32,
        session_id: Option<i64>,
    ) -> Result<i64, CommandError> {
        self.call(|reply| Command::AddMmr {
            team_size,
            mmr,
            session_id,
            reply,
        })
        .await
    }

    pub async fn set_session_notes(
        &self,
        session_id: i64,
        notes: String,
    ) -> Result<String, CommandError> {
        self.call(|reply| Command::SetNotes {
            session_id,
            notes,
            reply,
        })
        .await
    }

    pub async fn review_session(
        &self,
        session_id: i64,
        mmr: Vec<MmrInput>,
        notes: Option<String>,
    ) -> Result<SessionSummary, CommandError> {
        self.call(|reply| Command::ReviewSession {
            session_id,
            mmr,
            notes,
            reply,
        })
        .await
    }

    /// Closes the open session and stops the engine. Returns when it has finished.
    pub async fn shutdown(&self) {
        let (reply, rx) = oneshot::channel();
        if self.tx.send(Command::Shutdown { reply }).await.is_ok() {
            let _ = rx.await;
        }
    }
}

pub struct EngineTask {
    pub handle: EngineHandle,
    pub join: JoinHandle<()>,
}

struct IngestTask {
    stop: watch::Sender<bool>,
    join: JoinHandle<()>,
}

struct Engine {
    pipeline: Pipeline,
    cfg: EngineConfig,
    snapshots: Arc<watch::Sender<AppSnapshot>>,
    snap: AppSnapshot,
    ingest: Option<IngestTask>,
    ingest_tx: mpsc::Sender<IngestMsg>,
}

pub fn spawn_engine(
    conn: Connection,
    cfg: EngineConfig,
    snapshots: Arc<watch::Sender<AppSnapshot>>,
) -> Result<EngineTask, StoreError> {
    let pipeline = Pipeline::new(conn, cfg.tracker.clone())?;
    let (tx, rx) = mpsc::channel(64);
    let (ingest_tx, ingest_rx) = mpsc::channel(cfg.ingest_buffer.max(1));
    let previous = snapshots.borrow().clone();
    // A new engine (startup or after a restore) jumps the version so UIs reload everything.
    let snap = AppSnapshot {
        version: previous.version + 1000,
        generation: previous.generation + 1,
        stats_api: previous.stats_api,
        phone_clients: previous.phone_clients,
        replays: previous.replays,
        ..AppSnapshot::default()
    };
    let engine = Engine {
        pipeline,
        cfg,
        snapshots,
        snap,
        ingest: None,
        ingest_tx,
    };
    let join = tokio::spawn(engine.run(rx, ingest_rx));
    Ok(EngineTask {
        handle: EngineHandle { tx },
        join,
    })
}

fn day_start_ms() -> i64 {
    Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|midnight| Local.from_local_datetime(&midnight).earliest())
        .map_or_else(now_ms, |t| t.timestamp_millis())
}

impl Engine {
    async fn run(
        mut self,
        mut commands: mpsc::Receiver<Command>,
        mut ingest_rx: mpsc::Receiver<IngestMsg>,
    ) {
        let mut tick = interval(self.cfg.tick_every);
        let mut heartbeat = interval(self.cfg.heartbeat_every);
        let mut publish = interval(self.cfg.publish_every);
        for t in [&mut tick, &mut heartbeat, &mut publish] {
            t.set_missed_tick_behavior(MissedTickBehavior::Delay);
        }
        self.publish(true);
        loop {
            tokio::select! {
                Some(msg) = ingest_rx.recv() => self.on_ingest(&msg),
                command = commands.recv() => match command {
                    Some(Command::Shutdown { reply }) => {
                        self.stop(&mut ingest_rx).await;
                        let _ = reply.send(());
                        return;
                    }
                    Some(command) => self.on_command(command, &mut ingest_rx).await,
                    None => {
                        self.stop(&mut ingest_rx).await;
                        return;
                    }
                },
                _ = tick.tick() => {
                    let result = self.pipeline.tick(now_ms());
                    self.log_store(result.map(|_| ()));
                }
                _ = heartbeat.tick() => {
                    let result = self.pipeline.heartbeat(now_ms());
                    self.log_store(result);
                }
                _ = publish.tick() => self.publish(false),
            }
        }
    }

    fn log_store(&self, result: Result<(), StoreError>) {
        if let Err(err) = result {
            tracing::error!(
                error = %err,
                queued = self.pipeline.pending_facts(),
                "could not save yet; facts stay queued"
            );
        }
    }

    fn on_ingest(&mut self, msg: &IngestMsg) {
        match msg {
            IngestMsg::Connected { .. } => self.snap.ingest_connected = true,
            IngestMsg::Disconnected { .. } => self.snap.ingest_connected = false,
            IngestMsg::DecodeFailed { error, .. } => {
                tracing::warn!(%error, "Stats API message skipped")
            }
            IngestMsg::Event(_) => {}
        }
        let result = self.pipeline.handle(msg);
        self.log_store(result.map(|_| ()));
    }

    fn ingest_addr(&self) -> String {
        self.cfg.stats_addr_override.clone().unwrap_or_else(|| {
            format!(
                "127.0.0.1:{}",
                self.snap.stats_api.port().unwrap_or(DEFAULT_PORT)
            )
        })
    }

    fn start_ingest(&mut self) {
        if self.ingest.is_some() {
            return;
        }
        let (stop, stop_rx) = watch::channel(false);
        let addr = self.ingest_addr();
        tracing::info!(%addr, "connecting to the Stats API");
        let join = tokio::spawn(ingest::run(addr, self.ingest_tx.clone(), stop_rx));
        self.ingest = Some(IngestTask { stop, join });
    }

    /// Stops the reader while still draining its queue: a reader blocked on a full queue never
    /// sees the stop signal, so waiting without draining could hang forever. Messages read before
    /// the stop are applied before the caller closes the session.
    async fn stop_ingest(&mut self, ingest_rx: &mut mpsc::Receiver<IngestMsg>) {
        if let Some(task) = self.ingest.take() {
            let _ = task.stop.send(true);
            let mut join = task.join;
            loop {
                tokio::select! {
                    joined = &mut join => {
                        if let Err(err) = joined {
                            tracing::error!(error = %err, "Stats API reader ended abnormally");
                        }
                        break;
                    }
                    Some(msg) = ingest_rx.recv() => self.on_ingest(&msg),
                }
            }
            while let Ok(msg) = ingest_rx.try_recv() {
                self.on_ingest(&msg);
            }
        }
        self.snap.ingest_connected = false;
    }

    async fn stop(&mut self, ingest_rx: &mut mpsc::Receiver<IngestMsg>) {
        self.stop_ingest(ingest_rx).await;
        let result = self.pipeline.game_exited(now_ms());
        self.log_store(result.map(|_| ()));
        self.snap.game_running = false;
        self.publish(true);
    }

    fn reply<T>(&mut self, reply: Reply<T>, result: Result<T, EditError>) {
        if let Err(EditError::Store(err)) = &result {
            tracing::error!(error = %err, "edit could not be saved");
        }
        // The caller may have gone away (closed phone page); nothing else to do then.
        let _ = reply.send(result);
        self.publish(true);
    }

    async fn on_command(&mut self, command: Command, ingest_rx: &mut mpsc::Receiver<IngestMsg>) {
        match command {
            Command::Game(GameWatchEvent::Started { .. }) => {
                self.snap.game_running = true;
                self.start_ingest();
                self.publish(true);
            }
            Command::Game(GameWatchEvent::Stopped) => {
                self.stop_ingest(ingest_rx).await;
                let result = self.pipeline.game_exited(now_ms());
                self.log_store(result.map(|_| ()));
                self.snap.game_running = false;
                self.publish(true);
            }
            Command::StatsApi(status) => {
                self.snap.stats_api = status;
                self.publish(true);
            }
            Command::PhoneClients(count) => {
                self.snap.phone_clients = count;
                self.publish(true);
            }
            Command::ReplaysImported => {
                self.snap.replays += 1;
                self.publish(true);
            }
            Command::SetBlockPack {
                block_id,
                pack_id,
                reply,
            } => {
                let result = self
                    .pipeline
                    .set_block_pack(block_id, pack_id, now_ms())
                    .map(|_| ());
                self.reply(reply, result);
            }
            Command::SetSkipped {
                attempt_id,
                skipped,
                reply,
            } => {
                let result = self
                    .pipeline
                    .set_attempt_skipped(attempt_id, skipped, now_ms());
                self.reply(reply, result);
            }
            Command::AddMmr {
                team_size,
                mmr,
                session_id,
                reply,
            } => {
                let result = self.pipeline.add_mmr(team_size, mmr, session_id, now_ms());
                self.reply(reply, result);
            }
            Command::SetNotes {
                session_id,
                notes,
                reply,
            } => {
                let result = self.pipeline.set_session_notes(session_id, &notes);
                self.reply(reply, result);
            }
            Command::ReviewSession {
                session_id,
                mmr,
                notes,
                reply,
            } => {
                let result =
                    self.pipeline
                        .review_session(session_id, &mmr, notes.as_deref(), now_ms());
                self.reply(reply, result);
            }
            Command::Run(job) => {
                job(&mut self.pipeline);
                self.publish(true);
            }
            Command::Shutdown { .. } => unreachable!("handled in run"),
        }
    }

    /// Publishes when anything changed (or when forced). Throttled by the publish interval.
    fn publish(&mut self, force: bool) {
        match self.pipeline.live_snapshot(day_start_ms()) {
            Ok(live) => {
                let next = AppSnapshot {
                    live,
                    ..self.snap.clone()
                };
                if force || next != self.snap {
                    self.snap = AppSnapshot {
                        version: self.snap.version + 1,
                        ..next
                    };
                    self.snapshots.send_replace(self.snap.clone());
                }
            }
            Err(err) => tracing::error!(error = %err, "could not build the live snapshot"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devtools::{update_state_json, FakeStatsApi};
    use rl_core::playlist::Activity;
    use rl_core::store::open_in_memory;
    use tokio::time::timeout;

    fn cfg(addr: &str) -> EngineConfig {
        EngineConfig {
            stats_addr_override: Some(addr.to_owned()),
            publish_every: Duration::from_millis(50),
            ..EngineConfig::new(TrackerConfig::default())
        }
    }

    async fn wait_for(
        rx: &mut watch::Receiver<AppSnapshot>,
        what: &str,
        f: impl FnMut(&AppSnapshot) -> bool,
    ) -> AppSnapshot {
        timeout(Duration::from_secs(10), rx.wait_for(f))
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .unwrap()
            .clone()
    }

    fn kind_is(s: &AppSnapshot, kind: Activity) -> bool {
        s.live.block.as_ref().is_some_and(|b| b.kind == kind)
    }

    async fn api(playlist: i32) -> FakeStatsApi {
        FakeStatsApi::start(
            vec![],
            Some((Duration::from_millis(50), update_state_json(playlist, 10.0))),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn records_only_while_the_game_runs() {
        let api = api(9).await;
        let snapshots = Arc::new(watch::channel(AppSnapshot::default()).0);
        let mut rx = snapshots.subscribe();
        let task =
            spawn_engine(open_in_memory().unwrap(), cfg(&api.addr), snapshots.clone()).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !rx.borrow().ingest_connected,
            "no connection before the game starts"
        );
        task.handle
            .game_event(GameWatchEvent::Started { exe: None })
            .await
            .unwrap();
        let s = wait_for(&mut rx, "a free play block", |s| {
            kind_is(s, Activity::Freeplay)
        })
        .await;
        assert!(s.game_running && s.ingest_connected);
        task.handle
            .game_event(GameWatchEvent::Stopped)
            .await
            .unwrap();
        let s = wait_for(&mut rx, "the session to close", |s| {
            !s.game_running && s.live.session.is_none()
        })
        .await;
        assert_eq!(s.live.last_session.unwrap().freeplay_blocks, 1);
        task.handle.shutdown().await;
        task.join.await.unwrap();
    }

    #[tokio::test]
    async fn game_exit_finishes_while_the_reader_is_blocked_on_a_full_queue() {
        let api = FakeStatsApi::start(vec![], Some((Duration::ZERO, update_state_json(9, 10.0))))
            .await
            .unwrap();
        let snapshots = Arc::new(watch::channel(AppSnapshot::default()).0);
        let mut rx = snapshots.subscribe();
        let cfg = EngineConfig {
            ingest_buffer: 1,
            ..cfg(&api.addr)
        };
        let task = spawn_engine(open_in_memory().unwrap(), cfg, snapshots.clone()).unwrap();
        task.handle
            .game_event(GameWatchEvent::Started { exe: None })
            .await
            .unwrap();
        wait_for(&mut rx, "free play", |s| kind_is(s, Activity::Freeplay)).await;
        task.handle
            .game_event(GameWatchEvent::Stopped)
            .await
            .unwrap();
        let closed = timeout(Duration::from_secs(5), async {
            wait_for(&mut rx, "the session to close", |s| {
                s.live.session.is_none()
            })
            .await;
            task.handle.shutdown().await;
            task.join.await.unwrap();
        })
        .await;
        assert!(closed.is_ok(), "the engine hung waiting for the reader");
        let s = snapshots.borrow().clone();
        assert!(!s.ingest_connected && s.live.last_session.is_some());
    }

    #[tokio::test]
    async fn commands_reply_and_publish() {
        let api = api(21).await;
        let snapshots = Arc::new(watch::channel(AppSnapshot::default()).0);
        let mut rx = snapshots.subscribe();
        let task =
            spawn_engine(open_in_memory().unwrap(), cfg(&api.addr), snapshots.clone()).unwrap();
        let h = task.handle.clone();
        h.game_event(GameWatchEvent::Started { exe: None })
            .await
            .unwrap();
        let s = wait_for(&mut rx, "a training block", |s| {
            kind_is(s, Activity::Training)
        })
        .await;
        let block = s.live.block.unwrap().id;
        h.set_block_pack(block, Some(1)).await.unwrap();
        wait_for(&mut rx, "the pack", |s| {
            s.live
                .block
                .as_ref()
                .and_then(|b| b.pack.as_ref())
                .is_some_and(|p| p.id == 1)
        })
        .await;
        assert!(matches!(
            h.set_block_pack(9_999, Some(1)).await,
            Err(CommandError::Edit(EditError::NotFound("activity")))
        ));
        h.add_mmr(2, 900, None).await.unwrap();
        wait_for(&mut rx, "the MMR", |s| {
            s.live.latest_mmr.iter().any(|m| m.mmr == 900)
        })
        .await;
        assert!(matches!(
            h.add_mmr(4, 900, None).await,
            Err(CommandError::Edit(EditError::Invalid(_)))
        ));
        h.shutdown().await;
        task.join.await.unwrap();
        assert!(matches!(
            h.add_mmr(2, 900, None).await,
            Err(CommandError::Stopped)
        ));
    }

    #[tokio::test]
    async fn shutdown_closes_the_session_and_a_new_engine_jumps_the_version() {
        let api = api(9).await;
        let snapshots = Arc::new(watch::channel(AppSnapshot::default()).0);
        let mut rx = snapshots.subscribe();
        let first =
            spawn_engine(open_in_memory().unwrap(), cfg(&api.addr), snapshots.clone()).unwrap();
        first
            .handle
            .stats_api(StatsApiStatus::InstallNotFound)
            .await
            .unwrap();
        first
            .handle
            .game_event(GameWatchEvent::Started { exe: None })
            .await
            .unwrap();
        wait_for(&mut rx, "free play", |s| kind_is(s, Activity::Freeplay)).await;
        first.handle.shutdown().await;
        first.join.await.unwrap();
        let closed = snapshots.borrow().clone();
        assert!(closed.live.session.is_none() && closed.live.last_session.is_some());
        let second =
            spawn_engine(open_in_memory().unwrap(), cfg(&api.addr), snapshots.clone()).unwrap();
        let s = wait_for(&mut rx, "the new engine", |s| {
            s.version >= closed.version + 1000
        })
        .await;
        assert_eq!(
            s.generation,
            closed.generation + 1,
            "a new engine counts as a new generation"
        );
        assert_eq!(
            s.stats_api,
            StatsApiStatus::InstallNotFound,
            "app-level status survives a restart"
        );
        second.handle.shutdown().await;
        second.join.await.unwrap();
    }
}
