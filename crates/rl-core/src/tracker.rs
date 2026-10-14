//! Sessions and blocks: turns the event stream into facts using the interpreters.

use crate::events::{GameEvent, PlayerRef, TimedEvent, UpdateState};
use crate::facts::Fact;
use crate::interp::freeplay::{FreeplayTracker, MOVING_KMH};
use crate::interp::{matches::MatchTracker, training::TrainingTracker};
use crate::playlist::{classify, Activity};

/// Free play without movement or a ball touch for this long ends at the last movement.
pub const FREEPLAY_IDLE_MS: i64 = 180_000;

/// How long a match camera target must stay identical before it is trusted as "me"
/// (while demolished, the camera can follow a teammate).
pub const MATCH_IDENTITY_CONFIRM_MS: i64 = 3_000;
/// A match stretch shorter than this, right after another match, with no touch of mine, is the
/// gap between two games of a lobby (see `close_block`).
pub const MATCH_FRAGMENT_MS: i64 = 15_000;

/// Does a typed hint (in-game name or account id) name this player? Case-insensitive; the id is
/// the middle part of `Platform|id|splitscreen`.
pub fn hint_matches(hint: &str, name: &str, primary_id: &str) -> bool {
    let hint = hint.trim();
    if hint.is_empty() {
        return false;
    }
    let id = primary_id.split('|').nth(1).unwrap_or("");
    hint.eq_ignore_ascii_case(name) || hint.eq_ignore_ascii_case(id)
}

#[derive(Debug, Clone)]
pub struct TrackerConfig {
    pub me_primary_id: Option<String>,
    /// In-game name or account id the user typed; detection only accepts a matching player.
    pub me_hint: Option<String>,
    pub session_timeout_ms: i64,
    pub hard_hit_kmh: f64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            me_primary_id: None,
            me_hint: None,
            session_timeout_ms: 30 * 60 * 1000,
            hard_hit_kmh: 80.0,
        }
    }
}

#[derive(Debug, Clone)]
struct MeRef {
    name: String,
    team_num: i32,
}

impl MeRef {
    fn is(&self, other: &PlayerRef) -> bool {
        other.team_num == self.team_num && other.name.eq_ignore_ascii_case(&self.name)
    }
}

enum BlockState {
    Freeplay(FreeplayTracker),
    Training(TrainingTracker),
    Match(Box<MatchTracker>),
    Other,
}

struct Block {
    activity: Activity,
    playlist_id: Option<i32>,
    match_guid: String,
    started_at_ms: i64,
    gap_ms: i64,
    /// Opened when another match's id gave way to this one (a lobby moving to its next game).
    after_match: bool,
    state: BlockState,
}

pub struct Tracker {
    cfg: TrackerConfig,
    session_open: bool,
    last_activity_ms: i64,
    block: Option<Block>,
    in_saved_replay: bool,
    in_goal_replay: bool,
    announced_guid: Option<String>,
    me: Option<MeRef>,
    identity_candidate: Option<(String, i64)>,
    disconnected_at_ms: Option<i64>,
    paused: bool,
    /// Last movement or touch in the open free-play block.
    freeplay_active_ms: i64,
    /// Free play was closed for idleness; a still car does not reopen it.
    parked: bool,
}

impl Tracker {
    pub fn new(cfg: TrackerConfig) -> Self {
        Self {
            cfg,
            session_open: false,
            last_activity_ms: 0,
            block: None,
            in_saved_replay: false,
            in_goal_replay: false,
            announced_guid: None,
            me: None,
            identity_candidate: None,
            disconnected_at_ms: None,
            paused: false,
            freeplay_active_ms: 0,
            parked: false,
        }
    }

    /// Applies new limits from now on: the session timeout at once, the hard-hit speed to free
    /// play that starts after this.
    pub fn set_limits(&mut self, session_timeout_ms: i64, hard_hit_kmh: f64) {
        self.cfg.session_timeout_ms = session_timeout_ms;
        self.cfg.hard_hit_kmh = hard_hit_kmh;
    }

    pub fn me_primary_id(&self) -> Option<&str> {
        self.cfg.me_primary_id.as_deref()
    }

