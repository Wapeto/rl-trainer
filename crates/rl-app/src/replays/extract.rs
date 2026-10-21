//! One replay → my `ReplayStats`, with boxcars (parsing) and subtr-actor (stats). The stats
//! collector's output is read as JSON: its module types are not public, and the spike read the
//! same JSON paths (spikes/replays/FINDINGS.md §4).

use rl_core::store::past_games::ReplayOutcome;
use rl_core::store::replays::{PlayerReplayStats, ReplayPlayer, ReplayStats, LOW_BOOST};
use serde_json::Value;

pub const PARSER_VERSION: &str = "subtr-actor 1.4.0";

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("the replay could not be read: {0}")]
    Parse(String),
    #[error("the replay's stats could not be worked out: {0}")]
    Stats(String),
}

/// A player's id as replays write it: `{"Epic": "<id>"}`, `{"Steam": "<id>"}`, …
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerKey {
    platform: String,
    id: String,
}

impl PlayerKey {
    /// From the Stats API's `PrimaryId` (`Epic|<id>|0`), as stored in `profile.primary_id`.
    pub fn from_primary_id(primary_id: &str) -> Option<Self> {
        let mut parts = primary_id.split('|');
        let platform = parts.next()?.trim();
        let id = parts.next()?.trim();
        (!platform.is_empty() && !id.is_empty()).then(|| Self {
            platform: platform.to_owned(),
            id: id.to_owned(),
        })
    }

    /// From a replay's player id (`{"Epic": "<id>"}`, `{"Steam": 7656…}`).
    pub fn from_remote_id(value: &Value) -> Option<Self> {
        let (platform, id) = value.as_object()?.iter().next()?;
        let id = match id {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return None,
        };
        (!id.is_empty()).then(|| Self {
            platform: platform.clone(),
            id,
        })
    }

    /// Back to the Stats API's form, as stored in `profile.primary_id`.
    pub fn primary_id(&self) -> String {
        format!("{}|{}|0", self.platform, self.id)
    }

    fn matches(&self, value: &Value) -> bool {
        match value.get(&self.platform) {
            Some(Value::String(s)) => s.eq_ignore_ascii_case(&self.id),
            Some(Value::Number(n)) => n.to_string() == self.id,
            _ => false,
        }
    }
}

pub fn read_guid(bytes: &[u8]) -> Result<Option<String>, ExtractError> {
    let replay = boxcars::ParserBuilder::new(bytes)
        .never_parse_network_data()
        .parse()
        .map_err(|e| ExtractError::Parse(e.to_string()))?;
    Ok(replay
        .properties
        .iter()
        .find(|(k, _)| k == "MatchGUID")
        .and_then(|(_, v)| v.as_string())
        .map(str::to_owned))
}

/// The replay's human (not bot) players, in team order. Tests pick their "me" from it, so no
/// account id is written into code; the CLI's `replay-header` prints them.
pub fn human_keys(bytes: &[u8]) -> Result<Vec<PlayerKey>, ExtractError> {
    let replay = boxcars::ParserBuilder::new(bytes)
        .must_parse_network_data()
        .parse()
        .map_err(|e| ExtractError::Parse(e.to_string()))?;
    let stats = subtr_actor::StatsCollector::new()
        .get_stats(&replay)
        .map_err(|e| ExtractError::Stats(format!("{e:?}")))?;
    let json = serde_json::to_value(&stats).map_err(|e| ExtractError::Stats(e.to_string()))?;
    let meta = &json["replay_meta"];
    Ok(["team_zero", "team_one"]
        .iter()
        .flat_map(|team| meta[*team].as_array().into_iter().flatten())
        .filter(|p| !p["stats"]["bBot"].as_bool().unwrap_or(false))
        .filter_map(|p| PlayerKey::from_remote_id(&p["remote_id"]))
        .collect())
}

/// My coach stats and every player's page stats, from one parse and one stats pass.
#[derive(Debug, Clone, PartialEq)]
pub struct Extracted {
    pub mine: ReplayStats,
    pub players: Vec<ReplayPlayer>,
    /// The final score from the header; `None` when my team is not known.
    pub outcome: Option<ReplayOutcome>,
    pub game: GameHeader,
}

