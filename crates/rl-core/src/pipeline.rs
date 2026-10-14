//! Glue between ingest, the pure tracker and the recorder.

use rusqlite::Connection;

use crate::facts::Fact;
use crate::ingest::IngestMsg;
use crate::live::{LiveBlock, LiveSnapshot, SessionSummary};
use crate::playlist::Activity;
use crate::store::edits::{self, EditError, MmrInput};
use crate::store::live as live_q;
use crate::store::{profile_primary_id, Recorder, StoreError};
use crate::tracker::{Tracker, TrackerConfig};

pub struct Pipeline {
    tracker: Tracker,
    recorder: Recorder,
    pending: Vec<Fact>,
}

impl Pipeline {
    /// Uses the stored profile as "me" unless the config already names one.
    pub fn new(conn: Connection, mut config: TrackerConfig) -> Result<Self, StoreError> {
        if config.me_primary_id.is_none() {
            config.me_primary_id = profile_primary_id(&conn)?;
        }
        Ok(Self {
            tracker: Tracker::new(config),
            recorder: Recorder::new(conn),
            pending: Vec::new(),
        })
    }

    pub fn handle(&mut self, msg: &IngestMsg) -> Result<Vec<Fact>, StoreError> {
        let facts = match msg {
            IngestMsg::Event(event) => self.tracker.handle(event),
            IngestMsg::Connected { at_ms } => self.tracker.connection_restored(*at_ms),
            IngestMsg::Disconnected { at_ms } => {
                self.tracker.connection_lost(*at_ms);
                Vec::new()
            }
            IngestMsg::DecodeFailed { .. } => Vec::new(),
        };
        self.persist(facts)
    }

    pub fn tick(&mut self, now_ms: i64) -> Result<Vec<Fact>, StoreError> {
        let facts = self.tracker.tick(now_ms);
        self.persist(facts)
    }

    pub fn game_exited(&mut self, at_ms: i64) -> Result<Vec<Fact>, StoreError> {
        let facts = self.tracker.game_exited(at_ms);
        self.persist(facts)
    }

    pub fn open_session_id(&self) -> Option<i64> {
        self.recorder.cursor().session_id
    }

    pub fn open_block_id(&self) -> Option<i64> {
        self.recorder.cursor().block_id
    }

    /// Saves queued facts so commands see the same rows the tracker believes in.
    fn flush(&mut self) -> Result<(), StoreError> {
        self.persist(Vec::new()).map(|_| ())
    }

    /// Like `flush`, but first applies the idle timeout as of `now_ms`, so a command arriving
    /// between the timeout and the next tick never acts on an expired session.
    fn flush_at(&mut self, now_ms: i64) -> Result<(), StoreError> {
        let facts = self.tracker.tick(now_ms);
        self.persist(facts).map(|_| ())
    }

    pub fn set_block_pack(
        &mut self,
        block_id: i64,
        pack_id: Option<i64>,
        now_ms: i64,
    ) -> Result<Vec<Fact>, EditError> {
        self.flush_at(now_ms)?;
        if let Some(id) = pack_id {
            if edits::pack(self.recorder.conn(), id)?.is_none() {
                return Err(EditError::NotFound("pack"));
            }
        }
        let cursor = self.recorder.cursor();
        if cursor.block_id != Some(block_id) {
            edits::set_closed_block_pack(self.recorder.conn(), block_id, pack_id)?;
            return Ok(Vec::new());
        }
        if cursor.block_kind != Some(Activity::Training) {
            return Err(EditError::Invalid(
                "only training activities have a pack".into(),
            ));
        }
        if cursor.block_pack == pack_id {
            return Ok(Vec::new());
        }
        if cursor.block_attempts == 0 {
            self.recorder.set_open_block_pack(pack_id)?;
            return Ok(Vec::new());
        }
        let facts = self.tracker.split_block(now_ms, pack_id);
        Ok(self.persist(facts)?)
    }

    pub fn set_attempt_skipped(
        &mut self,
        attempt_id: i64,
        skipped: bool,
        now_ms: i64,
    ) -> Result<(), EditError> {
        self.flush_at(now_ms)?;
        let open = self.recorder.cursor().session_id;
        edits::set_attempt_skipped(self.recorder.conn(), attempt_id, skipped, open)
    }