    pub fn handle(&mut self, ev: &TimedEvent) -> Vec<Fact> {
        let at = ev.at_ms;
        let mut facts = self.tick(at);
        let idle = self.is_idle(&ev.event);
        // A still car neither opens a session nor reopens free play closed for idleness. Other
        // events (pause, lifecycle) are still applied; they just do not count as activity.
        let still_update = idle && matches!(ev.event, GameEvent::UpdateState(_));
        if still_update && (self.parked || !self.session_open) {
            return facts;
        }
        // Only game state opens a session; lifecycle events alone (menus, joins) record nothing.
        if !self.session_open && matches!(ev.event, GameEvent::UpdateState(_)) {
            self.session_open = true;
            facts.push(Fact::SessionStarted { at_ms: at });
        }
        if self.session_open && !idle {
            self.last_activity_ms = at;
        }
        match &ev.event {
            GameEvent::UpdateState(u) => self.on_update(at, u, &mut facts),
            GameEvent::MatchCreated { match_guid } | GameEvent::MatchInitialized { match_guid } => {
                if !match_guid.is_empty() {
                    self.announced_guid = Some(match_guid.clone());
                    if let Some(Block {
                        state: BlockState::Match(m),
                        match_guid: tracked,
                        ..
                    }) = self.block.as_mut()
                    {
                        if tracked == match_guid {
                            m.mark_started();
                        }
                    }
                }
            }
            GameEvent::MatchDestroyed => {
                self.close_block(at, &mut facts);
                self.in_saved_replay = false;
                self.in_goal_replay = false;
                self.announced_guid = None;
                self.paused = false;
            }
            GameEvent::MatchPaused => self.set_paused(true),
            GameEvent::MatchUnpaused => self.set_paused(false),
            GameEvent::ReplayCreated => {
                self.close_block(at, &mut facts);
                self.in_saved_replay = true;
            }
            GameEvent::GoalReplayStart => self.in_goal_replay = true,
            GameEvent::GoalReplayEnd => self.in_goal_replay = false,
            GameEvent::RoundStarted => {
                self.in_goal_replay = false;
                match self.state_mut() {
                    Some(BlockState::Training(t)) => {
                        facts.extend(t.round_started(at).map(Fact::AttemptFinished));
                    }
                    Some(BlockState::Match(m)) => m.kickoff(),
                    _ => {}
                }
            }
            GameEvent::CountdownBegin => match self.state_mut() {
                Some(BlockState::Training(t)) => {
                    facts.extend(t.countdown_began(at).map(Fact::AttemptFinished));
                }
                Some(BlockState::Match(m)) => m.kickoff(),
                _ => {}
            },
            GameEvent::GoalScored(goal) => {
                // A timed-out training shot ends with a GoalScored that has no scorer.
                let has_scorer = goal.scorer.as_ref().is_some_and(|s| !s.name.is_empty());
                let mine = matches!((&self.me, &goal.scorer), (Some(me), Some(s)) if me.is(s));
                if !self.in_goal_replay {
                    match self.state_mut() {
                        Some(BlockState::Training(t)) if has_scorer => t.goal(at, goal.goal_speed),
                        Some(BlockState::Freeplay(f)) => f.goal(),
                        Some(BlockState::Match(m)) if mine => m.goal(goal.goal_speed),
                        _ => {}
                    }
                }
            }
            GameEvent::BallHit(hit) => {
                let mine = self
                    .me
                    .as_ref()
                    .is_some_and(|me| hit.players.iter().any(|p| me.is(p)));
                match (mine, self.state_mut()) {
                    (true, Some(BlockState::Freeplay(f))) => f.ball_hit(hit.ball.post_hit_speed),
                    (true, Some(BlockState::Match(m))) => m.touch(at, &hit.ball),
                    _ => {}
                }
            }
            GameEvent::CrossbarHit => {
                if let Some(BlockState::Freeplay(f)) = self.state_mut() {
                    f.crossbar();
                }
            }
            GameEvent::Statfeed(feed) => {
                let is_me = |target: &Option<PlayerRef>| matches!((&self.me, target), (Some(me), Some(p)) if me.is(p));
                let epic_save = feed.event_name == "EpicSave" && is_me(&feed.main_target);
                let demolished = feed.event_name == "Demolish" && is_me(&feed.secondary_target);
                if let Some(BlockState::Match(m)) = self.state_mut() {
                    if epic_save {
                        m.epic_save();
                    }
                    if demolished {
                        m.demolished_me();
                    }
                }
            }
            GameEvent::MatchEnded { winner_team_num } => {
                if let Some(BlockState::Match(m)) = self.state_mut() {
                    m.ended(*winner_team_num);
                }
            }
            GameEvent::Other(_) => {}
        }
        if !idle && self.in_freeplay() {
            self.freeplay_active_ms = at;
        }
        facts
    }

    /// Closes idle free play, then the session if it has been idle longer than the timeout.
    pub fn tick(&mut self, now_ms: i64) -> Vec<Fact> {
        let mut facts = Vec::new();
        if self.in_freeplay() && now_ms - self.freeplay_active_ms >= FREEPLAY_IDLE_MS {
            let end = self.freeplay_active_ms;
            self.close_block(end, &mut facts);
            self.parked = true;
        }
        if self.session_open && now_ms - self.last_activity_ms > self.cfg.session_timeout_ms {
            let end = self.last_activity_ms;
            self.close_session(end, &mut facts);
        }
        facts
    }

    pub fn game_exited(&mut self, at_ms: i64) -> Vec<Fact> {
        let mut facts = Vec::new();
        if self.session_open {
            self.close_session(at_ms, &mut facts);
        }
        facts
    }

    pub fn connection_lost(&mut self, at_ms: i64) {
        if self.disconnected_at_ms.is_none() {
            self.disconnected_at_ms = Some(at_ms);
        }
    }

    /// Applies the session timeout first: a gap that outlived the session must not be credited
    /// to a block of the next one.
    pub fn connection_restored(&mut self, at_ms: i64) -> Vec<Fact> {
        let facts = self.tick(at_ms);
        if let Some(lost) = self.disconnected_at_ms.take() {
            if let Some(block) = self.block.as_mut() {
                block.gap_ms += (at_ms - lost).max(0);
            }
        }
        facts
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        match self.state_mut() {
            Some(BlockState::Freeplay(f)) => f.set_paused(paused),
            Some(BlockState::Match(m)) => m.set_paused(paused),
            Some(BlockState::Training(_) | BlockState::Other) | None => {}
        }
    }

    /// Ends the open training block and starts a new one with `pack_id` at the same instant.
    /// The interpreter state (including an attempt in progress and the attempt counter) carries
    /// over, so the attempt being played belongs to the new pack.
    pub fn split_block(&mut self, at_ms: i64, pack_id: Option<i64>) -> Vec<Fact> {
        let Some(block) = self.block.as_mut() else {
            return Vec::new();
        };
        if block.activity != Activity::Training {
            return Vec::new();
        }
        let gap_ms = std::mem::take(&mut block.gap_ms);
        block.started_at_ms = at_ms;
        vec![
            Fact::BlockEnded {
                at_ms,
                data_gap_seconds: gap_ms / 1000,
            },
            Fact::BlockStarted {
                at_ms,
                activity: Activity::Training,
                playlist_id: block.playlist_id,
                pack_id,
            },
        ]
    }