/// What the header says about the game itself (spike: spikes/replays/FINDINGS.md).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameHeader {
    /// The header `Date` (UTC), when the replay was saved: the end of the game.
    pub ended_at_ms: Option<i64>,
    pub seconds_played: Option<f64>,
    pub team_size: Option<i64>,
    pub arena: Option<String>,
}

fn prop<'a>(replay: &'a boxcars::Replay, key: &str) -> Option<&'a boxcars::HeaderProp> {
    replay
        .properties
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

fn game_header(replay: &boxcars::Replay) -> GameHeader {
    GameHeader {
        ended_at_ms: prop(replay, "Date")
            .and_then(|p| p.as_string())
            .and_then(|d| chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H-%M-%S").ok())
            .map(|d| d.and_utc().timestamp_millis()),
        seconds_played: prop(replay, "TotalSecondsPlayed")
            .and_then(|p| p.as_float())
            .map(f64::from),
        team_size: prop(replay, "TeamSize")
            .and_then(|p| p.as_i32())
            .map(i64::from),
        arena: prop(replay, "MapName")
            .and_then(|p| p.as_string())
            .map(str::to_owned),
    }
}

/// The header leaves a team's score out when it is 0.
fn outcome(replay: &boxcars::Replay, players: &[ReplayPlayer]) -> Option<ReplayOutcome> {
    let my_team = players.iter().find(|p| p.is_me)?.team;
    let score = |key| {
        prop(replay, key)
            .and_then(|p| p.as_i32())
            .map_or(0, i64::from)
    };
    let (mine, theirs) = if my_team == 0 {
        (score("Team0Score"), score("Team1Score"))
    } else {
        (score("Team1Score"), score("Team0Score"))
    };
    Some(ReplayOutcome {
        my_score: mine,
        their_score: theirs,
    })
}

pub fn extract(bytes: &[u8], me: &PlayerKey) -> Result<Option<Extracted>, ExtractError> {
    let replay = boxcars::ParserBuilder::new(bytes)
        .must_parse_network_data()
        .parse()
        .map_err(|e| ExtractError::Parse(e.to_string()))?;
    let data = subtr_actor::ReplayDataCollector::new()
        .get_replay_data(&replay)
        .map_err(|e| ExtractError::Stats(format!("{e:?}")))?;
    let boosts = boosts(&data)?;
    let Some((_, my_boost)) = boosts.iter().find(|(id, _)| me.matches(id)) else {
        return Ok(None);
    };
    let stats = subtr_actor::StatsCollector::new()
        .get_stats(&replay)
        .map_err(|e| ExtractError::Stats(format!("{e:?}")))?;
    let json = serde_json::to_value(&stats).map_err(|e| ExtractError::Stats(e.to_string()))?;
    let players = players(&json, &boosts, me);
    Ok(Some(Extracted {
        mine: from_modules(&json["modules"], my_boost, me),
        outcome: outcome(&replay, &players),
        game: game_header(&replay),
        players,
    }))
}

/// Each player's boost per frame on the 0–100 scale (`None` while they have no car), keyed by
/// the replay's player id. Raw replay boost is 0–255.
fn boosts(data: &subtr_actor::ReplayData) -> Result<Vec<(Value, Vec<Option<f64>>)>, ExtractError> {
    data.frame_data
        .players
        .iter()
        .map(|(id, player)| {
            let id = serde_json::to_value(id).map_err(|e| ExtractError::Stats(e.to_string()))?;
            let boost = player
                .frames()
                .iter()
                .map(|f| match f {
                    subtr_actor::PlayerFrame::Data { boost_amount, .. } => {
                        Some(f64::from(*boost_amount) / 2.55)
                    }
                    _ => None,
                })
                .collect();
            Ok((id, boost))
        })
        .collect()
}

/// Every player in the replay's order: team 0, then team 1.
fn players(
    json: &Value,
    boosts: &[(Value, Vec<Option<f64>>)],
    me: &PlayerKey,
) -> Vec<ReplayPlayer> {
    let meta = &json["replay_meta"];
    [("team_zero", 0), ("team_one", 1)]
        .iter()
        .flat_map(|(key, team)| {
            meta[*key]
                .as_array()
                .into_iter()
                .flatten()
                .map(move |p| (*team, p))
        })
        .enumerate()
        .map(|(slot, (team, p))| {
            let id = &p["remote_id"];
            let boost = boosts
                .iter()
                .find(|(b, _)| b == id)
                .map_or(&[][..], |(_, b)| b.as_slice());
            ReplayPlayer {
                slot: slot as i64,
                team,
                name: p["name"].as_str().unwrap_or("?").to_owned(),
                is_me: me.matches(id),
                is_bot: p["stats"]["bBot"].as_bool().unwrap_or(false),
                stats: player_stats(&json["modules"], &p["stats"], id, boost),
            }
        })
        .collect()
}

/// One player's page stats (spec §3). A module with no entry for the player means none happened.
fn player_stats(m: &Value, header: &Value, id: &Value, boost: &[Option<f64>]) -> PlayerReplayStats {
    let null = Value::Null;
    let of = |module: &str| {
        m[module]["player_stats"]
            .as_array()
            .and_then(|ps| ps.iter().find(|p| p["player_id"] == *id))
            .map_or(&null, |p| &p["stats"])
    };
    let (pos, mv, boo, rot) = (
        of("positioning"),
        of("movement"),
        of("boost"),
        of("rotation"),
    );
    let (touch, fifty, whiff) = (of("touch"), of("fifty_fifty"), of("whiff"));
    let n = |v: &Value, k: &str| Some(num(v, k));
    let share = |v: &Value, k: &str| {
        let t = num(v, "tracked_time");
        (t > 0.0).then(|| num(v, k) * 100.0 / t)
    };
    let boost_units = |k: &str| Some(num(boo, k) / 2.55);
    let low_touches = (!boost.is_empty()).then(|| {
        events(&m["touch"], "events")
            .filter(|e| e["player"] == *id)
            .filter(|e| boost_at(boost, num(e, "frame") as usize) < LOW_BOOST)
            .count() as f64
    });
    let pos_t = num(pos, "tracked_time");
    let mv_t = num(mv, "tracked_time");
    PlayerReplayStats {
        tracked_seconds: Some(pos_t),
        score: n(header, "Score"),
        goals: n(header, "Goals"),
        assists: n(header, "Assists"),
        saves: n(header, "Saves"),
        shots: n(header, "Shots"),
        pct_behind_ball: share(pos, "time_behind_ball"),
        pct_in_front_of_ball: share(pos, "time_in_front_of_ball"),
        pct_defensive_third: share(pos, "time_defensive_third"),
        pct_neutral_third: share(pos, "time_neutral_third"),
        pct_offensive_third: share(pos, "time_offensive_third"),
        pct_most_back: share(pos, "time_most_back"),
        pct_shadow_defense: share(pos, "time_shadow_defense"),
        avg_distance_to_ball_m: (pos_t > 0.0)
            .then(|| num(pos, "sum_distance_to_ball") / pos_t / 100.0),
        secs_first_man: n(rot, "time_first_man"),
        secs_second_man: n(rot, "time_second_man"),
        secs_third_man: n(rot, "time_third_man"),
        became_first_man: n(rot, "became_first_man_count"),
        boost_collected: boost_units("amount_collected"),
        boost_used: boost_units("amount_used"),
        boost_stolen: boost_units("amount_stolen"),
        big_pads: n(boo, "big_pads_collected"),
        small_pads: n(boo, "small_pads_collected"),
        pct_zero_boost: share(boo, "time_zero_boost"),
        pct_full_boost: share(boo, "time_hundred_boost"),
        boost_used_supersonic: boost_units("amount_used_while_supersonic"),
        touches_low_boost: low_touches,
        touches: n(touch, "touch_count"),
        hard_hits: n(touch, "hard_hit_count"),
        aerial_touches: n(touch, "aerial_touch_count"),
        control_touches: n(touch, "control_touch_count"),
        first_touches: n(touch, "first_touch_count"),
        fifty_wins: Some(num(fifty, "wins") - num(fifty, "kickoff_wins")),
        fifty_losses: Some(num(fifty, "losses") - num(fifty, "kickoff_losses")),
        fifty_neutral: Some(
            num(fifty, "neutral_outcomes") - num(fifty, "kickoff_neutral_outcomes"),
        ),
        kickoff_wins: n(fifty, "kickoff_wins"),
        kickoff_losses: n(fifty, "kickoff_losses"),
        beaten_to_ball: n(whiff, "beaten_to_ball_count"),
        whiffs: n(whiff, "whiff_count"),
        flicks: n(of("flick"), "count"),
        wavedashes: n(of("wavedash"), "count"),
        half_flips: n(of("half_flip"), "count"),
        speed_flips: n(of("speed_flip"), "count"),
        avg_speed_kmh: (mv_t > 0.0).then(|| num(mv, "speed_integral") / mv_t * 0.036),
        pct_supersonic: share(mv, "time_supersonic_speed"),
        pct_on_ground: share(mv, "time_on_ground"),
        pct_high_air: share(mv, "time_high_air"),
        demos_inflicted: n(of("demo"), "demos_inflicted"),
        demos_taken: n(of("demo"), "demos_taken"),
        bumps_inflicted: n(of("bump"), "bumps_inflicted"),
        bumps_taken: n(of("bump"), "bumps_taken"),
        passes_completed: n(of("pass"), "completed_pass_count"),
        passes_received: n(of("pass"), "received_pass_count"),
    }
}

fn boost_at(boost: &[Option<f64>], frame: usize) -> f64 {
    let end = frame.min(boost.len().saturating_sub(1));
    boost[..=end].iter().rev().find_map(|b| *b).unwrap_or(0.0)
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn mine<'a>(module: &'a Value, me: &PlayerKey) -> Option<&'a Value> {
    module["player_stats"]
        .as_array()?
        .iter()
        .find(|p| me.matches(&p["player_id"]))
        .map(|p| &p["stats"])
}

