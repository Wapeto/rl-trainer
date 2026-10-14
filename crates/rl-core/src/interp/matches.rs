//! Online match aggregation: own scoreboard, boost metrics, result.

use serde::Serialize;

use crate::events::{HitBall, PlayerState, UpdateState};
use crate::facts::{MatchRecord, MatchResult};
use crate::interp::match_metrics::{MatchMetricsAcc, DEFAULT_HARD_HIT_KMH};
use crate::interp::sampling::{pct, SampleClock};
use crate::live::{CompareStat, LiveMatch, StatCompare};
use crate::playlist::playlist_info;

#[derive(Serialize)]
struct ScoreboardRow<'a> {
    name: &'a str,
    team_num: i32,
    score: i32,
    goals: i32,
    assists: i32,
    saves: i32,
    shots: i32,
    demos: i32,
    /// My row (Plan 6): progress finds it by this flag, not by an equal stat line.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    me: bool,
}

#[derive(Debug, Default)]
pub struct MatchTracker {
    match_guid: String,
    saw_start: bool,
    playlist_id: Option<i32>,
    arena: String,
    overtime: bool,
    last_update: Option<UpdateState>,
    /// Last-seen row of every player, kept after they leave (players often leave at the podium).
    roster: Vec<PlayerState>,
    last_me: Option<PlayerState>,
    winner: Option<i32>,
    ended: bool,
    paused: bool,
    epic_saves: i32,
    clock: SampleClock,
    total_ms: i64,
    boost_ms_sum: f64,
    zero_boost_ms: i64,
    supersonic_ms: i64,
    max_hit_speed: f64,
    metrics: MatchMetricsAcc,
}

impl MatchTracker {
    pub fn new(match_guid: String, saw_start: bool) -> Self {
        Self {
            match_guid,
            saw_start,
            metrics: MatchMetricsAcc::new(DEFAULT_HARD_HIT_KMH),
            ..Self::default()
        }
    }

    /// Uses the hard-hit setting instead of the default.
    pub fn with_hard_hit_kmh(mut self, kmh: f64) -> Self {
        self.metrics = MatchMetricsAcc::new(kmh);
        self
    }

    pub fn update(&mut self, at_ms: i64, update: &UpdateState, me_primary_id: Option<&str>) {
        self.playlist_id = update.game.playlist_id.or(self.playlist_id);
        if !update.game.arena.is_empty() {
            self.arena = update.game.arena.clone();
        }
        self.overtime |= update.game.overtime;
        if update.game.has_winner && self.winner.is_none() {
            self.winner = update
                .game
                .teams
                .iter()
                .find(|t| !t.name.is_empty() && t.name == update.game.winner)
                .map(|t| t.team_num);
        }
        let me = me_primary_id.and_then(|id| update.players.iter().find(|p| p.primary_id == id));
        if let Some(me) = me {
            self.last_me = Some(me.clone());
            // Goal replays, pauses and the podium after MatchEnded are not play time.
            if update.game.replay || self.paused || self.ended {
                self.clock.pause();
                self.metrics.pause();
            } else {
                let dt = self.clock.tick(at_ms);
                self.total_ms += dt;
                let our_touch = update.game.last_touch_team() == Some(me.team_num);
                self.metrics.sample(dt, me, our_touch);
                self.boost_ms_sum += me.boost * dt as f64;
                if me.boost <= 0.0 {
                    self.zero_boost_ms += dt;
                }
                if me.supersonic {
                    self.supersonic_ms += dt;
                }
            }
        }
        for player in &update.players {
            let key = roster_key(player);
            match self.roster.iter_mut().find(|p| roster_key(p) == key) {
                Some(seen) => *seen = player.clone(),
                None => self.roster.push(player.clone()),
            }
        }
        if !update.players.is_empty() {
            self.last_update = Some(update.clone());
        }
    }

    pub fn epic_save(&mut self) {
        self.epic_saves += 1;
    }