    pub fn live(&self) -> Option<crate::live::TrackerLive> {
        let block = self.block.as_ref()?;
        let (freeplay, match_live) = match &block.state {
            BlockState::Freeplay(f) => (Some(f.live()), None),
            BlockState::Match(m) => (None, Some(m.live())),
            BlockState::Training(_) | BlockState::Other => (None, None),
        };
        Some(crate::live::TrackerLive {
            activity: block.activity,
            playlist_id: block.playlist_id,
            started_at_ms: block.started_at_ms,
            paused: self.paused,
            freeplay,
            match_live,
        })
    }

    fn state_mut(&mut self) -> Option<&mut BlockState> {
        self.block.as_mut().map(|b| &mut b.state)
    }

    fn in_freeplay(&self) -> bool {
        self.block
            .as_ref()
            .is_some_and(|b| b.activity == Activity::Freeplay)
    }

    /// In free play only a moving car or a ball touch is activity: the game keeps streaming state
    /// while the player is away from the keyboard, and while paused it repeats the frozen speed.
    fn is_idle(&self, event: &GameEvent) -> bool {
        match event {
            GameEvent::UpdateState(u) => {
                classify(u, self.in_saved_replay) == Activity::Freeplay
                    && (self.paused || self.car_still(u))
            }
            GameEvent::BallHit(_) => false,
            _ => self.parked || self.in_freeplay(),
        }
    }

    fn car_still(&self, u: &UpdateState) -> bool {
        // Without a known player we cannot tell, so treat the update as activity.
        let Some(id) = self.cfg.me_primary_id.as_deref() else {
            return false;
        };
        u.players
            .iter()
            .find(|p| p.primary_id == id)
            .is_some_and(|me| me.speed < MOVING_KMH)
    }

    fn on_update(&mut self, at: i64, u: &UpdateState, facts: &mut Vec<Fact>) {
        let activity = classify(u, self.in_saved_replay);
        let needs_new_block = match &self.block {
            None => true,
            Some(b) => {
                b.activity != activity
                    || (activity == Activity::Match && b.match_guid != u.match_guid)
                    || (activity == Activity::Other && b.playlist_id != u.game.playlist_id)
            }
        };
        if needs_new_block {
            let after_match = self
                .block
                .as_ref()
                .is_some_and(|b| b.activity == Activity::Match);
            self.close_block(at, facts);
            self.open_block(at, activity, u, after_match, facts);
        }
        self.detect_identity(at, activity, u, facts);

        let me_id = self.cfg.me_primary_id.clone();
        let me_state = me_id
            .as_deref()
            .and_then(|id| u.players.iter().find(|p| p.primary_id == id));
        if let Some(me) = me_state {
            self.me = Some(MeRef {
                name: me.name.clone(),
                team_num: me.team_num,
            });
        }
        match self.state_mut() {
            Some(BlockState::Training(t)) => t.update(&u.game),
            Some(BlockState::Freeplay(f)) => {
                if let Some(me) = me_state {
                    f.sample(at, me, u.game.replay);
                }
            }
            Some(BlockState::Match(m)) => m.update(at, u, me_id.as_deref()),
            Some(BlockState::Other) | None => {}
        }
    }

    fn detect_identity(
        &mut self,
        at: i64,
        activity: Activity,
        u: &UpdateState,
        facts: &mut Vec<Fact>,
    ) {
        if self.cfg.me_primary_id.is_some()
            || u.game.replay
            || self.in_saved_replay
            || !u.game.has_target
        {
            return;
        }
        let Some(target) = &u.game.target else {
            return;
        };
        let Some(player) = u
            .players
            .iter()
            .find(|p| p.team_num == target.team_num && p.name.eq_ignore_ascii_case(&target.name))
        else {
            return;
        };
        if player.primary_id.is_empty() || player.primary_id.starts_with("Unknown") {
            return;
        }
        if let Some(hint) = &self.cfg.me_hint {
            if !hint_matches(hint, &player.name, &player.primary_id) {
                return;
            }
        }
        let first_seen = match &self.identity_candidate {
            Some((id, since)) if *id == player.primary_id => *since,
            _ => {
                self.identity_candidate = Some((player.primary_id.clone(), at));
                at
            }
        };
        let required = if activity == Activity::Match {
            MATCH_IDENTITY_CONFIRM_MS
        } else {
            0
        };
        if at - first_seen >= required {
            self.identity_candidate = None;
            self.cfg.me_primary_id = Some(player.primary_id.clone());
            facts.push(Fact::ProfileDetected {
                at_ms: at,
                primary_id: player.primary_id.clone(),
                display_name: player.name.clone(),
            });
        }
    }

    fn open_block(
        &mut self,
        at: i64,
        activity: Activity,
        u: &UpdateState,
        after_match: bool,
        facts: &mut Vec<Fact>,
    ) {
        let state = match activity {
            Activity::Freeplay => BlockState::Freeplay(FreeplayTracker::new(self.cfg.hard_hit_kmh)),
            Activity::Training => BlockState::Training(TrainingTracker::default()),
            Activity::Match => {
                let saw_start = self.announced_guid.as_deref() == Some(u.match_guid.as_str());
                BlockState::Match(Box::new(
                    MatchTracker::new(u.match_guid.clone(), saw_start)
                        .with_hard_hit_kmh(self.cfg.hard_hit_kmh),
                ))
            }
            Activity::Other => BlockState::Other,
        };
        facts.push(Fact::BlockStarted {
            at_ms: at,
            activity,
            playlist_id: u.game.playlist_id,
            pack_id: None,
        });
        self.block = Some(Block {
            activity,
            playlist_id: u.game.playlist_id,
            match_guid: u.match_guid.clone(),
            started_at_ms: at,
            gap_ms: 0,
            after_match,
            state,
        });
        self.parked = false;
        if activity == Activity::Freeplay {
            self.freeplay_active_ms = at;
        }
        if self.paused {
            self.set_paused(true);
        }
    }