fn events<'a>(module: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    module[key].as_array().into_iter().flatten()
}

fn from_modules(m: &Value, boost: &[Option<f64>], me: &PlayerKey) -> ReplayStats {
    let null = Value::Null;
    let pos = mine(&m["positioning"], me).unwrap_or(&null);
    let tracked = num(pos, "tracked_time");
    let pct = |k: &str| {
        if tracked > 0.0 {
            num(pos, k) * 100.0 / tracked
        } else {
            0.0
        }
    };
    let rot = mine(&m["rotation"], me);
    let picks: Vec<&Value> = events(&m["boost"], "pickup_events")
        .filter(|e| me.matches(&e["player_id"]))
        .collect();
    let touch_boost: Vec<f64> = events(&m["touch"], "events")
        .filter(|e| me.matches(&e["player"]))
        .map(|e| boost_at(boost, num(e, "frame") as usize))
        .collect();
    let count = |it: &mut dyn Iterator<Item = bool>| it.filter(|b| *b).count() as i64;
    let mut s = ReplayStats {
        tracked_seconds: tracked,
        pct_behind_ball: pct("time_behind_ball"),
        pct_in_front_of_ball: pct("time_in_front_of_ball"),
        pct_defensive_third: pct("time_defensive_third"),
        pct_neutral_third: pct("time_neutral_third"),
        pct_offensive_third: pct("time_offensive_third"),
        secs_first_man: rot.map(|r| num(r, "time_first_man")),
        secs_second_man: rot.map(|r| num(r, "time_second_man")),
        secs_third_man: rot.map(|r| num(r, "time_third_man")),
        became_first_man: rot.map(|r| num(r, "became_first_man_count") as i64),
        big_pads: mine(&m["boost"], me).map_or(0, |b| num(b, "big_pads_collected") as i64),
        small_pads: count(&mut picks.iter().map(|e| e["pad_type"] == "small")),
        pads_stolen: count(&mut picks.iter().map(|e| e["is_steal"] == true)),
        touches: touch_boost.len() as i64,
        touches_low_boost: count(&mut touch_boost.iter().map(|b| *b < LOW_BOOST)),
        touches_zero_boost: count(&mut touch_boost.iter().map(|b| *b < 1.0)),
        whiffs: mine(&m["whiff"], me).map_or(0, |w| num(w, "whiff_count") as i64),
        ..ReplayStats::default()
    };
    add_fifties(&mut s, m, boost, me);
    s
}