    pub fn add_mmr(
        &mut self,
        team_size: u8,
        mmr: i32,
        session_id: Option<i64>,
        now_ms: i64,
    ) -> Result<i64, EditError> {
        self.flush_at(now_ms)?;
        edits::add_mmr(self.recorder.conn(), team_size, mmr, session_id, now_ms)
    }

    pub fn set_session_notes(&mut self, session_id: i64, notes: &str) -> Result<String, EditError> {
        self.flush()?;
        edits::set_session_notes(self.recorder.conn(), session_id, notes)
    }

    pub fn review_session(
        &mut self,
        session_id: i64,
        mmr: &[MmrInput],
        notes: Option<&str>,
        now_ms: i64,
    ) -> Result<SessionSummary, EditError> {
        self.flush_at(now_ms)?;
        edits::review_session(self.recorder.conn(), session_id, mmr, notes, now_ms)?;
        let conn = self.recorder.conn();
        match live_q::session_summary(conn, session_id)? {
            Some(summary) => Ok(summary),
            None => {
                live_q::current_session(conn, session_id)?.ok_or(EditError::NotFound("session"))
            }
        }
    }

    pub fn set_limits(&mut self, session_timeout_ms: i64, hard_hit_kmh: f64) {
        self.tracker.set_limits(session_timeout_ms, hard_hit_kmh);
    }

    /// Runs a desktop edit on the recording connection after saving what is due, so the edit
    /// sees the same state the live view shows.
    pub fn with_conn<T>(
        &mut self,
        now_ms: i64,
        f: impl FnOnce(&Connection) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        self.flush_at(now_ms)?;
        f(self.recorder.conn())
    }

    /// Live state for the UIs. `day_start_ms` is local midnight (for today's record).
    pub fn live_snapshot(&self, day_start_ms: i64) -> Result<LiveSnapshot, StoreError> {
        let cursor = self.recorder.cursor();
        let conn = self.recorder.conn();
        let session = match cursor.session_id {
            Some(id) => live_q::session(conn, id)?,
            None => None,
        };
        let block = match (cursor.session_id, cursor.block_id, self.tracker.live()) {
            (Some(session_id), Some(block_id), Some(t)) => {
                let pack = match cursor.block_pack {
                    Some(id) => edits::pack(conn, id)?,
                    None => None,
                };
                let is_training = t.activity == Activity::Training;
                let suggested_pack = if is_training && pack.is_none() {
                    live_q::suggested_pack(conn, session_id, block_id)?
                } else {
                    None
                };
                let training = if is_training {
                    Some(live_q::training(conn, block_id)?)
                } else {
                    None
                };
                let match_live = match t.match_live {
                    Some(mut m) => {
                        (m.today_wins, m.today_losses) =
                            live_q::today_record(conn, m.playlist_id, day_start_ms)?;
                        Some(m)
                    }
                    None => None,
                };
                let freeplay = match t.freeplay {
                    Some(mut f) => {
                        if let Some(best) = live_q::best_freeplay_hit(conn, day_start_ms)? {
                            f.best_today_hit = f.best_today_hit.max(best);
                        }
                        Some(f)
                    }
                    None => None,
                };
                Some(LiveBlock {
                    id: block_id,
                    kind: t.activity,
                    playlist_id: t.playlist_id,
                    started_at: t.started_at_ms,
                    paused: t.paused,
                    pack,
                    suggested_pack,
                    training,
                    freeplay,
                    match_live,
                })
            }
            _ => None,
        };
        Ok(LiveSnapshot {
            session,
            block,
            latest_mmr: live_q::latest_mmr(conn)?,
            last_session: live_q::last_session(conn)?,
            current_session: match cursor.session_id {
                Some(id) => live_q::current_session(conn, id)?,
                None => None,
            },
        })
    }

    pub fn heartbeat(&mut self, now_ms: i64) -> Result<(), StoreError> {
        self.flush()?;
        self.recorder.touch_session(now_ms)
    }

    pub fn recorder(&self) -> &Recorder {
        &self.recorder
    }