    fn close_block(&mut self, at: i64, facts: &mut Vec<Fact>) {
        let Some(block) = self.block.take() else {
            return;
        };
        match block.state {
            BlockState::Training(mut t) => facts.extend(t.finish(at).map(Fact::AttemptFinished)),
            BlockState::Freeplay(f) => facts.push(Fact::FreeplayFinished(f.finish())),
            BlockState::Match(m) => {
                // Between two games of a private lobby the next game's id shows up for a few
                // seconds over the last scoreboard; that is not a match of its own.
                let between_games = block.after_match
                    && at - block.started_at_ms < MATCH_FRAGMENT_MS
                    && !m.touched_by_me();
                if !between_games {
                    facts.push(Fact::MatchFinished(m.finish()));
                }
            }
            BlockState::Other => {}
        }
        facts.push(Fact::BlockEnded {
            at_ms: at,
            data_gap_seconds: block.gap_ms / 1000,
        });
    }

    fn close_session(&mut self, at: i64, facts: &mut Vec<Fact>) {
        self.close_block(at, facts);
        self.session_open = false;
        self.in_saved_replay = false;
        self.in_goal_replay = false;
        self.parked = false;
        facts.push(Fact::SessionEnded { at_ms: at });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{
        BallHit, GameState, GoalScored, HitBall, PlayerRef, PlayerState, Statfeed, TeamState,
        UpdateState,
    };
    use crate::facts::{Attempt, AttemptOutcome, FreeplayStats, MatchRecord, MatchResult};

    const ME: &str = "Epic|me|0";
    const MIN: i64 = 60_000;

    fn ev(at_ms: i64, event: GameEvent) -> TimedEvent {
        TimedEvent { at_ms, event }
    }

    fn player(name: &str, id: &str, team: i32) -> PlayerState {
        PlayerState {
            name: name.into(),
            primary_id: id.into(),
            team_num: team,
            ..Default::default()
        }
    }

    fn me() -> PlayerState {
        PlayerState {
            speed: 40.0,
            ..player("Me", ME, 0)
        }
    }

    fn update(
        playlist: i32,
        guid: &str,
        players: Vec<PlayerState>,
        target: Option<&PlayerState>,
    ) -> GameEvent {
        GameEvent::UpdateState(UpdateState {
            match_guid: guid.into(),
            game: GameState {
                playlist_id: Some(playlist),
                time_seconds: 10.0,
                has_target: target.is_some(),
                target: target.map(|p| PlayerRef {
                    name: p.name.clone(),
                    shortcut: 0,
                    team_num: p.team_num,
                }),
                ..Default::default()
            },
            players,
        })
    }

    fn training_update(time: f64, blue: i32) -> GameEvent {
        GameEvent::UpdateState(UpdateState {
            players: vec![me()],
            game: GameState {
                playlist_id: Some(21),
                time_seconds: time,
                teams: vec![TeamState {
                    team_num: 0,
                    score: blue,
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn hit(name: &str, team: i32, speed: f64) -> GameEvent {
        GameEvent::BallHit(BallHit {
            players: vec![PlayerRef {
                name: name.into(),
                shortcut: 0,
                team_num: team,
            }],
            ball: HitBall {
                pre_hit_speed: 0.0,
                post_hit_speed: speed,
                location: None,
            },
        })
    }

    fn goal() -> GameEvent {
        GameEvent::GoalScored(GoalScored {
            goal_speed: 70.0,
            scorer: Some(PlayerRef {
                name: "Me".into(),
                shortcut: 0,
                team_num: 0,
            }),
        })
    }

    fn timeout_goal() -> GameEvent {
        GameEvent::GoalScored(GoalScored {
            goal_speed: 11.0,
            scorer: Some(PlayerRef::default()),
        })
    }

    fn known_me() -> TrackerConfig {
        TrackerConfig {
            me_primary_id: Some(ME.into()),
            ..TrackerConfig::default()
        }
    }

    fn run(tracker: &mut Tracker, events: Vec<TimedEvent>) -> Vec<Fact> {
        events.iter().flat_map(|e| tracker.handle(e)).collect()
    }

    fn freeplay_stats(facts: &[Fact]) -> Vec<FreeplayStats> {
        facts
            .iter()
            .filter_map(|f| match f {
                Fact::FreeplayFinished(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    fn matches(facts: &[Fact]) -> Vec<MatchRecord> {
        facts
            .iter()
            .filter_map(|f| match f {
                Fact::MatchFinished(m) => Some(m.clone()),
                _ => None,
            })
            .collect()
    }

    fn attempts(facts: &[Fact]) -> Vec<Attempt> {
        facts
            .iter()
            .filter_map(|f| match f {
                Fact::AttemptFinished(a) => Some(a.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn freeplay_detects_profile_and_counts_my_hits_case_insensitively() {
        let mut t = Tracker::new(TrackerConfig::default());
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(9, "", vec![me()], Some(&me()))),
                ev(100, hit("me", 0, 90.0)),
                ev(200, hit("Other", 1, 90.0)),
            ],
        );
        facts.extend(t.game_exited(300));
        assert!(matches!(&facts[0], Fact::SessionStarted { at_ms: 0 }));
        assert!(matches!(
            &facts[1],
            Fact::BlockStarted {
                activity: Activity::Freeplay,
                playlist_id: Some(9),
                ..
            }
        ));
        assert!(matches!(&facts[2], Fact::ProfileDetected { primary_id, .. } if primary_id == ME));
        let stats = freeplay_stats(&facts);
        assert_eq!((stats[0].touches, stats[0].hard_hits), (1, 1));
        assert!(matches!(
            facts.last(),
            Some(Fact::SessionEnded { at_ms: 300 })
        ));
        assert_eq!(t.me_primary_id(), Some(ME));
    }

    #[test]
    fn goals_during_goal_replays_are_ignored() {
        let mut t = Tracker::new(known_me());
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(9, "", vec![me()], None)),
                ev(100, goal()),
                ev(200, GameEvent::GoalReplayStart),
                ev(300, goal()),
                ev(400, GameEvent::GoalReplayEnd),
            ],
        );
        facts.extend(t.game_exited(500));
        assert_eq!(freeplay_stats(&facts)[0].goals, 1);
    }

    #[test]
    fn match_destroyed_without_end_is_unknown() {
        let mut t = Tracker::new(known_me());
        let opp = player("Opp", "Epic|opp|0", 1);
        let facts = run(
            &mut t,
            vec![
                ev(
                    0,
                    GameEvent::MatchCreated {
                        match_guid: "G1".into(),
                    },
                ),
                ev(100, update(2, "G1", vec![me(), opp], Some(&me()))),
                ev(200, GameEvent::MatchDestroyed),
            ],
        );
        let m = matches(&facts);
        assert_eq!(m.len(), 1);
        assert_eq!(
            (m[0].result, m[0].metrics_partial),
            (MatchResult::Unknown, false)
        );
        assert!(matches!(
            facts.last(),
            Some(Fact::BlockEnded { at_ms: 200, .. })
        ));
    }

    #[test]
    fn match_initialized_after_first_update_is_not_partial() {
        // Ranked 2v2 in the evening recording: MatchCreated had an empty guid and
        // MatchInitialized arrived 8 s after the first UpdateState of the match.
        let mut t = Tracker::new(known_me());
        let facts = run(
            &mut t,
            vec![
                ev(
                    0,
                    GameEvent::MatchCreated {
                        match_guid: String::new(),
                    },
                ),
                ev(100, update(11, "G", vec![me()], None)),
                ev(
                    8_000,
                    GameEvent::MatchInitialized {
                        match_guid: "G".into(),
                    },
                ),
                ev(9_000, update(11, "G", vec![me()], None)),
                ev(10_000, GameEvent::MatchDestroyed),
            ],
        );
        assert!(!matches(&facts)[0].metrics_partial);
    }

    #[test]
    fn new_match_guid_starts_a_new_block() {
        let mut t = Tracker::new(known_me());
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(2, "G1", vec![me()], None)),
                ev(
                    100,
                    GameEvent::MatchEnded {
                        winner_team_num: Some(0),
                    },
                ),
                ev(200, update(2, "G2", vec![me()], None)),
            ],
        );
        // G2 is a real game (longer than a between-games fragment).
        facts.extend(t.game_exited(30_000));
        let m = matches(&facts);
        assert_eq!(
            m.iter().map(|r| r.match_guid.as_str()).collect::<Vec<_>>(),
            ["G1", "G2"]
        );
        assert_eq!(m[0].result, MatchResult::Win);
        assert!(m[1].metrics_partial, "G2 was never announced");
    }

    #[test]
    fn the_scoreboard_between_two_private_games_is_not_a_match() {
        // Private 1v1 on 2026-10-08: the next game's id arrived while the last scoreboard was
        // still shown (3 s), then the game restarted under that id.
        let mut t = Tracker::new(known_me());
        let facts = run(
            &mut t,
            vec![
                ev(0, update(6, "G1", vec![me()], None)),
                ev(1_000, hit("Me", 0, 50.0)),
                ev(
                    400_000,
                    GameEvent::MatchEnded {
                        winner_team_num: Some(1),
                    },
                ),
                ev(410_000, update(6, "G2", vec![me()], None)),
                ev(413_000, GameEvent::MatchDestroyed),
                ev(415_000, update(6, "G2", vec![me()], None)),
                ev(420_000, hit("Me", 0, 60.0)),
                ev(
                    800_000,
                    GameEvent::MatchEnded {
                        winner_team_num: Some(0),
                    },
                ),
                ev(810_000, GameEvent::MatchDestroyed),
            ],
        );
        let m = matches(&facts);
        assert_eq!(
            m.iter()
                .map(|r| (r.match_guid.as_str(), r.result))
                .collect::<Vec<_>>(),
            [("G1", MatchResult::Loss), ("G2", MatchResult::Win)]
        );
        let ended = facts
            .iter()
            .filter(|f| matches!(f, Fact::BlockEnded { .. }))
            .count();
        assert_eq!(ended, 3, "the 3-second block still opens and closes");
    }

    #[test]
    fn saved_replays_are_other_blocks() {
        let mut t = Tracker::new(known_me());
        let mut replay = update(13, "R", vec![me()], None);
        if let GameEvent::UpdateState(u) = &mut replay {
            u.game.replay = true;
        }
        let facts = run(
            &mut t,
            vec![
                ev(0, GameEvent::ReplayCreated),
                ev(100, replay),
                ev(200, GameEvent::MatchDestroyed),
            ],
        );
        assert!(facts.iter().any(|f| matches!(
            f,
            Fact::BlockStarted {
                activity: Activity::Other,
                ..
            }
        )));
        assert!(matches(&facts).is_empty());
    }

    #[test]
    fn event_after_timeout_starts_a_new_session_without_tick() {
        let mut t = Tracker::new(known_me());
        let facts = run(
            &mut t,
            vec![
                ev(0, update(9, "", vec![me()], None)),
                ev(31 * MIN, update(9, "", vec![me()], None)),
            ],
        );
        let sessions: Vec<_> = facts
            .iter()
            .filter(|f| matches!(f, Fact::SessionStarted { .. } | Fact::SessionEnded { .. }))
            .collect();
        assert_eq!(
            sessions,
            [
                &Fact::SessionStarted { at_ms: 0 },
                &Fact::SessionEnded { at_ms: 0 },
                &Fact::SessionStarted { at_ms: 31 * MIN }
            ]
        );
    }

    #[test]
    fn tick_closes_an_idle_session_once() {
        let mut t = Tracker::new(known_me());
        t.handle(&ev(0, update(9, "", vec![me()], None)));
        // Free play idle for 3 minutes already closed the block; the session stays open.
        assert!(!t
            .tick(29 * MIN)
            .iter()
            .any(|f| matches!(f, Fact::SessionEnded { .. })));
        assert!(matches!(
            t.tick(31 * MIN).last(),
            Some(Fact::SessionEnded { at_ms: 0 })
        ));
        assert!(t.tick(40 * MIN).is_empty());
        assert!(t.game_exited(41 * MIN).is_empty());
    }

    #[test]
    fn match_identity_needs_a_target_held_for_three_seconds() {
        let mut t = Tracker::new(TrackerConfig::default());
        let opp = player("Opp", "Epic|opp|0", 1);
        let at = |ms: i64| ev(ms, update(2, "G", vec![me(), opp.clone()], Some(&me())));
        let early = run(&mut t, (0..30).map(|i| at(i * 100)).collect());
        assert!(!early
            .iter()
            .any(|f| matches!(f, Fact::ProfileDetected { .. })));
        let late = run(&mut t, vec![at(MATCH_IDENTITY_CONFIRM_MS)]);
        assert!(late
            .iter()
            .any(|f| matches!(f, Fact::ProfileDetected { primary_id, .. } if primary_id == ME)));
    }

    #[test]
    fn bots_are_never_detected_as_me() {
        let mut t = Tracker::new(TrackerConfig::default());
        let bot = player("Bot", "Unknown|0|0", 0);
        let facts = run(
            &mut t,
            vec![ev(0, update(6, "", vec![bot.clone()], Some(&bot)))],
        );
        assert!(!facts
            .iter()
            .any(|f| matches!(f, Fact::ProfileDetected { .. })));
        assert_eq!(t.me_primary_id(), None);
    }

    #[test]
    fn connection_gaps_are_recorded_on_the_block() {
        let mut t = Tracker::new(known_me());
        t.handle(&ev(0, update(9, "", vec![me()], None)));
        t.connection_lost(1_000);
        assert!(t.connection_restored(6_000).is_empty());
        let facts = t.game_exited(7_000);
        assert!(facts.iter().any(|f| matches!(
            f,
            Fact::BlockEnded {
                data_gap_seconds: 5,
                ..
            }
        )));
    }

    #[test]
    fn training_attempts_flow_through() {
        let mut t = Tracker::new(known_me());
        let mut facts = run(
            &mut t,
            vec![
                ev(0, training_update(10.0, 0)),
                ev(100, GameEvent::RoundStarted),
                ev(1_100, training_update(9.0, 0)),
                ev(2_100, training_update(8.0, 0)),
                ev(2_500, goal()),
                ev(3_000, GameEvent::RoundStarted),
                ev(4_000, training_update(10.0, 1)),
                ev(5_000, training_update(9.0, 1)),
                ev(6_000, training_update(8.0, 1)),
            ],
        );
        facts.extend(t.game_exited(7_000));
        let outcomes: Vec<_> = attempts(&facts).iter().map(|a| a.outcome).collect();
        assert_eq!(outcomes, [AttemptOutcome::Make, AttemptOutcome::Miss]);
    }

    #[test]
    fn timed_out_shot_with_scorerless_goal_is_a_miss_closed_at_countdown() {
        let mut t = Tracker::new(known_me());
        let mut events = vec![
            ev(0, training_update(10.0, 0)),
            ev(100, GameEvent::RoundStarted),
        ];
        events.extend((0..=10).map(|i| ev(1_000 + i * 1_000, training_update((10 - i) as f64, 0))));
        events.push(ev(11_500, timeout_goal()));
        events.push(ev(12_000, GameEvent::CountdownBegin));
        events.push(ev(12_100, training_update(10.0, 0)));
        let facts = run(&mut t, events);
        let a = attempts(&facts);
        assert_eq!(
            a.len(),
            1,
            "closed by CountdownBegin, not by the next RoundStarted"
        );
        assert_eq!(
            (a[0].outcome, a[0].goal_speed),
            (AttemptOutcome::Miss, None)
        );
    }

    #[test]
    fn match_metrics_follow_my_touches_goals_and_demolitions() {
        let mut t = Tracker::new(known_me());
        let player = |name: &str, team: i32| PlayerRef {
            name: name.into(),
            shortcut: 1,
            team_num: team,
        };
        let demolish = |victim: &str| {
            GameEvent::Statfeed(Statfeed {
                event_name: "Demolish".into(),
                main_target: Some(player("Them", 1)),
                secondary_target: Some(player(victim, 0)),
            })
        };
        let goal_by = |name: &str, team: i32, speed: f64| {
            GameEvent::GoalScored(GoalScored {
                goal_speed: speed,
                scorer: Some(player(name, team)),
            })
        };
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(11, "G", vec![me()], None)),
                ev(100, update(11, "G", vec![me()], None)),
                ev(150, hit("Me", 0, 90.0)),
                ev(200, goal_by("Me", 0, 95.0)),
                ev(250, goal_by("Them", 1, 50.0)),
                ev(300, demolish("Me")),
                ev(350, demolish("Mate")),
                ev(400, update(11, "G", vec![me()], None)),
            ],
        );
        facts.extend(t.game_exited(500));
        let m = matches(&facts)[0]
            .metrics
            .clone()
            .expect("metrics recorded");
        assert_eq!(
            (m.hard_hits, m.goal_speed_avg, m.demos_taken),
            (1, Some(95.0), 1)
        );
        assert!((m.play_seconds - 0.4).abs() < 1e-9);
    }

    #[test]
    fn a_kickoff_resets_the_boost_sequence() {
        let mut t = Tracker::new(known_me());
        let with_boost = |boost: f64| {
            let mut p = me();
            p.boost = boost;
            update(11, "G", vec![p], None)
        };
        let mut facts = run(
            &mut t,
            vec![
                ev(0, with_boost(0.0)),
                ev(100, GameEvent::CountdownBegin),
                ev(200, with_boost(33.0)),
                ev(300, with_boost(100.0)),
            ],
        );
        facts.extend(t.game_exited(400));
        let m = matches(&facts)[0].metrics.clone().unwrap();
        assert_eq!(
            m.big_pads, 1,
            "0 → 33 at the kickoff is not a pad; 33 → 100 is"
        );
    }

    #[test]
    fn epic_saves_are_attributed_to_me() {
        let mut t = Tracker::new(known_me());
        let feed = |name: &str| {
            GameEvent::Statfeed(Statfeed {
                event_name: "EpicSave".into(),
                main_target: Some(PlayerRef {
                    name: name.into(),
                    shortcut: 0,
                    team_num: 0,
                }),
                secondary_target: None,
            })
        };
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(2, "G", vec![me()], None)),
                ev(100, feed("ME")),
                ev(200, feed("Someone")),
            ],
        );
        facts.extend(t.game_exited(300));
        assert_eq!(matches(&facts)[0].epic_saves, 1);
    }