/// 50/50s: open play ones count as wins/losses (and low boost at their start), kickoffs apart.
fn add_fifties(s: &mut ReplayStats, m: &Value, boost: &[Option<f64>], me: &PlayerKey) {
    for e in events(&m["fifty_fifty"], "events") {
        let zero = me.matches(&e["team_zero_player"]);
        if !zero && !me.matches(&e["team_one_player"]) {
            continue;
        }
        let won = e["winning_team_is_team_0"].as_bool().map(|w| w == zero);
        if e["is_kickoff"].as_bool().unwrap_or(false) {
            s.kickoff_wins += i64::from(won == Some(true));
            s.kickoff_losses += i64::from(won == Some(false));
        } else {
            s.fifty_wins += i64::from(won == Some(true));
            s.fifty_losses += i64::from(won == Some(false));
            let start = num(e, "start_frame") as usize;
            s.fifty_low_boost += i64::from(boost_at(boost, start) < LOW_BOOST);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // From spikes/replays/expected.py on the fixture (Task 6 Step 3); no ids here.
    const EXPECTED_TOUCHES: i64 = 21;
    const EXPECTED_TOUCHES_LOW_BOOST: i64 = 9;
    const EXPECTED_TOUCHES_ZERO_BOOST: i64 = 5;
    const EXPECTED_BIG_PADS: i64 = 11;
    const EXPECTED_SMALL_PADS: i64 = 57;
    const EXPECTED_PADS_STOLEN: i64 = 19;
    const EXPECTED_FIFTIES: (i64, i64) = (2, 2);
    const EXPECTED_KICKOFFS: (i64, i64) = (1, 0);
    const EXPECTED_WHIFFS: i64 = 2;
    const EXPECTED_TRACKED: f64 = 308.779;
    const EXPECTED_BEHIND: f64 = 63.964;

    /// An online replay. It is not committed (it holds players' names and account ids), so the
    /// tests that need it skip, with a note, when it is missing.
    fn fixture() -> Option<Vec<u8>> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/match-2026-10.replay"
        );
        match std::fs::read(path) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                eprintln!("skipped: no replay fixture at {path}");
                None
            }
        }
    }

    /// The tests' "me": the fixture's first human player with an Epic id (as `expected.py`
    /// picks it), so no account id is written here.
    fn me(bytes: &[u8]) -> PlayerKey {
        human_keys(bytes)
            .unwrap()
            .into_iter()
            .find(|k| k.primary_id().starts_with("Epic|"))
            .expect("the fixture has an Epic player")
    }

    #[test]
    fn the_header_gives_the_match_guid_without_the_network_data() {
        let Some(bytes) = fixture() else { return };
        let guid = read_guid(&bytes)
            .unwrap()
            .expect("the fixture has a MatchGUID");
        assert_eq!(guid.len(), 32);
        assert!(guid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn my_stats_match_the_spikes_independent_numbers() {
        let Some(bytes) = fixture() else { return };
        let s = extract(&bytes, &me(&bytes))
            .unwrap()
            .expect("I played in the fixture")
            .mine;
        // Values from spikes/replays/expected.py on the same file (Task 6 Step 3).
        assert_eq!(s.touches, EXPECTED_TOUCHES);
        assert_eq!(s.touches_low_boost, EXPECTED_TOUCHES_LOW_BOOST);
        assert_eq!(s.touches_zero_boost, EXPECTED_TOUCHES_ZERO_BOOST);
        assert_eq!(s.big_pads, EXPECTED_BIG_PADS);
        assert_eq!(s.small_pads, EXPECTED_SMALL_PADS);
        assert_eq!(s.pads_stolen, EXPECTED_PADS_STOLEN);
        assert_eq!((s.fifty_wins, s.fifty_losses), EXPECTED_FIFTIES);
        assert_eq!((s.kickoff_wins, s.kickoff_losses), EXPECTED_KICKOFFS);
        assert_eq!(s.whiffs, EXPECTED_WHIFFS);
        assert!((s.tracked_seconds - EXPECTED_TRACKED).abs() < 0.01);
        assert!((s.pct_behind_ball - EXPECTED_BEHIND).abs() < 0.01);
        assert!((0.0..=100.0).contains(&s.pct_defensive_third));
    }

    #[test]
    fn the_header_gives_the_score_and_the_game() {
        let Some(bytes) = fixture() else { return };
        let got = extract(&bytes, &me(&bytes)).unwrap().unwrap();
        let o = got.outcome.expect("my team is known");
        assert_eq!(
            o.my_score + o.their_score,
            3,
            "the fixture ended 3-0 or 0-3"
        );
        assert!(
            o.my_score == 0 || o.their_score == 0,
            "a missing score reads as 0"
        );
        assert_eq!(got.game.team_size, Some(2));
        assert!(got.game.ended_at_ms.is_some() && got.game.seconds_played > Some(300.0));
    }

    #[test]
    fn someone_elses_replay_is_not_mine() {
        let Some(bytes) = fixture() else { return };
        let other = PlayerKey::from_primary_id("Epic|00000000000000000000000000000000|0").unwrap();
        assert!(extract(&bytes, &other).unwrap().is_none());
    }

    #[test]
    fn a_broken_file_is_an_error_not_a_panic() {
        assert!(read_guid(b"not a replay").is_err());
        let Some(bytes) = fixture() else { return };
        assert!(extract(&bytes[..bytes.len() / 2], &me(&bytes)).is_err());
    }

    #[test]
    fn primary_ids_split_into_platform_and_id_and_back() {
        let key = PlayerKey::from_primary_id("Epic|abc|0").unwrap();
        assert_eq!(key.primary_id(), "Epic|abc|0");
        assert!(PlayerKey::from_primary_id("Steam|7656|0").is_some());
        assert!(PlayerKey::from_primary_id("garbage").is_none());
        assert!(PlayerKey::from_primary_id("Epic||0").is_none());
        let steam = serde_json::json!({ "Steam": 76561197971842946_u64 });
        assert_eq!(
            PlayerKey::from_remote_id(&steam).unwrap().primary_id(),
            "Steam|76561197971842946|0"
        );
    }

    #[test]
    fn every_player_gets_stats_and_mine_agree_with_my_replay_stats() {
        let Some(bytes) = fixture() else { return };
        let me = me(&bytes);
        let got = extract(&bytes, &me)
            .unwrap()
            .expect("I played in the fixture");
        assert_eq!(got.players.len(), human_keys(&bytes).unwrap().len());
        assert_eq!(got.players.iter().filter(|p| p.is_me).count(), 1);
        assert!(got.players.iter().all(|p| !p.name.is_empty()));
        let mine = &got.players.iter().find(|p| p.is_me).unwrap().stats;
        let s = &got.mine;
        assert_eq!(mine.tracked_seconds, Some(s.tracked_seconds));
        assert_eq!(mine.touches, Some(s.touches as f64));
        assert_eq!(mine.touches_low_boost, Some(s.touches_low_boost as f64));
        assert_eq!(mine.fifty_wins, Some(s.fifty_wins as f64));
        assert_eq!(mine.fifty_losses, Some(s.fifty_losses as f64));
        assert_eq!(mine.kickoff_wins, Some(s.kickoff_wins as f64));
        assert_eq!(mine.whiffs, Some(s.whiffs as f64));
        assert_eq!(mine.big_pads, Some(s.big_pads as f64));
        // Pinned units (plan Global Constraints): metres, km/h, boost 0–100.
        let dist = mine.avg_distance_to_ball_m.unwrap();
        assert!((10.0..40.0).contains(&dist), "average distance {dist} m");
        let speed = mine.avg_speed_kmh.unwrap();
        assert!((30.0..80.0).contains(&speed), "average speed {speed} km/h");
        let collected = mine.boost_collected.unwrap();
        assert!(
            collected >= mine.big_pads.unwrap() * 50.0,
            "boost on the 0–100 scale"
        );
        assert!(
            collected <= mine.big_pads.unwrap() * 100.0 + mine.small_pads.unwrap() * 12.0 + 1.0
        );
    }

    #[test]
    fn a_player_with_no_boost_frames_has_no_low_boost_count() {
        let modules = serde_json::json!({});
        let header = serde_json::json!({ "Goals": 1 });
        let s = player_stats(&modules, &header, &serde_json::json!({"Epic": "x"}), &[]);
        assert_eq!(s.touches_low_boost, None);
        assert_eq!(s.goals, Some(1.0));
        assert_eq!(s.whiffs, Some(0.0), "no entry in a module: none happened");
        assert_eq!(s.pct_behind_ball, None, "no tracked time: no share");
        assert_eq!(
            s.tracked_seconds,
            Some(0.0),
            "stored, so the backfill doesn't re-read it at every start"
        );
    }
}