    pub fn ended(&mut self, winner_team_num: Option<i32>) {
        self.ended = true;
        self.winner = winner_team_num.or(self.winner);
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// The match's start was announced after tracking began (seen in ranked 2v2).
    pub fn mark_started(&mut self) {
        self.saw_start = true;
    }

    /// Whether I touched the ball in this stretch of the match.
    pub fn touched_by_me(&self) -> bool {
        self.max_hit_speed > 0.0
    }

    pub fn ball_hit(&mut self, post_hit_speed: f64) {
        self.max_hit_speed = self.max_hit_speed.max(post_hit_speed);
    }

    /// One of my touches: the hardest hit and the per-match metrics.
    pub fn touch(&mut self, at_ms: i64, ball: &HitBall) {
        self.ball_hit(ball.post_hit_speed);
        if let Some(team) = self.last_me.as_ref().map(|m| m.team_num) {
            self.metrics.touch(at_ms, team, ball);
        }
    }

    /// A goal I scored (goal replays are filtered by the tracker).
    pub fn goal(&mut self, goal_speed: f64) {
        self.metrics.goal(goal_speed);
    }

    pub fn demolished_me(&mut self) {
        self.metrics.demolished();
    }

    /// A countdown or kickoff resets boost.
    pub fn kickoff(&mut self) {
        self.metrics.pause();
    }

    pub fn live(&self) -> LiveMatch {
        let me_team = self.last_me.as_ref().map(|m| m.team_num);
        let (team_score, opponent_score, clock_seconds) = match (me_team, self.last_update.as_ref())
        {
            (Some(team), Some(u)) => (
                u.game.team_score(team),
                u.game.team_score(1 - team),
                u.game.time_seconds.round() as i32,
            ),
            (None, Some(u)) => (0, 0, u.game.time_seconds.round() as i32),
            _ => (0, 0, 0),
        };
        let info = playlist_info(self.playlist_id);
        let me = self.last_me.clone().unwrap_or_default();
        LiveMatch {
            playlist_id: self.playlist_id,
            ranked: info.ranked,
            team_size: info
                .team_size
                .or_else(|| (!self.roster.is_empty()).then(|| largest_team(&self.roster))),
            arena: self.arena.clone(),
            team_score,
            opponent_score,
            clock_seconds,
            overtime: self.overtime,
            goals: me.goals,
            assists: me.assists,
            saves: me.saves,
            shots: me.shots,
            score: me.score,
            hardest_hit: self.max_hit_speed,
            pct_supersonic: pct(self.supersonic_ms, self.total_ms),
            compare: compare(
                &self.roster,
                self.last_me.as_ref().map(roster_key).as_deref(),
            ),
            avg_boost: if self.total_ms > 0 {
                self.boost_ms_sum / self.total_ms as f64
            } else {
                0.0
            },
            pct_zero_boost: pct(self.zero_boost_ms, self.total_ms),
            today_wins: 0,
            today_losses: 0,
        }
    }

    pub fn finish(self) -> MatchRecord {
        let me_team = self.last_me.as_ref().map(|m| m.team_num);
        let (team_score, opponent_score) = match (me_team, self.last_update.as_ref()) {
            (Some(team), Some(u)) => (u.game.team_score(team), u.game.team_score(1 - team)),
            _ => (0, 0),
        };
        // Without a reported winner (a forfeit), the team ahead at the end won.
        let winner = self.winner.or_else(|| {
            let team = me_team?;
            match team_score.cmp(&opponent_score) {
                std::cmp::Ordering::Greater => Some(team),
                std::cmp::Ordering::Less => Some(1 - team),
                std::cmp::Ordering::Equal => None,
            }
        });
        let result = match (winner, me_team) {
            (Some(winner), Some(team)) if winner == team => MatchResult::Win,
            (Some(_), Some(_)) => MatchResult::Loss,
            _ => MatchResult::Unknown,
        };
        let info = playlist_info(self.playlist_id);
        let team_size = info
            .team_size
            .or_else(|| (!self.roster.is_empty()).then(|| largest_team(&self.roster)));
        let scoreboard_json = scoreboard_json(
            &self.roster,
            self.last_me.as_ref().map(|m| m.primary_id.as_str()),
        );
        let metrics_partial = !self.saw_start || self.last_me.is_none();
        let avg_boost = if self.total_ms > 0 {
            self.boost_ms_sum / self.total_ms as f64
        } else {
            0.0
        };
        let metrics = self.metrics.finish();
        let me = self.last_me.unwrap_or_default();
        MatchRecord {
            match_guid: self.match_guid,
            playlist_id: self.playlist_id,
            ranked: info.ranked,
            team_size,
            arena: self.arena,
            result,
            team_score,
            opponent_score,
            overtime: self.overtime,
            score: me.score,
            goals: me.goals,
            assists: me.assists,
            saves: me.saves,
            shots: me.shots,
            touches: me.touches,
            demos: me.demos,
            epic_saves: self.epic_saves,
            avg_boost,
            pct_zero_boost: pct(self.zero_boost_ms, self.total_ms),
            pct_supersonic: pct(self.supersonic_ms, self.total_ms),
            metrics_partial,
            scoreboard_json,
            metrics,
        }
    }
}

fn largest_team(players: &[PlayerState]) -> u8 {
    let blue = players.iter().filter(|p| p.team_num == 0).count();
    let orange = players.len() - blue;
    u8::try_from(blue.max(orange)).unwrap_or(u8::MAX)
}

/// Identifies a player across updates; offline bots all share `Unknown|0|0`.
fn roster_key(p: &PlayerState) -> String {
    if p.primary_id.is_empty() || p.primary_id.starts_with("Unknown") {
        format!("{}#{}", p.name, p.team_num)
    } else {
        p.primary_id.clone()
    }
}

type StatOf = fn(&PlayerState) -> i32;

const COMPARED: [(CompareStat, StatOf); 5] = [
    (CompareStat::Score, |p| p.score),
    (CompareStat::Goals, |p| p.goals),
    (CompareStat::Assists, |p| p.assists),
    (CompareStat::Saves, |p| p.saves),
    (CompareStat::Shots, |p| p.shots),
];

fn compare(roster: &[PlayerState], me_key: Option<&str>) -> Vec<StatCompare> {
    let is_me = |p: &PlayerState| me_key.is_some_and(|key| roster_key(p) == key);
    COMPARED
        .iter()
        .map(|&(stat, value_of)| {
            let mine = roster.iter().find(|p| is_me(p)).map_or(0, value_of);
            let best = roster.iter().map(value_of).max().unwrap_or(0);
            let holders: Vec<&PlayerState> =
                roster.iter().filter(|p| value_of(p) == best).collect();
            let tie = holders.len() > 1;
            let me_holds = holders.iter().any(|p| is_me(p));
            StatCompare {
                stat,
                mine,
                best,
                best_name: (!tie && !me_holds)
                    .then(|| holders.first().map(|p| p.name.clone()))
                    .flatten(),
                tie,
                mine_is_best: me_holds && !tie,
            }
        })
        .collect()
}

fn scoreboard_json(roster: &[PlayerState], me: Option<&str>) -> String {
    let rows: Vec<ScoreboardRow> = roster
        .iter()
        .map(|p| ScoreboardRow {
            name: &p.name,
            team_num: p.team_num,
            score: p.score,
            goals: p.goals,
            assists: p.assists,
            saves: p.saves,
            shots: p.shots,
            demos: p.demos,
            me: me.is_some_and(|id| !id.is_empty() && p.primary_id == id),
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{GameState, TeamState};
    use crate::facts::MatchResult;
    use crate::live::CompareStat;

    const ME: &str = "Epic|me|0";

    fn me(boost: f64, supersonic: bool) -> PlayerState {
        PlayerState {
            name: "Me".into(),
            primary_id: ME.into(),
            team_num: 1,
            score: 300,
            goals: 1,
            shots: 2,
            saves: 1,
            boost,
            supersonic,
            ..Default::default()
        }
    }

    fn opponent(name: &str) -> PlayerState {
        PlayerState {
            name: name.into(),
            primary_id: format!("Epic|{name}|0"),
            team_num: 0,
            ..Default::default()
        }
    }

    fn update(players: Vec<PlayerState>, playlist: Option<i32>, replay: bool) -> UpdateState {
        UpdateState {
            match_guid: "G".into(),
            players,
            game: GameState {
                teams: vec![
                    TeamState {
                        team_num: 0,
                        score: 1,
                        ..Default::default()
                    },
                    TeamState {
                        team_num: 1,
                        score: 3,
                        ..Default::default()
                    },
                ],
                playlist_id: playlist,
                arena: "Stadium_P".into(),
                replay,
                ..Default::default()
            },
        }
    }

    #[test]
    fn records_a_win_with_my_scoreboard() {
        let mut m = MatchTracker::new("G".into(), true);
        m.update(
            0,
            &update(vec![me(50.0, false), opponent("a")], Some(2), false),
            Some(ME),
        );
        m.epic_save();
        m.ended(Some(1));
        let r = m.finish();
        assert_eq!(r.result, MatchResult::Win);
        assert_eq!((r.team_score, r.opponent_score), (3, 1));
        assert_eq!(
            (r.score, r.goals, r.shots, r.saves, r.epic_saves),
            (300, 1, 2, 1, 1)
        );
        assert_eq!(
            (r.ranked, r.team_size, r.arena.as_str()),
            (false, Some(2), "Stadium_P")
        );
        assert!(!r.metrics_partial);
        assert!(r.scoreboard_json.contains("\"name\":\"a\""));
    }

    #[test]
    fn loss_and_unknown_results() {
        let mut lost = MatchTracker::new("G".into(), true);
        lost.update(0, &update(vec![me(0.0, false)], Some(2), false), Some(ME));
        lost.ended(Some(0));
        assert_eq!(lost.finish().result, MatchResult::Loss);

        let mut left_level = MatchTracker::new("G".into(), true);
        let mut level = update(vec![me(0.0, false)], Some(2), false);
        level.game.teams[0].score = 3;
        left_level.update(0, &level, Some(ME));
        assert_eq!(
            left_level.finish().result,
            MatchResult::Unknown,
            "3–3, no winner"
        );
    }

    #[test]
    fn boost_metrics_are_time_weighted() {
        let mut m = MatchTracker::new("G".into(), true);
        m.update(0, &update(vec![me(100.0, true)], Some(2), false), Some(ME));
        m.update(
            500,
            &update(vec![me(100.0, true)], Some(2), false),
            Some(ME),
        );
        m.update(
            1_000,
            &update(vec![me(0.0, false)], Some(2), false),
            Some(ME),
        );
        let r = m.finish();
        assert_eq!(
            (r.avg_boost, r.pct_zero_boost, r.pct_supersonic),
            (50.0, 50.0, 50.0)
        );
    }

    #[test]
    fn goal_replays_do_not_count_toward_metrics() {
        let mut m = MatchTracker::new("G".into(), true);
        m.update(0, &update(vec![me(100.0, false)], Some(2), false), Some(ME));
        m.update(
            100,
            &update(vec![me(100.0, false)], Some(2), false),
            Some(ME),
        );
        m.update(200, &update(vec![me(0.0, false)], Some(2), true), Some(ME));
        m.update(300, &update(vec![me(0.0, false)], Some(2), false), Some(ME));
        assert_eq!(m.finish().avg_boost, 100.0);
    }

    #[test]
    fn joining_late_or_without_me_is_partial() {
        let mut late = MatchTracker::new("G".into(), false);
        late.update(0, &update(vec![me(10.0, false)], Some(2), false), Some(ME));
        assert!(late.finish().metrics_partial);

        let mut unknown_me = MatchTracker::new("G".into(), true);
        unknown_me.update(0, &update(vec![me(10.0, false)], Some(2), false), None);
        let r = unknown_me.finish();
        assert!(r.metrics_partial);
        assert_eq!(r.result, MatchResult::Unknown);
    }

    #[test]
    fn players_who_leave_before_the_end_stay_on_the_scoreboard() {
        // Ranked 1v1 in the evening recording: the opponent left 0.1 s before MatchEnded.
        let mut m = MatchTracker::new("G".into(), true);
        m.update(
            0,
            &update(vec![me(0.0, false), opponent("a")], None, false),
            Some(ME),
        );
        m.update(100, &update(vec![me(0.0, false)], None, false), Some(ME));
        let r = m.finish();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&r.scoreboard_json).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(r.team_size, Some(1));
    }

    #[test]
    fn team_size_falls_back_to_player_count() {
        let mut m = MatchTracker::new("G".into(), true);
        let players = vec![me(0.0, false), opponent("a"), opponent("b"), opponent("c")];
        m.update(0, &update(players, None, false), Some(ME));
        assert_eq!(m.finish().team_size, Some(3));
    }

    #[test]
    fn without_a_reported_winner_the_final_score_decides() {
        // Ranked 2v2 on 2026-10-08: the team behind forfeited and no winner was reported.
        let mut ahead = MatchTracker::new("G".into(), true);
        ahead.update(0, &update(vec![me(0.0, false)], Some(11), false), Some(ME));
        ahead.ended(None);
        assert_eq!(ahead.finish().result, MatchResult::Win, "3–1 for my team");

        let mut level = MatchTracker::new("G".into(), true);
        let mut u = update(vec![me(0.0, false)], Some(11), false);
        u.game.teams[0].score = 3;
        level.update(0, &u, Some(ME));
        level.ended(None);
        assert_eq!(level.finish().result, MatchResult::Unknown);
    }

    #[test]
    fn the_winner_named_in_the_game_state_counts() {
        let mut m = MatchTracker::new("G".into(), true);
        let mut u = update(vec![me(0.0, false)], Some(11), false);
        u.game.teams[0].name = "Blue".into();
        u.game.teams[1].name = "Orange".into();
        u.game.has_winner = true;
        u.game.winner = "Blue".into();
        m.update(0, &u, Some(ME));
        assert_eq!(
            m.finish().result,
            MatchResult::Loss,
            "Blue won though Orange (mine) led"
        );
    }

    #[test]
    fn paused_and_post_match_time_is_not_weighted() {
        let mut m = MatchTracker::new("G".into(), true);
        let at = |m: &mut MatchTracker, ms: i64, boost: f64| {
            m.update(
                ms,
                &update(vec![me(boost, false)], Some(11), false),
                Some(ME),
            );
        };
        at(&mut m, 0, 0.0);
        at(&mut m, 100, 0.0); // 100 ms at zero boost
        m.set_paused(true);
        at(&mut m, 5_000, 100.0); // paused: ignored
        m.set_paused(false);
        at(&mut m, 5_100, 100.0); // first sample after a pause credits nothing
        at(&mut m, 5_200, 100.0); // 100 ms at full boost
        m.ended(Some(1));
        at(&mut m, 6_000, 0.0); // podium: ignored
        let r = m.finish();
        assert!((r.avg_boost - 50.0).abs() < 1e-9);
        assert!((r.pct_zero_boost - 50.0).abs() < 1e-9);
        assert_eq!(r.result, MatchResult::Win);
    }

    #[test]
    fn compares_each_stat_with_the_best_in_the_lobby() {
        let mut m = MatchTracker::new("G".into(), true);
        let mine = PlayerState {
            score: 420,
            goals: 2,
            assists: 1,
            saves: 3,
            shots: 3,
            ..me(30.0, false)
        };
        let kuro = PlayerState {
            score: 515,
            shots: 4,
            assists: 1,
            ..opponent("kuro")
        };
        let vex = PlayerState {
            saves: 5,
            ..opponent("vex")
        };
        m.update(0, &update(vec![mine, kuro, vex], Some(11), false), Some(ME));
        let rows = m.live().compare;
        let row = |stat: CompareStat| rows.iter().find(|r| r.stat == stat).unwrap().clone();
        assert_eq!(rows.len(), 5);
        let score = row(CompareStat::Score);
        assert_eq!(
            (
                score.mine,
                score.best,
                score.best_name.as_deref(),
                score.tie,
                score.mine_is_best
            ),
            (420, 515, Some("kuro"), false, false)
        );
        let goals = row(CompareStat::Goals);
        assert_eq!(
            (
                goals.mine,
                goals.best,
                goals.best_name,
                goals.tie,
                goals.mine_is_best
            ),
            (2, 2, None, false, true)
        );
        let assists = row(CompareStat::Assists);
        assert!(assists.tie && !assists.mine_is_best && assists.best_name.is_none());
        assert_eq!(row(CompareStat::Saves).best_name.as_deref(), Some("vex"));
    }

    #[test]
    fn players_who_left_still_count_for_the_best() {
        let mut m = MatchTracker::new("G".into(), true);
        let leaver = PlayerState {
            saves: 6,
            ..opponent("gone")
        };
        m.update(
            0,
            &update(vec![me(30.0, false), leaver], Some(11), false),
            Some(ME),
        );
        m.update(
            1_000,
            &update(vec![me(30.0, false)], Some(11), false),
            Some(ME),
        );
        let saves = m
            .live()
            .compare
            .into_iter()
            .find(|r| r.stat == CompareStat::Saves)
            .unwrap();
        assert_eq!((saves.best, saves.best_name.as_deref()), (6, Some("gone")));
    }

    #[test]
    fn hardest_hit_supersonic_share_and_score_are_live() {
        let mut m = MatchTracker::new("G".into(), true);
        m.update(0, &update(vec![me(30.0, true)], Some(11), false), Some(ME));
        m.update(
            1_000,
            &update(vec![me(30.0, true)], Some(11), false),
            Some(ME),
        );
        m.update(
            2_000,
            &update(vec![me(30.0, false)], Some(11), false),
            Some(ME),
        );
        m.ball_hit(84.5);
        m.ball_hit(61.0);
        let live = m.live();
        assert_eq!(live.hardest_hit, 84.5);
        assert!((live.pct_supersonic - 50.0).abs() < 0.01);
        assert_eq!(live.score, 300);
    }

    #[test]
    fn the_scoreboard_flags_my_row_once() {
        let mut m = MatchTracker::new("G".into(), true);
        let twin = PlayerState {
            name: "Twin".into(),
            primary_id: "Epic|twin|0".into(),
            team_num: 0,
            score: 300,
            goals: 1,
            shots: 2,
            saves: 1,
            ..Default::default()
        };
        m.update(
            0,
            &update(vec![me(50.0, false), twin], Some(2), false),
            Some(ME),
        );
        m.ended(Some(1));
        let r = m.finish();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&r.scoreboard_json).unwrap();
        let flagged: Vec<&str> = rows
            .iter()
            .filter(|row| row["me"] == true)
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        assert_eq!(flagged, ["Me"], "the twin with my stat line is not me");
    }
}