    /// Facts not yet saved because a write failed; retried before any newer fact.
    pub fn pending_facts(&self) -> usize {
        self.pending.len()
    }

    /// Saves the tracker's facts in order. If the write fails, the facts stay queued (the tracker
    /// has already moved on) and are retried, together with newer ones, on the next call.
    fn persist(&mut self, facts: Vec<Fact>) -> Result<Vec<Fact>, StoreError> {
        self.pending.extend(facts.iter().cloned());
        if !self.pending.is_empty() {
            self.recorder.apply_all(&self.pending)?;
            self.pending.clear();
        }
        Ok(facts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{
        BallHit, GameEvent, GameState, GoalScored, HitBall, PlayerRef, PlayerState, TimedEvent,
        UpdateState,
    };
    use crate::store::edits::MmrInput;
    use crate::store::open_in_memory;
    use crate::store::EditError;

    fn freeplay_update(at_ms: i64) -> IngestMsg {
        let me = PlayerState {
            name: "Me".into(),
            primary_id: "Epic|me|0".into(),
            speed: 40.0,
            ..Default::default()
        };
        IngestMsg::Event(TimedEvent {
            at_ms,
            event: GameEvent::UpdateState(UpdateState {
                players: vec![me],
                game: GameState {
                    playlist_id: Some(9),
                    has_target: true,
                    target: Some(PlayerRef {
                        name: "Me".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
        })
    }

    fn count(p: &Pipeline, table: &str) -> i64 {
        p.recorder()
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn uses_the_stored_profile() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO profile (primary_id, display_name, created_at, is_active)
             VALUES ('Epic|me|0', 'Me', 0, 1)",
            [],
        )
        .unwrap();
        let mut p = Pipeline::new(conn, TrackerConfig::default()).unwrap();
        let facts = p.handle(&freeplay_update(0)).unwrap();
        assert!(!facts
            .iter()
            .any(|f| matches!(f, Fact::ProfileDetected { .. })));
    }

    #[test]
    fn records_events_and_survives_decode_failures() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&IngestMsg::Connected { at_ms: 0 }).unwrap();
        p.handle(&freeplay_update(10)).unwrap();
        p.handle(&IngestMsg::DecodeFailed {
            at_ms: 20,
            error: "bad".into(),
        })
        .unwrap();
        p.handle(&IngestMsg::Disconnected { at_ms: 1_000 }).unwrap();
        p.handle(&IngestMsg::Connected { at_ms: 4_000 }).unwrap();
        p.game_exited(5_000).unwrap();
        assert_eq!(count(&p, "profile"), 1);
        assert_eq!(count(&p, "freeplay_stats"), 1);
        let gap: i64 = p
            .recorder()
            .conn()
            .query_row("SELECT data_gap_seconds FROM block", [], |r| r.get(0))
            .unwrap();
        assert_eq!(gap, 3);
    }

    #[test]
    fn facts_that_fail_to_save_are_retried_with_the_next_message() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        // Simulate a transient write failure (e.g. SQLITE_BUSY during a backup).
        p.recorder()
            .conn()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_block BEFORE INSERT ON block
                 BEGIN SELECT RAISE(ABORT, 'busy'); END;",
            )
            .unwrap();
        assert!(p.handle(&freeplay_update(0)).is_err());
        p.recorder()
            .conn()
            .execute_batch("DROP TRIGGER fail_block;")
            .unwrap();
        p.handle(&freeplay_update(100)).unwrap();
        p.game_exited(200).unwrap();
        assert_eq!(count(&p, "session"), 1);
        assert_eq!(count(&p, "block"), 1);
        assert_eq!(count(&p, "freeplay_stats"), 1);
    }

    #[test]
    fn tick_closes_idle_sessions_in_the_database() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&freeplay_update(0)).unwrap();
        // The car moves after free play opens; free play it never moved in is dropped.
        p.handle(&freeplay_update(1_000)).unwrap();
        p.tick(31 * 60 * 1000).unwrap();
        let ended: Option<i64> = p
            .recorder()
            .conn()
            .query_row("SELECT ended_at FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ended, Some(1_000));
    }

    fn me_player() -> PlayerState {
        PlayerState {
            name: "Me".into(),
            primary_id: "Epic|me|0".into(),
            ..Default::default()
        }
    }

    fn training(at_ms: i64, time: f64) -> IngestMsg {
        IngestMsg::Event(TimedEvent {
            at_ms,
            event: GameEvent::UpdateState(UpdateState {
                players: vec![me_player()],
                game: GameState {
                    playlist_id: Some(21),
                    time_seconds: time,
                    ..Default::default()
                },
                ..Default::default()
            }),
        })
    }

    fn event(at_ms: i64, event: GameEvent) -> IngestMsg {
        IngestMsg::Event(TimedEvent { at_ms, event })
    }

    fn goal() -> GameEvent {
        GameEvent::GoalScored(GoalScored {
            goal_speed: 80.0,
            scorer: Some(PlayerRef {
                name: "Me".into(),
                ..Default::default()
            }),
        })
    }

    fn feed(p: &mut Pipeline, msgs: Vec<IngestMsg>) {
        for m in msgs {
            p.handle(&m).unwrap();
        }
    }

    /// One recorded make (seq 1) and a second attempt in progress.
    fn one_make_then_in_progress(p: &mut Pipeline) {
        feed(
            p,
            vec![
                training(0, 10.0),
                event(100, GameEvent::RoundStarted),
                training(1_100, 9.0),
                training(2_100, 8.0),
                event(2_500, goal()),
                event(3_000, GameEvent::RoundStarted),
                training(4_000, 10.0),
                training(5_000, 9.0),
            ],
        );
    }

    fn rows(p: &Pipeline, sql: &str) -> Vec<(i64, Option<i64>)> {
        let mut stmt = p.recorder().conn().prepare(sql).unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn changing_pack_after_attempts_splits_the_training_block() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        one_make_then_in_progress(&mut p);
        let first = p.open_block_id().unwrap();
        let facts = p.set_block_pack(first, Some(1), 5_500).unwrap();
        assert!(matches!(facts[0], Fact::BlockEnded { at_ms: 5_500, .. }));
        assert!(matches!(
            facts[1],
            Fact::BlockStarted {
                pack_id: Some(1),
                ..
            }
        ));
        let second = p.open_block_id().unwrap();
        assert_ne!(first, second);
        feed(
            &mut p,
            vec![training(6_000, 8.0), event(7_000, GameEvent::RoundStarted)],
        );
        p.game_exited(8_000).unwrap();
        assert_eq!(
            rows(&p, "SELECT id, pack_id FROM block ORDER BY id"),
            [(first, None), (second, Some(1))]
        );
        assert_eq!(
            rows(
                &p,
                "SELECT block_id, COUNT(*) FROM training_attempt GROUP BY block_id ORDER BY block_id"
            ),
            [(first, Some(1)), (second, Some(1))],
            "the attempt in progress moved to the new block"
        );
    }

    #[test]
    fn pack_on_a_block_without_attempts_is_set_in_place() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        feed(&mut p, vec![training(0, 10.0)]);
        let block = p.open_block_id().unwrap();
        assert!(p.set_block_pack(block, Some(2), 100).unwrap().is_empty());
        assert!(
            p.set_block_pack(block, Some(2), 150).unwrap().is_empty(),
            "same pack: no-op"
        );
        assert_eq!(p.open_block_id(), Some(block));
        feed(
            &mut p,
            vec![
                event(200, GameEvent::RoundStarted),
                training(1_200, 9.0),
                event(1_500, goal()),
                event(2_000, GameEvent::RoundStarted),
            ],
        );
        p.game_exited(3_000).unwrap();
        assert_eq!(
            rows(&p, "SELECT id, pack_id FROM block"),
            [(block, Some(2))]
        );
    }