    #[test]
    fn events_before_the_first_update_do_not_open_a_session() {
        let mut t = Tracker::new(known_me());
        let early = run(
            &mut t,
            vec![
                ev(
                    0,
                    GameEvent::MatchCreated {
                        match_guid: "G".into(),
                    },
                ),
                ev(50, GameEvent::Other("PlayerJoined".into())),
            ],
        );
        assert!(early.is_empty());
        let facts = run(&mut t, vec![ev(100, update(9, "", vec![me()], None))]);
        assert!(matches!(facts[0], Fact::SessionStarted { at_ms: 100 }));
    }

    #[test]
    fn pauses_reach_the_freeplay_aggregates() {
        let mut t = Tracker::new(known_me());
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(9, "", vec![me()], None)),
                ev(50, GameEvent::MatchPaused),
                ev(1_000, update(9, "", vec![me()], None)),
                ev(1_050, GameEvent::MatchUnpaused),
                ev(1_100, update(9, "", vec![me()], None)),
            ],
        );
        facts.extend(t.game_exited(1_200));
        assert_eq!(freeplay_stats(&facts)[0].pct_airborne, 0.0);
    }

    #[test]
    fn a_new_playlist_inside_other_starts_a_new_block() {
        let mut t = Tracker::new(known_me());
        let mut facts = run(
            &mut t,
            vec![
                ev(0, update(6, "", vec![me()], None)),
                ev(100, update(27, "", vec![me()], None)),
            ],
        );
        facts.extend(t.game_exited(200));
        let playlists: Vec<_> = facts
            .iter()
            .filter_map(|f| match f {
                Fact::BlockStarted { playlist_id, .. } => Some(*playlist_id),
                _ => None,
            })
            .collect();
        assert_eq!(playlists, [Some(6), Some(27)]);
    }

    #[test]
    fn a_gap_longer_than_the_timeout_closes_the_session_instead_of_growing_the_block() {
        let mut t = Tracker::new(known_me());
        t.handle(&ev(0, update(9, "", vec![me()], None)));
        t.connection_lost(1_000);
        let facts = t.connection_restored(40 * MIN);
        assert!(facts.iter().any(|f| matches!(
            f,
            Fact::BlockEnded {
                data_gap_seconds: 0,
                ..
            }
        )));
        assert!(matches!(
            facts.last(),
            Some(Fact::SessionEnded { at_ms: 0 })
        ));
    }

    #[test]
    fn identity_matches_the_target_name_case_insensitively() {
        let mut t = Tracker::new(TrackerConfig::default());
        let target = player("ME", ME, 0);
        let facts = run(
            &mut t,
            vec![ev(0, update(9, "", vec![me()], Some(&target)))],
        );
        assert!(facts
            .iter()
            .any(|f| matches!(f, Fact::ProfileDetected { primary_id, .. } if primary_id == ME)));
    }

    fn detected_with_hint(hint: &str, target: &PlayerState) -> bool {
        let mut t = Tracker::new(TrackerConfig {
            me_hint: Some(hint.into()),
            ..TrackerConfig::default()
        });
        let me = PlayerState {
            name: target.name.clone(),
            primary_id: target.primary_id.clone(),
            ..me()
        };
        run(&mut t, vec![ev(0, update(9, "", vec![me], Some(target)))])
            .iter()
            .any(|f| matches!(f, Fact::ProfileDetected { .. }))
    }

    #[test]
    fn a_name_hint_accepts_the_match_and_rejects_another_player() {
        let target = player("Some Name", "Epic|abc123|0", 0);
        assert!(detected_with_hint("some name", &target));
        assert!(!detected_with_hint("Other", &target));
    }

    #[test]
    fn an_account_id_hint_matches_epic_and_steam_ids() {
        let epic = player("A", "Epic|AbC123|0", 0);
        assert!(detected_with_hint("abc123", &epic));
        let steam = player("B", "Steam|76561198000000000|0", 0);
        assert!(detected_with_hint("76561198000000000", &steam));
        assert!(!detected_with_hint("76561198000000001", &steam));
    }

    #[test]
    fn split_block_only_applies_to_training() {
        let mut t = Tracker::new(known_me());
        t.handle(&ev(0, update(9, "", vec![me()], None)));
        assert!(t.split_block(100, Some(1)).is_empty());
    }

    #[test]
    fn live_reports_running_freeplay_and_match_numbers() {
        let mut t = Tracker::new(known_me());
        assert!(t.live().is_none());
        run(
            &mut t,
            vec![
                ev(0, update(9, "", vec![me()], None)),
                ev(100, hit("Me", 0, 95.0)),
            ],
        );
        let live = t.live().unwrap();
        assert_eq!(live.activity, Activity::Freeplay);
        assert_eq!(live.started_at_ms, 0);
        let fp = live.freeplay.unwrap();
        assert_eq!((fp.touches, fp.hard_hits), (1, 1));

        let mut scored = me();
        scored.goals = 2;
        run(&mut t, vec![ev(1_000, update(11, "G", vec![scored], None))]);
        let live = t.live().unwrap();
        let m = live.match_live.unwrap();
        assert_eq!((m.goals, m.ranked, m.team_size), (2, true, Some(2)));
        assert_eq!(m.clock_seconds, 10);
    }

    fn freeplay(speed: f64) -> GameEvent {
        update(9, "", vec![PlayerState { speed, ..me() }], None)
    }

    fn block_starts(facts: &[Fact]) -> Vec<i64> {
        facts
            .iter()
            .filter_map(|f| match f {
                Fact::BlockStarted { at_ms, .. } => Some(*at_ms),
                _ => None,
            })
            .collect()
    }

    fn block_ends(facts: &[Fact]) -> Vec<i64> {
        facts
            .iter()
            .filter_map(|f| match f {
                Fact::BlockEnded { at_ms, .. } => Some(*at_ms),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn three_still_minutes_close_free_play_at_the_last_movement() {
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0)), ev(10_000, freeplay(40.0))];
        events.extend((11..=200).map(|s| ev(s * 1_000, freeplay(0.0))));
        let facts = run(&mut t, events);
        assert_eq!(block_ends(&facts), [10_000]);
        assert_eq!(freeplay_stats(&facts).len(), 1);
        assert!(t.live().is_none(), "nothing is recorded while parked");
    }

    #[test]
    fn a_ball_touch_counts_as_activity() {
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0))];
        events.extend((1..=290).map(|s| ev(s * 1_000, freeplay(0.0))));
        events.insert(101, ev(100_500, hit("Me", 0, 50.0))); // right after the 100 s update
        let facts = run(&mut t, events);
        assert_eq!(block_ends(&facts), [100_500]);
    }

    #[test]
    fn moving_again_starts_a_new_free_play_stretch_in_the_same_session() {
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0))];
        events.extend((1..=200).map(|s| ev(s * 1_000, freeplay(0.0))));
        events.push(ev(300_000, freeplay(40.0)));
        let facts = run(&mut t, events);
        let sessions = facts
            .iter()
            .filter(|f| matches!(f, Fact::SessionStarted { .. }))
            .count();
        assert_eq!(sessions, 1);
        assert_eq!(block_starts(&facts), [0, 300_000]);
    }

    #[test]
    fn a_still_car_does_not_keep_the_session_open() {
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0))];
        events.extend((1..=200).map(|n| ev(n * 10_000, freeplay(0.0)))); // 33 minutes
        let facts = run(&mut t, events);
        assert!(facts
            .iter()
            .any(|f| matches!(f, Fact::SessionEnded { at_ms: 0 })));
    }

    #[test]
    fn a_still_car_never_opens_a_session() {
        let mut t = Tracker::new(known_me());
        let facts = run(
            &mut t,
            vec![ev(0, freeplay(0.0)), ev(60_000, freeplay(0.0))],
        );
        assert!(
            facts.is_empty(),
            "walked away before moving: nothing to record"
        );
    }

    #[test]
    fn the_engine_tick_closes_idle_free_play() {
        let mut t = Tracker::new(known_me());
        run(&mut t, vec![ev(0, freeplay(40.0))]);
        assert!(t.tick(179_999).is_empty());
        assert_eq!(block_ends(&t.tick(180_000)), [0]);
    }

    #[test]
    fn my_ball_hits_in_a_match_count_toward_the_hardest_shot() {
        let mut t = Tracker::new(known_me());
        run(
            &mut t,
            vec![
                ev(0, update(11, "G", vec![me()], None)),
                ev(500, hit("Me", 0, 97.0)),
                ev(600, hit("Other", 1, 120.0)),
            ],
        );
        assert_eq!(t.live().unwrap().match_live.unwrap().hardest_hit, 97.0);
    }

    #[test]
    fn a_paused_free_play_game_counts_as_idle() {
        // The Stats API keeps streaming the frozen car speed while the game is paused.
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0)), ev(1_000, GameEvent::MatchPaused)];
        events.extend((2..=200).map(|s| ev(s * 1_000, freeplay(17.7))));
        let facts = run(&mut t, events);
        assert_eq!(block_ends(&facts), [0]);
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0)), ev(1_000, GameEvent::MatchPaused)];
        events.extend((1..=210).map(|n| ev(n * 10_000, freeplay(17.7)))); // 35 minutes
        let facts = run(&mut t, events);
        assert!(facts
            .iter()
            .any(|f| matches!(f, Fact::SessionEnded { at_ms: 0 })));
    }

    #[test]
    fn unpausing_while_parked_is_not_lost() {
        let mut t = Tracker::new(known_me());
        let mut events = vec![ev(0, freeplay(40.0)), ev(1_000, GameEvent::MatchPaused)];
        events.extend((2..=200).map(|s| ev(s * 1_000, freeplay(17.7))));
        events.push(ev(210_000, GameEvent::MatchUnpaused));
        events.extend((211..=213).map(|s| ev(s * 1_000, freeplay(40.0))));
        let facts = run(&mut t, events);
        assert_eq!(block_starts(&facts), [0, 211_000]);
        let live = t.live().unwrap();
        assert!(!live.paused, "the unpause reached the tracker while parked");
        assert!(
            live.freeplay.unwrap().pct_airborne > 0.0,
            "time is credited again"
        );
    }

    #[test]
    fn new_limits_apply_to_the_running_session() {
        let mut t = Tracker::new(known_me());
        run(&mut t, vec![ev(0, freeplay(40.0))]);
        t.set_limits(MIN, 80.0);
        let facts = t.tick(MIN + 1);
        assert!(facts.iter().any(|f| matches!(f, Fact::SessionEnded { .. })));
    }
}
