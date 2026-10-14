//! Persists tracker facts. Keeps a cursor on the rows it is writing to.

use rusqlite::{params, Connection};

use crate::facts::Fact;
use crate::playlist::Activity;
use crate::store::scoring::{is_goalie, rescore_block, scored};
use crate::store::StoreError;

/// What the recorder knows about the rows it is currently writing to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub session_id: Option<i64>,
    pub block_id: Option<i64>,
    pub block_kind: Option<Activity>,
    pub block_pack: Option<i64>,
    pub block_attempts: u32,
    pub block_started_at: i64,
    /// A match record was written for the open match activity.
    pub block_has_match: bool,
    pub session_blocks: u32,
    pub last_attempt_id: Option<i64>,
}

pub struct Recorder {
    conn: Connection,
    cursor: Cursor,
}

impl Recorder {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            cursor: Cursor::default(),
        }
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Applies a batch atomically: on any failure nothing is written and the cursor is restored,
    /// so the caller can retry the same batch later.
    pub fn apply_all(&mut self, facts: &[Fact]) -> Result<(), StoreError> {
        let saved = self.cursor;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = facts
            .iter()
            .try_for_each(|fact| self.apply(fact))
            .and_then(|()| self.conn.execute_batch("COMMIT").map_err(StoreError::from));
        if let Err(err) = result {
            // The original error is the one worth reporting; a rollback failure leaves SQLite
            // to roll back on its own when the connection closes.
            if let Err(rollback) = self.conn.execute_batch("ROLLBACK") {
                eprintln!("rl-core: rollback after failed write also failed: {rollback}");
            }
            self.cursor = saved;
            return Err(err);
        }
        Ok(())
    }

    /// Assigns a pack to the open block (used when it has no attempts yet).
    pub fn set_open_block_pack(&mut self, pack_id: Option<i64>) -> Result<(), StoreError> {
        let block_id = self.cursor.block_id.ok_or(StoreError::NoOpenBlock)?;
        let was_goalie = is_goalie(&self.conn, self.cursor.block_pack)?;
        let goalie = is_goalie(&self.conn, pack_id)?;
        self.conn.execute(
            "UPDATE block SET pack_id = ?1 WHERE id = ?2",
            params![pack_id, block_id],
        )?;
        rescore_block(&self.conn, block_id, was_goalie, goalie)?;
        self.cursor.block_pack = pack_id;
        Ok(())
    }

    /// Records that the open session was alive at `at_ms` (crash recovery closes it there).
    pub fn touch_session(&mut self, at_ms: i64) -> Result<(), StoreError> {
        if let Some(id) = self.cursor.session_id {
            self.conn.execute(
                "UPDATE session SET last_seen_at = ?1 WHERE id = ?2",
                params![at_ms, id],
            )?;
        }
        Ok(())
    }

    pub fn apply(&mut self, fact: &Fact) -> Result<(), StoreError> {
        match fact {
            Fact::ProfileDetected {
                at_ms,
                primary_id,
                display_name,
            } => {
                self.conn.execute(
                    "INSERT INTO profile (primary_id, display_name, created_at, is_active)
                     VALUES (?1, ?2, ?3, NOT EXISTS (SELECT 1 FROM profile WHERE is_active = 1))
                     ON CONFLICT(primary_id) DO UPDATE SET display_name = excluded.display_name",
                    params![primary_id, display_name, at_ms],
                )?;
            }
            Fact::SessionStarted { at_ms } => {
                self.conn.execute(
                    "INSERT INTO session (started_at) VALUES (?1)",
                    params![at_ms],
                )?;
                self.cursor = Cursor {
                    session_id: Some(self.conn.last_insert_rowid()),
                    ..Cursor::default()
                };
            }
            Fact::SessionEnded { at_ms } => {
                let session_id = self.cursor.session_id.ok_or(StoreError::NoOpenSession)?;
                if self.cursor.session_blocks == 0 {
                    // Nothing was recorded (e.g. only an empty training block): keep no trace.
                    self.conn
                        .execute("DELETE FROM session WHERE id = ?1", params![session_id])?;
                } else {
                    self.conn.execute(
                        "UPDATE session SET ended_at = ?1 WHERE id = ?2",
                        params![at_ms, session_id],
                    )?;
                }
                self.cursor = Cursor::default();
            }
            Fact::BlockStarted {
                at_ms,
                activity,
                playlist_id,
                pack_id,
            } => {
                let session_id = self.cursor.session_id.ok_or(StoreError::NoOpenSession)?;
                self.conn.execute(
                    "INSERT INTO block (session_id, kind, playlist_id, pack_id, started_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![session_id, activity.as_str(), playlist_id, pack_id, at_ms],
                )?;
                self.cursor.block_id = Some(self.conn.last_insert_rowid());
                self.cursor.block_kind = Some(*activity);
                self.cursor.block_pack = *pack_id;
                self.cursor.block_started_at = *at_ms;
                self.cursor.block_has_match = false;
                self.cursor.block_attempts = 0;
                self.cursor.session_blocks += 1;
            }
            Fact::BlockEnded {
                at_ms,
                data_gap_seconds,
            } => {
                let block_id = self.cursor.block_id.take().ok_or(StoreError::NoOpenBlock)?;
                let kind = self.cursor.block_kind.take();
                let empty_training =
                    kind == Some(Activity::Training) && self.cursor.block_attempts == 0;
                // Free play closed for idleness ends where the car last moved: at its start when it
                // never moved after opening (loaded in, then walked away).
                let still_free_play =
                    kind == Some(Activity::Freeplay) && *at_ms <= self.cursor.block_started_at;
                // The tracker writes no record for the seconds between two games of a lobby.
                let not_a_match = kind == Some(Activity::Match) && !self.cursor.block_has_match;
                if empty_training || still_free_play || not_a_match {
                    // A pack reload or a mid-training connect leaves a block with no attempts.
                    self.conn
                        .execute("DELETE FROM block WHERE id = ?1", params![block_id])?;
                    self.cursor.session_blocks = self.cursor.session_blocks.saturating_sub(1);
                } else {
                    self.conn.execute(
                        "UPDATE block SET ended_at = ?1, data_gap_seconds = ?2 WHERE id = ?3",
                        params![at_ms, data_gap_seconds, block_id],
                    )?;
                }
                self.cursor.block_pack = None;
                self.cursor.block_attempts = 0;
            }
            Fact::AttemptFinished(a) => {
                let block_id = self.cursor.block_id.ok_or(StoreError::NoOpenBlock)?;
                let outcome = scored(a.outcome, is_goalie(&self.conn, self.cursor.block_pack)?);
                self.conn.execute(
                    "INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome,
                         retry_after_make, goal_speed)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        block_id,
                        a.seq,
                        a.started_at_ms,
                        a.duration_ms,
                        outcome.as_str(),
                        a.retry_after_make,
                        a.goal_speed
                    ],
                )?;
                self.cursor.last_attempt_id = Some(self.conn.last_insert_rowid());
                self.cursor.block_attempts += 1;
            }
            Fact::FreeplayFinished(s) => {
                let block_id = self.cursor.block_id.ok_or(StoreError::NoOpenBlock)?;
                self.conn.execute(
                    "INSERT INTO freeplay_stats (block_id, touches, hard_hits, avg_hit_speed,
                         max_hit_speed, goals, crossbar_hits, pct_supersonic, pct_on_wall,
                         pct_airborne)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        block_id,
                        s.touches,
                        s.hard_hits,
                        s.avg_hit_speed,
                        s.max_hit_speed,
                        s.goals,
                        s.crossbar_hits,
                        s.pct_supersonic,
                        s.pct_on_wall,
                        s.pct_airborne
                    ],
                )?;
            }
            Fact::MatchFinished(m) => {
                let block_id = self.cursor.block_id.ok_or(StoreError::NoOpenBlock)?;
                self.conn.execute(
                    "INSERT INTO match_record (block_id, match_guid, playlist_id, ranked, team_size,
                         arena, result, team_score, opponent_score, overtime, score, goals,
                         assists, saves, shots, touches, demos, epic_saves, avg_boost,
                         pct_zero_boost, pct_supersonic, metrics_partial, scoreboard_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                         ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
                     ON CONFLICT(match_guid) DO UPDATE SET
                         -- Same match finished twice (rejoin after a crash or disconnect).
                         -- Scoreboard stats are cumulative, so the fragment with more touches
                         -- is the later, more complete one; a known result always wins.
                         block_id = CASE WHEN excluded.touches >= match_record.touches
                             THEN excluded.block_id ELSE match_record.block_id END,
                         result = CASE WHEN excluded.result = 'unknown'
                             THEN match_record.result ELSE excluded.result END,
                         team_score = MAX(match_record.team_score, excluded.team_score),
                         opponent_score = MAX(match_record.opponent_score, excluded.opponent_score),
                         overtime = MAX(match_record.overtime, excluded.overtime),
                         score = MAX(match_record.score, excluded.score),
                         goals = MAX(match_record.goals, excluded.goals),
                         assists = MAX(match_record.assists, excluded.assists),
                         saves = MAX(match_record.saves, excluded.saves),
                         shots = MAX(match_record.shots, excluded.shots),
                         touches = MAX(match_record.touches, excluded.touches),
                         demos = MAX(match_record.demos, excluded.demos),
                         epic_saves = match_record.epic_saves + excluded.epic_saves,
                         scoreboard_json = CASE WHEN excluded.touches >= match_record.touches
                             THEN excluded.scoreboard_json ELSE match_record.scoreboard_json END,
                         metrics_partial = 1",
                    params![
                        block_id,
                        m.match_guid,
                        m.playlist_id,
                        m.ranked,
                        m.team_size,
                        m.arena,
                        m.result.as_str(),
                        m.team_score,
                        m.opponent_score,
                        m.overtime,
                        m.score,
                        m.goals,
                        m.assists,
                        m.saves,
                        m.shots,
                        m.touches,
                        m.demos,
                        m.epic_saves,
                        m.avg_boost,
                        m.pct_zero_boost,
                        m.pct_supersonic,
                        m.metrics_partial,
                        m.scoreboard_json
                    ],
                )?;
                if let Some(x) = &m.metrics {
                    // A rejoined match keeps the block of the fragment with more touches (the
                    // merge above); only that fragment's stats are written. A winning fragment
                    // without metrics keeps the earlier fragment's; the upsert above marked it
                    // partial.
                    self.conn.execute(
                        "UPDATE match_record SET play_seconds = ?3, hit_avg_speed = ?4,
                             hard_hits = ?5, aerial_touches = ?6, touches_def = ?7,
                             touches_mid = ?8, touches_att = ?9, pct_on_ground = ?10,
                             pct_on_wall = ?11, pct_airborne = ?12, avg_speed = ?13,
                             pct_boosting = ?14, pct_full_boost = ?15, boost_used = ?16,
                             big_pads = ?17, goal_speed_avg = ?18, demos_taken = ?19,
                             pct_possession = ?20
                         WHERE match_guid = ?1 AND block_id = ?2",
                        params![
                            m.match_guid,
                            block_id,
                            x.play_seconds,
                            x.hit_avg_speed,
                            x.hard_hits,
                            x.aerial_touches,
                            x.touches_def,
                            x.touches_mid,
                            x.touches_att,
                            x.pct_on_ground,
                            x.pct_on_wall,
                            x.pct_airborne,
                            x.avg_speed,
                            x.pct_boosting,
                            x.pct_full_boost,
                            x.boost_used,
                            x.big_pads,
                            x.goal_speed_avg,
                            x.demos_taken,
                            x.pct_possession
                        ],
                    )?;
                }
                self.cursor.block_has_match = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{
        Attempt, AttemptOutcome, FreeplayStats, MatchMetrics, MatchRecord, MatchResult,
    };
    use crate::playlist::Activity;
    use crate::store::{list_sessions, open_in_memory, profile_primary_id};

    fn recorder() -> Recorder {
        Recorder::new(open_in_memory().unwrap())
    }

    fn attempt(seq: u32, outcome: AttemptOutcome) -> Fact {
        Fact::AttemptFinished(Attempt {
            seq,
            started_at_ms: i64::from(seq) * 1_000,
            duration_ms: 3_000,
            outcome,
            retry_after_make: false,
            goal_speed: None,
        })
    }

    fn sample_match(guid: &str) -> Fact {
        Fact::MatchFinished(MatchRecord {
            match_guid: guid.into(),
            playlist_id: Some(2),
            team_size: Some(2),
            arena: "Stadium_P".into(),
            result: MatchResult::Win,
            scoreboard_json: "[]".into(),
            ..Default::default()
        })
    }

    fn apply_all(r: &mut Recorder, facts: &[Fact]) {
        for f in facts {
            r.apply(f).unwrap();
        }
    }

    #[test]
    fn records_a_full_session() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::ProfileDetected {
                    at_ms: 0,
                    primary_id: "Epic|me|0".into(),
                    display_name: "Me".into(),
                },
                Fact::SessionStarted { at_ms: 0 },
                Fact::BlockStarted {
                    at_ms: 0,
                    activity: Activity::Training,
                    playlist_id: Some(21),
                    pack_id: None,
                },
                attempt(1, AttemptOutcome::Make),
                attempt(2, AttemptOutcome::Miss),
                attempt(3, AttemptOutcome::Make),
                Fact::BlockEnded {
                    at_ms: 5_000,
                    data_gap_seconds: 0,
                },
                Fact::BlockStarted {
                    at_ms: 5_000,
                    activity: Activity::Freeplay,
                    playlist_id: Some(9),
                    pack_id: None,
                },
                Fact::FreeplayFinished(FreeplayStats {
                    touches: 12,
                    ..Default::default()
                }),
                Fact::BlockEnded {
                    at_ms: 9_000,
                    data_gap_seconds: 2,
                },
                Fact::BlockStarted {
                    at_ms: 9_000,
                    activity: Activity::Match,
                    playlist_id: Some(2),
                    pack_id: None,
                },
                sample_match("G1"),
                Fact::BlockEnded {
                    at_ms: 20_000,
                    data_gap_seconds: 0,
                },
                Fact::SessionEnded { at_ms: 20_000 },
            ],
        );
        let sessions = list_sessions(r.conn()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].ended_at_ms, Some(20_000));
        let kinds: Vec<_> = sessions[0].blocks.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, ["training", "freeplay", "match"]);
        let training = &sessions[0].blocks[0];
        assert_eq!((training.attempts, training.makes), (3, 2));
        assert!((training.make_rate().unwrap() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(sessions[0].blocks[1].make_rate(), None);
        assert_eq!(sessions[0].blocks[2].match_result.as_deref(), Some("win"));
        assert_eq!(
            profile_primary_id(r.conn()).unwrap().as_deref(),
            Some("Epic|me|0")
        );
    }

    #[test]
    fn skipped_attempts_are_excluded_from_make_rate() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                Fact::BlockStarted {
                    at_ms: 0,
                    activity: Activity::Training,
                    playlist_id: Some(21),
                    pack_id: None,
                },
                attempt(1, AttemptOutcome::Make),
                attempt(2, AttemptOutcome::Miss),
            ],
        );
        r.conn()
            .execute("UPDATE training_attempt SET skipped = 1 WHERE seq = 2", [])
            .unwrap();
        let block = &list_sessions(r.conn()).unwrap()[0].blocks[0];
        assert_eq!(block.make_rate(), Some(1.0));
    }

    #[test]
    fn duplicate_match_guid_is_ignored() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                Fact::BlockStarted {
                    at_ms: 0,
                    activity: Activity::Match,
                    playlist_id: Some(2),
                    pack_id: None,
                },
                sample_match("G1"),
                Fact::BlockEnded {
                    at_ms: 1,
                    data_gap_seconds: 0,
                },
                Fact::BlockStarted {
                    at_ms: 2,
                    activity: Activity::Match,
                    playlist_id: Some(2),
                    pack_id: None,
                },
                sample_match("G1"),
            ],
        );
        let n: i64 = r
            .conn()
            .query_row("SELECT COUNT(*) FROM match_record", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    fn fragment(result: MatchResult, score: i32, touches: i32) -> Fact {
        Fact::MatchFinished(MatchRecord {
            match_guid: "G1".into(),
            playlist_id: Some(11),
            team_size: Some(2),
            arena: "Stadium_P".into(),
            result,
            score,
            touches,
            scoreboard_json: format!("[{score}]"),
            ..Default::default()
        })
    }

    fn rejoin(r: &mut Recorder, first: Fact, second: Fact) {
        apply_all(
            r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                Fact::BlockStarted {
                    at_ms: 0,
                    activity: Activity::Match,
                    playlist_id: Some(11),
                    pack_id: None,
                },
                first,
                Fact::BlockEnded {
                    at_ms: 1,
                    data_gap_seconds: 0,
                },
                Fact::BlockStarted {
                    at_ms: 2,
                    activity: Activity::Match,
                    playlist_id: Some(11),
                    pack_id: None,
                },
                second,
            ],
        );
    }

    #[test]
    fn rejoined_match_merges_into_the_more_complete_record() {
        // Crash or disconnect, then rejoin: the same MatchGuid is finished twice.
        let mut r = recorder();
        rejoin(
            &mut r,
            fragment(MatchResult::Unknown, 100, 10),
            fragment(MatchResult::Win, 300, 30),
        );
        let row: (String, i64, i64, bool, String, i64) = r
            .conn()
            .query_row(
                "SELECT result, score, touches, metrics_partial, scoreboard_json, block_id
                 FROM match_record",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            row,
            ("win".to_string(), 300, 30, true, "[300]".to_string(), 2)
        );
    }

    #[test]
    fn rejoined_match_never_loses_a_known_result_or_stats() {
        let mut r = recorder();
        rejoin(
            &mut r,
            fragment(MatchResult::Loss, 300, 30),
            fragment(MatchResult::Unknown, 0, 0),
        );
        let row: (String, i64, i64) = r
            .conn()
            .query_row(
                "SELECT result, score, block_id FROM match_record",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, ("loss".to_string(), 300, 1));
    }

    #[test]
    fn failed_batch_rolls_back_and_keeps_the_recorder_consistent() {
        let mut r = recorder();
        let batch = [
            Fact::SessionStarted { at_ms: 0 },
            attempt(1, AttemptOutcome::Make), // no open block: fails mid-batch
        ];
        assert!(matches!(r.apply_all(&batch), Err(StoreError::NoOpenBlock)));
        let sessions: i64 = r
            .conn()
            .query_row("SELECT COUNT(*) FROM session", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sessions, 0, "the whole batch was rolled back");
        let block = Fact::BlockStarted {
            at_ms: 0,
            activity: Activity::Training,
            playlist_id: None,
            pack_id: None,
        };
        assert!(
            matches!(r.apply_all(&[block]), Err(StoreError::NoOpenSession)),
            "the rolled-back session id was not kept"
        );
    }

    #[test]
    fn facts_without_context_are_rejected() {
        let mut r = recorder();
        let block = Fact::BlockStarted {
            at_ms: 0,
            activity: Activity::Freeplay,
            playlist_id: None,
            pack_id: None,
        };
        assert!(matches!(r.apply(&block), Err(StoreError::NoOpenSession)));
        r.apply(&Fact::SessionStarted { at_ms: 0 }).unwrap();
        assert!(matches!(
            r.apply(&attempt(1, AttemptOutcome::Make)),
            Err(StoreError::NoOpenBlock)
        ));
    }

    #[test]
    fn profile_is_updated_not_duplicated() {
        let mut r = recorder();
        let detected = |name: &str| Fact::ProfileDetected {
            at_ms: 0,
            primary_id: "Epic|me|0".into(),
            display_name: name.into(),
        };
        apply_all(&mut r, &[detected("Me"), detected("me")]);
        let (n, name): (i64, String) = r
            .conn()
            .query_row(
                "SELECT COUNT(*), MAX(display_name) FROM profile",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((n, name.as_str()), (1, "me"));
    }

    #[test]
    fn first_detected_profile_becomes_active_and_later_ones_do_not() {
        let mut r = recorder();
        let detected = |id: &str| Fact::ProfileDetected {
            at_ms: 0,
            primary_id: id.into(),
            display_name: "X".into(),
        };
        apply_all(&mut r, &[detected("Epic|a|0"), detected("Epic|b|0")]);
        assert_eq!(
            profile_primary_id(r.conn()).unwrap().as_deref(),
            Some("Epic|a|0")
        );
    }

    fn block(at_ms: i64, activity: Activity) -> Fact {
        Fact::BlockStarted {
            at_ms,
            activity,
            playlist_id: None,
            pack_id: None,
        }
    }

    fn ended(at_ms: i64) -> Fact {
        Fact::BlockEnded {
            at_ms,
            data_gap_seconds: 0,
        }
    }

    #[test]
    fn training_block_without_attempts_is_dropped() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                block(0, Activity::Training),
                ended(19_000),
                block(19_000, Activity::Freeplay),
                Fact::FreeplayFinished(FreeplayStats::default()),
                ended(30_000),
                Fact::SessionEnded { at_ms: 30_000 },
            ],
        );
        let kinds: Vec<_> = list_sessions(r.conn()).unwrap()[0]
            .blocks
            .iter()
            .map(|b| b.kind.clone())
            .collect();
        assert_eq!(kinds, ["freeplay"]);
    }

    #[test]
    fn session_without_blocks_is_dropped() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                block(0, Activity::Training),
                ended(5_000),
                Fact::SessionEnded { at_ms: 5_000 },
            ],
        );
        assert!(list_sessions(r.conn()).unwrap().is_empty());
        assert_eq!(r.cursor(), Cursor::default());
    }

    #[test]
    fn cursor_tracks_the_newest_attempt() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                block(0, Activity::Training),
                attempt(1, AttemptOutcome::Make),
                attempt(2, AttemptOutcome::Miss),
            ],
        );
        let newest: i64 = r
            .conn()
            .query_row("SELECT id FROM training_attempt WHERE seq = 2", [], |row| {
                row.get(0)
            })
            .unwrap();
        let c = r.cursor();
        assert_eq!(
            (c.last_attempt_id, c.block_attempts, c.session_blocks),
            (Some(newest), 2, 1)
        );
        assert_eq!(c.block_kind, Some(Activity::Training));
    }

    #[test]
    fn free_play_where_the_car_never_moved_leaves_no_trace() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 1_000 },
                Fact::BlockStarted {
                    at_ms: 1_000,
                    activity: Activity::Freeplay,
                    playlist_id: Some(9),
                    pack_id: None,
                },
                Fact::FreeplayFinished(FreeplayStats::default()),
                Fact::BlockEnded {
                    at_ms: 1_000,
                    data_gap_seconds: 0,
                },
                Fact::SessionEnded { at_ms: 1_000 },
            ],
        );
        assert!(list_sessions(r.conn()).unwrap().is_empty());
    }

    #[test]
    fn a_match_activity_without_a_record_leaves_no_trace() {
        let mut r = recorder();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 1_000 },
                Fact::BlockStarted {
                    at_ms: 1_000,
                    activity: Activity::Match,
                    playlist_id: Some(6),
                    pack_id: None,
                },
                Fact::BlockEnded {
                    at_ms: 4_000,
                    data_gap_seconds: 0,
                },
                Fact::SessionEnded { at_ms: 4_000 },
            ],
        );
        assert!(list_sessions(r.conn()).unwrap().is_empty());
    }

    fn metrics(play_seconds: f64) -> MatchMetrics {
        MatchMetrics {
            play_seconds,
            big_pads: 7,
            goal_speed_avg: Some(88.0),
            ..Default::default()
        }
    }

    #[test]
    fn match_metrics_are_stored_and_absent_ones_stay_null() {
        let mut r = recorder();
        let finished = |guid: &str, metrics: Option<MatchMetrics>| {
            Fact::MatchFinished(MatchRecord {
                match_guid: guid.into(),
                playlist_id: Some(11),
                team_size: Some(2),
                arena: "Stadium_P".into(),
                scoreboard_json: "[]".into(),
                metrics,
                ..Default::default()
            })
        };
        let block = |at_ms: i64| Fact::BlockStarted {
            at_ms,
            activity: Activity::Match,
            playlist_id: Some(11),
            pack_id: None,
        };
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                block(0),
                finished("G1", Some(metrics(240.0))),
                Fact::BlockEnded {
                    at_ms: 1,
                    data_gap_seconds: 0,
                },
                block(2),
                finished("G2", None),
            ],
        );
        let mut stmt = r
            .conn()
            .prepare("SELECT play_seconds, big_pads, goal_speed_avg FROM match_record ORDER BY id")
            .unwrap();
        let rows: Vec<(Option<f64>, Option<i64>, Option<f64>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            rows,
            [(Some(240.0), Some(7), Some(88.0)), (None, None, None)]
        );
    }

    #[test]
    fn a_rejoined_match_keeps_the_metrics_of_the_fragment_with_more_touches() {
        let mut r = recorder();
        let with_metrics = |touches: i32, play: f64| match fragment(MatchResult::Win, 100, touches)
        {
            Fact::MatchFinished(m) => Fact::MatchFinished(MatchRecord {
                metrics: Some(metrics(play)),
                ..m
            }),
            _ => unreachable!(),
        };
        rejoin(&mut r, with_metrics(30, 300.0), with_metrics(10, 60.0));
        let play: f64 = r
            .conn()
            .query_row("SELECT play_seconds FROM match_record", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(play, 300.0);
    }

    #[test]
    fn shots_in_a_goalie_pack_are_saves_unless_the_ball_goes_in() {
        let mut r = recorder();
        r.conn()
            .execute("UPDATE pack SET goalie = 1 WHERE id = 2", [])
            .unwrap();
        apply_all(
            &mut r,
            &[
                Fact::SessionStarted { at_ms: 0 },
                Fact::BlockStarted {
                    at_ms: 0,
                    activity: Activity::Training,
                    playlist_id: Some(21),
                    pack_id: Some(2),
                },
                attempt(1, AttemptOutcome::Miss),
                attempt(2, AttemptOutcome::Make),
            ],
        );
        let outcomes = |r: &Recorder| -> Vec<String> {
            let mut stmt = r
                .conn()
                .prepare("SELECT outcome FROM training_attempt ORDER BY seq")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(outcomes(&r), ["make", "miss"], "a save, then a goal let in");
        r.set_open_block_pack(None).unwrap();
        apply_all(&mut r, &[attempt(3, AttemptOutcome::Miss)]);
        assert_eq!(
            outcomes(&r),
            ["miss", "make", "miss"],
            "no pack: striker rules again"
        );
    }

    #[test]
    fn a_rejoin_without_metrics_keeps_the_earlier_fragments_metrics() {
        // Spec §7.2 item 8: the winning fragment (more touches) has no metrics, so the earlier
        // fragment's are kept and the match is marked partial.
        let mut r = recorder();
        let first = match fragment(MatchResult::Unknown, 100, 10) {
            Fact::MatchFinished(m) => Fact::MatchFinished(MatchRecord {
                metrics: Some(MatchMetrics {
                    play_seconds: 120.0,
                    ..MatchMetrics::default()
                }),
                ..m
            }),
            other => other,
        };
        rejoin(&mut r, first, fragment(MatchResult::Win, 300, 30));
        let (secs, partial): (Option<f64>, bool) = r
            .conn()
            .query_row(
                "SELECT play_seconds, metrics_partial FROM match_record",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(secs, Some(120.0));
        assert!(partial);
    }
}