    #[test]
    fn invalid_pack_targets_are_rejected() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&freeplay_update(0)).unwrap();
        let block = p.open_block_id().unwrap();
        assert!(matches!(
            p.set_block_pack(block, Some(1), 10),
            Err(EditError::Invalid(_))
        ));
        assert!(matches!(
            p.set_block_pack(999, Some(1), 10),
            Err(EditError::NotFound("activity"))
        ));
        assert!(matches!(
            p.set_block_pack(block, Some(999), 10),
            Err(EditError::NotFound("pack"))
        ));
    }

    #[test]
    fn commands_save_queued_facts_first_or_write_nothing() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.recorder()
            .conn()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_block BEFORE INSERT ON block
                 BEGIN SELECT RAISE(ABORT, 'busy'); END;",
            )
            .unwrap();
        assert!(p.handle(&freeplay_update(0)).is_err());
        assert!(matches!(
            p.add_mmr(2, 900, None, 0),
            Err(EditError::Store(_))
        ));
        assert_eq!(count(&p, "mmr_entry"), 0);
        p.recorder()
            .conn()
            .execute_batch("DROP TRIGGER fail_block;")
            .unwrap();
        p.add_mmr(2, 900, None, 0).unwrap();
        assert_eq!(count(&p, "block"), 1, "queued facts were saved first");
        assert!(p.open_block_id().is_some());
    }

    #[test]
    fn skip_reaches_only_attempts_of_the_open_session() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        one_make_then_in_progress(&mut p);
        let old: i64 = p
            .recorder()
            .conn()
            .query_row("SELECT id FROM training_attempt", [], |r| r.get(0))
            .unwrap();
        p.tick(40 * 60 * 1000).unwrap(); // session times out
        feed(&mut p, vec![training(41 * 60 * 1000, 10.0)]);
        assert!(matches!(
            p.set_attempt_skipped(old, true, 41 * 60 * 1000),
            Err(EditError::Conflict(_))
        ));
    }

    #[test]
    fn skip_after_the_timeout_conflicts_even_before_the_next_tick() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        one_make_then_in_progress(&mut p);
        let old: i64 = p
            .recorder()
            .conn()
            .query_row("SELECT id FROM training_attempt", [], |r| r.get(0))
            .unwrap();
        // No tick between the timeout and the tap: the command must still see the session closed.
        assert!(matches!(
            p.set_attempt_skipped(old, true, 40 * 60 * 1000),
            Err(EditError::Conflict(_))
        ));
        assert_eq!(p.open_session_id(), None);
    }

    #[test]
    fn pack_change_after_the_timeout_does_not_split_an_expired_block() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        one_make_then_in_progress(&mut p);
        let block = p.open_block_id().unwrap();
        p.set_block_pack(block, Some(3), 40 * 60 * 1000).unwrap();
        let (block_end, session_end): (i64, i64) = p
            .recorder()
            .conn()
            .query_row(
                "SELECT b.ended_at, s.ended_at FROM block b JOIN session s ON s.id = b.session_id WHERE b.id = ?1",
                [block],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(block_end <= session_end, "block outlives its session");
        assert_eq!(p.open_session_id(), None);
    }

    #[test]
    fn live_snapshot_reports_training_progress_and_suggests_the_last_pack() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        one_make_then_in_progress(&mut p);
        let first = p.open_block_id().unwrap();
        p.set_block_pack(first, Some(3), 4_500).unwrap(); // has 1 attempt: splits
        let snap = p.live_snapshot(0).unwrap();
        let block = snap.block.clone().unwrap();
        assert_eq!(block.pack.as_ref().map(|pk| pk.id), Some(3));
        assert!(block.suggested_pack.is_none(), "a pack is set");
        let progress = block.training.unwrap();
        assert_eq!((progress.attempts, progress.makes), (0, 0));
        assert!(snap.session.is_some());

        // Reloading the pack destroys the training match: the next block starts unassigned.
        feed(
            &mut p,
            vec![
                training(6_000, 8.0),
                event(7_000, GameEvent::RoundStarted),
                event(7_500, GameEvent::MatchDestroyed),
                training(8_000, 10.0),
            ],
        );
        let snap = p.live_snapshot(0).unwrap();
        let block = snap.block.unwrap();
        assert!(block.pack.is_none());
        assert_eq!(block.suggested_pack.map(|pk| pk.id), Some(3));
        let json = serde_json::to_value(p.live_snapshot(0).unwrap()).unwrap();
        assert_eq!(json["block"]["kind"], "training");
        assert!(
            json["block"].get("match").is_some(),
            "Plan 3 contract uses `match`"
        );
    }

    #[test]
    fn live_snapshot_has_last_session_and_latest_mmr() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&freeplay_update(0)).unwrap();
        p.game_exited(60_000).unwrap();
        p.add_mmr(2, 880, None, 1).unwrap();
        p.add_mmr(2, 899, None, 2).unwrap();
        p.add_mmr(1, 700, None, 3).unwrap();
        let snap = p.live_snapshot(0).unwrap();
        assert!(snap.session.is_none() && snap.block.is_none());
        let last = snap.last_session.unwrap();
        assert_eq!((last.freeplay_blocks, last.ended_at), (1, 60_000));
        assert_eq!(
            snap.latest_mmr
                .iter()
                .map(|m| (m.team_size, m.mmr))
                .collect::<Vec<_>>(),
            [(1, 700), (2, 899)]
        );
    }

    #[test]
    fn heartbeat_marks_the_open_session() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.heartbeat(5).unwrap(); // no session: nothing to do
        p.handle(&freeplay_update(0)).unwrap();
        p.heartbeat(30_000).unwrap();
        let seen: Option<i64> = p
            .recorder()
            .conn()
            .query_row("SELECT last_seen_at FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(seen, Some(30_000));
    }

    fn my_hit(at_ms: i64, speed: f64) -> IngestMsg {
        IngestMsg::Event(TimedEvent {
            at_ms,
            event: GameEvent::BallHit(BallHit {
                players: vec![PlayerRef {
                    name: "Me".into(),
                    ..Default::default()
                }],
                ball: HitBall {
                    pre_hit_speed: 0.0,
                    post_hit_speed: speed,
                    location: None,
                },
            }),
        })
    }

    #[test]
    fn free_play_shows_the_hardest_hit_of_the_day() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&freeplay_update(0)).unwrap();
        p.handle(&my_hit(500, 110.0)).unwrap();
        p.handle(&IngestMsg::Event(TimedEvent {
            at_ms: 1_000,
            event: GameEvent::MatchDestroyed,
        }))
        .unwrap();
        p.handle(&freeplay_update(2_000)).unwrap();
        p.handle(&my_hit(2_500, 90.0)).unwrap();
        let f = p.live_snapshot(0).unwrap().block.unwrap().freeplay.unwrap();
        assert_eq!((f.max_hit_speed, f.best_today_hit), (90.0, 110.0));
    }

    #[test]
    fn a_finished_session_can_be_reviewed_through_the_pipeline() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&freeplay_update(0)).unwrap();
        p.game_exited(1_000).unwrap();
        let id = p.live_snapshot(0).unwrap().last_session.unwrap().id;
        let summary = p
            .review_session(
                id,
                &[MmrInput {
                    team_size: 2,
                    mmr: 900,
                }],
                Some("ok"),
                2_000,
            )
            .unwrap();
        assert!(summary.reviewed);
        assert_eq!(summary.notes, "ok");
        assert_eq!(p.live_snapshot(0).unwrap().latest_mmr[0].mmr, 900);
    }

    #[test]
    fn the_open_session_is_summarised_and_reviewable_in_the_menus() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        p.handle(&freeplay_update(0)).unwrap();
        p.handle(&IngestMsg::Event(TimedEvent {
            at_ms: 1_000,
            event: GameEvent::MatchDestroyed,
        }))
        .unwrap();
        let snap = p.live_snapshot(0).unwrap();
        assert!(snap.block.is_none() && snap.last_session.is_none());
        let current = snap.current_session.unwrap();
        assert_eq!((current.ended_at, current.reviewed), (1_000, false));
        let summary = p
            .review_session(current.id, &[], Some("mid-session"), 2_000)
            .unwrap();
        assert!(summary.reviewed);
        let current = p.live_snapshot(0).unwrap().current_session.unwrap();
        assert_eq!(
            (current.reviewed, current.notes.as_str()),
            (true, "mid-session")
        );
    }

    #[test]
    fn edits_run_on_the_recording_connection() {
        let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
        let packs = p
            .with_conn(0, |conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM pack", [], |r| r.get::<_, i64>(0))?)
            })
            .unwrap();
        assert_eq!(packs, 11);
    }
}
