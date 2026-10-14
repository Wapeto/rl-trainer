//! Keeps `ui/src/shared/generated/types.ts` and the UI's sample snapshots in step with the Rust
//! types. `UPDATE_GENERATED=1 cargo test -p rl-app ts_export` rewrites them; without the variable
//! the tests fail when a committed file is stale.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::app::StartupReport;
    use crate::backup::{BackupInfo, BackupKind};
    use crate::coach_samples;
    use crate::desktop::About;
    use crate::desktop::MatchReplay;
    use crate::desktop::ReplayStatus;
    use crate::pairing::PairingInfo;
    use crate::report::ReportInfo;
    use crate::settings::Settings;
    use rl_core::coach::benchmark::{BenchStat, Benchmark};
    use rl_core::coach::insights::Insight;
    use rl_core::coach::nudges::Nudge;
    use rl_core::coach::review::{ModeReview, ReviewItem, SessionReview, TrainingItem};
    use rl_core::coach::usual::ReviewUnit;
    use rl_core::live::{
        CompareStat, LiveBlock, LiveFreeplay, LiveMatch, LiveSession, LiveSnapshot, LiveTraining,
        MmrLatest, RecentAttempt, SessionSummary, StatCompare,
    };
    use rl_core::playlist::Activity;
    use rl_core::store::packs::{PackDetail, PackInput, PackSessionRow, PackSort};
    use rl_core::store::progress::{
        MmrPoint, ModeLane, ModeMmr, ModeProgress, ModeStat, Mover, OverviewProgress, PackLane,
        PackPoint, PackProgress, ProgressList, ProgressRange, SessionTime, StatPoint, StatSeries,
    };
    use rl_core::store::replays::{
        PlayerReplayStats, ReplayCounts, ReplayPlayer, ReplayStats, ReplayToSave,
    };
    use rl_core::store::sessions::{
        ActivityRow, FreeplaySummary, MatchSummary, MmrRow, Neighbour, PackRef, SessionCard,
        SessionDetail, SessionTotals,
    };
    use rl_core::store::ProfileRow;
    use rl_core::store::{MmrInput, PackRow, PackStats};
    use serde::Serialize;
    use ts_rs::{Config, TS};

    use crate::engine::AppSnapshot;
    use crate::game_watch::ini::StatsApiStatus;
    use crate::phone::api::ReviewBody;

    fn shared_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/shared")
    }

    fn check(relative: &str, content: &str) {
        let path = shared_dir().join(relative);
        if std::env::var_os("UPDATE_GENERATED").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            committed == content,
            "{} is stale; regenerate with: UPDATE_GENERATED=1 cargo test -p rl-app ts_export",
            path.display()
        );
    }

    /// `decl()` returns `type X = …` (possibly after a doc comment); the UI imports named types.
    fn exported(decl: &str) -> String {
        let decl = decl.trim_end();
        match decl.find("\ntype ") {
            Some(i) => format!("{}export {}", &decl[..=i], &decl[i + 1..]),
            None => format!("export {decl}"),
        }
    }

    fn declarations() -> String {
        let cfg = Config::new().with_large_int("number");
        let decls = [
            Activity::decl(&cfg),
            PackRow::decl(&cfg),
            PackStats::decl(&cfg),
            MmrInput::decl(&cfg),
            RecentAttempt::decl(&cfg),
            Nudge::decl(&cfg),
            LiveTraining::decl(&cfg),
            LiveFreeplay::decl(&cfg),
            CompareStat::decl(&cfg),
            StatCompare::decl(&cfg),
            LiveMatch::decl(&cfg),
            LiveSession::decl(&cfg),
            LiveBlock::decl(&cfg),
            MmrLatest::decl(&cfg),
            SessionSummary::decl(&cfg),
            LiveSnapshot::decl(&cfg),
            StatsApiStatus::decl(&cfg),
            AppSnapshot::decl(&cfg),
            ReviewBody::decl(&cfg),
            SessionCard::decl(&cfg),
            PackRef::decl(&cfg),
            FreeplaySummary::decl(&cfg),
            ReplayStats::decl(&cfg),
            PlayerReplayStats::decl(&cfg),
            ReplayPlayer::decl(&cfg),
            Insight::decl(&cfg),
            BenchStat::decl(&cfg),
            Benchmark::decl(&cfg),
            MatchReplay::decl(&cfg),
            MatchSummary::decl(&cfg),
            ActivityRow::decl(&cfg),
            MmrRow::decl(&cfg),
            SessionTotals::decl(&cfg),
            SessionDetail::decl(&cfg),
            ReplayToSave::decl(&cfg),
            ReplayCounts::decl(&cfg),
            ReviewUnit::decl(&cfg),
            TrainingItem::decl(&cfg),
            ReviewItem::decl(&cfg),
            ModeReview::decl(&cfg),
            SessionReview::decl(&cfg),
            ReplayStatus::decl(&cfg),
            Neighbour::decl(&cfg),
            PackSort::decl(&cfg),
            PackInput::decl(&cfg),
            PackSessionRow::decl(&cfg),
            PackDetail::decl(&cfg),
            Settings::decl(&cfg),
            BackupKind::decl(&cfg),
            BackupInfo::decl(&cfg),
            PairingInfo::decl(&cfg),
            StartupReport::decl(&cfg),
            ProfileRow::decl(&cfg),
            About::decl(&cfg),
            ReportInfo::decl(&cfg),
            ProgressRange::decl(&cfg),
            MmrPoint::decl(&cfg),
            SessionTime::decl(&cfg),
            PackPoint::decl(&cfg),
            PackProgress::decl(&cfg),
            ModeStat::decl(&cfg),
            StatPoint::decl(&cfg),
            StatSeries::decl(&cfg),
            ModeProgress::decl(&cfg),
            ModeLane::decl(&cfg),
            PackLane::decl(&cfg),
            ProgressList::decl(&cfg),
            ModeMmr::decl(&cfg),
            Mover::decl(&cfg),
            OverviewProgress::decl(&cfg),
        ];
        let mut out = String::from(
            "// Generated from the Rust types; do not edit.\n\
             // Regenerate: UPDATE_GENERATED=1 cargo test -p rl-app ts_export\n\n",
        );
        for decl in decls {
            out.push_str(&exported(&decl));
            out.push_str("\n\n");
        }
        out
    }

    #[test]
    fn generated_types_are_current() {
        check("generated/types.ts", &declarations());
    }

    // ---- sample snapshots for the UI tests (fixed times so renders are stable) ----

    const T0: i64 = 1_791_500_000_000;

    fn pack(id: i64, code: &str, name: &str, tags: &[&str]) -> PackRow {
        PackRow {
            id,
            code: code.into(),
            name: name.into(),
            creator: String::new(),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            shot_count: None,
            target_rate: 0.65,
            hidden: false,
            notes: String::new(),
            goalie: false,
        }
    }

    fn aerial() -> PackRow {
        pack(
            11,
            "C7E0-9E0B-B739-A899",
            "Aerial Shots - Pass",
            &["aerial", "shooting"],
        )
    }

    fn app(game_running: bool, live: LiveSnapshot) -> AppSnapshot {
        AppSnapshot {
            version: 1042,
            generation: 1,
            game_running,
            ingest_connected: game_running,
            stats_api: StatsApiStatus::Ok {
                rate: 10.0,
                port: 49123,
                web_port: Some(49124),
            },
            phone_clients: 1,
            replays: 0,
            live,
        }
    }

    fn latest_mmr() -> Vec<MmrLatest> {
        vec![
            MmrLatest {
                team_size: 1,
                mmr: 632,
                recorded_at: T0 - 86_400_000,
            },
            MmrLatest {
                team_size: 2,
                mmr: 899,
                recorded_at: T0 - 86_400_000,
            },
        ]
    }

    fn open_session() -> Option<LiveSession> {
        Some(LiveSession {
            id: 12,
            started_at: T0,
            notes: String::new(),
        })
    }

    fn last_session(reviewed: bool) -> SessionSummary {
        SessionSummary {
            id: 11,
            started_at: T0 - 90_000_000,
            ended_at: T0 - 82_500_000,
            training_blocks: 3,
            freeplay_blocks: 2,
            match_blocks: 4,
            other_blocks: 0,
            wins: 3,
            losses: 1,
            ranked_team_sizes: vec![1, 2],
            notes: "Nice warm-up 1v1 match".into(),
            activities: 9,
            training_make_rate: Some(0.64),
            training_target_rate: Some(0.65),
            reviewed,
        }
    }

    /// The open session (id 12) so far: a ranked 2v2 win not reviewed yet, its last activity
    /// ended at `ended_at`.
    fn this_session(ended_at: i64) -> SessionSummary {
        SessionSummary {
            id: 12,
            started_at: T0,
            ended_at,
            training_blocks: 2,
            freeplay_blocks: 1,
            match_blocks: 1,
            other_blocks: 0,
            wins: 1,
            losses: 0,
            ranked_team_sizes: vec![2],
            notes: String::new(),
            activities: 4,
            training_make_rate: Some(0.58),
            training_target_rate: Some(0.65),
            reviewed: false,
        }
    }

    /// `m` make, `x` miss, `-` skipped miss; oldest first.
    fn recent(outcomes: &str) -> Vec<RecentAttempt> {
        outcomes
            .chars()
            .enumerate()
            .map(|(i, c)| RecentAttempt {
                id: 500 + i as i64,
                seq: 15 + i as i64,
                outcome: if c == 'm' { "make" } else { "miss" }.into(),
                skipped: c == '-',
            })
            .collect()
    }

    fn block(kind: Activity, playlist_id: Option<i32>) -> LiveBlock {
        LiveBlock {
            id: 40,
            kind,
            playlist_id,
            started_at: T0 + 3_368_000,
            paused: false,
            pack: None,
            suggested_pack: None,
            training: None,
            freeplay: None,
            match_live: None,
        }
    }

    fn with_block(block: LiveBlock) -> LiveSnapshot {
        LiveSnapshot {
            session: open_session(),
            block: Some(block),
            latest_mmr: latest_mmr(),
            last_session: Some(last_session(true)),
            current_session: Some(this_session(T0 + 3_368_000)),
        }
    }

    fn training(attempts: i64, makes: i64, outcomes: &str, streak: i64) -> AppSnapshot {
        app(
            true,
            with_block(LiveBlock {
                pack: Some(aerial()),
                training: Some(LiveTraining {
                    attempts,
                    makes,
                    make_rate: Some(makes as f64 / attempts as f64),
                    streak,
                    recent: recent(outcomes),
                    nudge: None,
                }),
                ..block(Activity::Training, Some(21))
            }),
        )
    }

    fn compare(
        stat: CompareStat,
        mine: i32,
        best: i32,
        name: Option<&str>,
        tie: bool,
    ) -> StatCompare {
        StatCompare {
            stat,
            mine,
            best,
            best_name: name.map(str::to_owned),
            tie,
            mine_is_best: !tie && name.is_none() && mine == best,
        }
    }

    fn samples() -> Vec<(&'static str, AppSnapshot)> {
        let suggest = app(
            true,
            with_block(LiveBlock {
                suggested_pack: Some(aerial()),
                training: Some(LiveTraining::default()),
                ..block(Activity::Training, Some(21))
            }),
        );
        let freeplay = app(
            true,
            with_block(LiveBlock {
                freeplay: Some(LiveFreeplay {
                    touches: 146,
                    hard_hits: 23,
                    max_hit_speed: 112.4,
                    goals: 9,
                    pct_supersonic: 18.2,
                    pct_on_wall: 11.0,
                    pct_airborne: 24.4,
                    best_today_hit: 118.0,
                }),
                ..block(Activity::Freeplay, Some(9))
            }),
        );
        let in_match = app(
            true,
            with_block(LiveBlock {
                match_live: Some(LiveMatch {
                    playlist_id: Some(11),
                    ranked: true,
                    team_size: Some(2),
                    arena: "Stadium_P".into(),
                    team_score: 2,
                    opponent_score: 1,
                    clock_seconds: 167,
                    overtime: false,
                    goals: 1,
                    assists: 1,
                    saves: 3,
                    shots: 3,
                    score: 420,
                    hardest_hit: 104.0,
                    pct_supersonic: 21.0,
                    compare: vec![
                        compare(CompareStat::Score, 420, 515, Some("Kuro"), false),
                        compare(CompareStat::Goals, 1, 1, None, false),
                        compare(CompareStat::Assists, 1, 1, None, true),
                        compare(CompareStat::Saves, 3, 5, Some("Vex"), false),
                        compare(CompareStat::Shots, 3, 4, Some("Kuro"), false),
                    ],
                    avg_boost: 38.0,
                    pct_zero_boost: 14.0,
                    today_wins: 3,
                    today_losses: 1,
                }),
                ..block(Activity::Match, Some(11))
            }),
        );
        let ended = |reviewed: bool| LiveSnapshot {
            session: None,
            block: None,
            latest_mmr: latest_mmr(),
            last_session: Some(last_session(reviewed)),
            current_session: None,
        };
        // In the menus 20 s after a ranked 2v2 (FIXTURE_NOW is T0 + 3 880 000 in the UI).
        let menus = LiveSnapshot {
            session: open_session(),
            block: None,
            latest_mmr: latest_mmr(),
            last_session: Some(last_session(true)),
            current_session: Some(this_session(T0 + 3_860_000)),
        };
        vec![
            ("training-below", training(24, 14, "mxmmx-mxmm", 2)),
            ("training-above", training(24, 17, "mmxmmmxmmm", 3)),
            ("training-suggest", suggest),
            ("training-nudge", {
                let mut s = training(41, 9, "XXXXXXXXXX", 0);
                if let Some(b) = s.live.block.as_mut() {
                    if let Some(t) = b.training.as_mut() {
                        t.nudge = Some(Nudge::RateDrop {
                            fired_at_attempt: 412,
                            last20_rate: 0.1,
                            best20_rate: 0.4,
                        });
                    }
                }
                s
            }),
            ("freeplay", freeplay),
            ("match", in_match),
            ("session-end", app(false, ended(false))),
            ("idle", app(false, ended(true))),
            ("idle-empty", app(false, LiveSnapshot::default())),
            ("menus", app(true, menus)),
        ]
    }

    fn json<T: Serialize>(value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap() + "\n"
    }

    #[test]
    fn ui_fixtures_are_current() {
        for (name, snapshot) in samples() {
            check(&format!("fixtures/{name}.json"), &json(&snapshot));
        }
        let packs = vec![
            PackStats {
                pack: pack(
                    5,
                    "5BFE-60D6-0D59-79F2",
                    "Aerial Off Wall",
                    &["aerial", "wall"],
                ),
                attempts: 0,
                makes: 0,
                make_rate: None,
                last_trained_at: None,
            },
            PackStats {
                pack: aerial(),
                attempts: 50,
                makes: 32,
                make_rate: Some(0.64),
                last_trained_at: Some(T0),
            },
            PackStats {
                pack: pack(
                    4,
                    "07E1-81BC-DD2E-BF8C",
                    "Backboard Reads",
                    &["defence", "backboard"],
                ),
                attempts: 0,
                makes: 0,
                make_rate: None,
                last_trained_at: None,
            },
            PackStats {
                pack: pack(6, "55C9-36FE-613D-7F12", "Consistency", &["shooting"]),
                attempts: 58,
                makes: 0,
                make_rate: Some(0.0),
                last_trained_at: Some(T0 - 3_000_000),
            },
        ];
        check("fixtures/packs.json", &json(&packs));
    }

    // ---- desktop samples (Thursday 8 October 2026, 20:14 local time is not assumed: the UI
    // tests format with the machine's zone, so they assert on relative text only) ----

    const D0: i64 = 1_791_489_240_000;

    fn activity(id: i64, kind: Activity, start_min: i64, end_min: Option<i64>) -> ActivityRow {
        ActivityRow {
            id,
            kind,
            playlist_id: None,
            started_at: D0 + start_min * 60_000,
            ended_at: end_min.map(|m| D0 + m * 60_000),
            pack: None,
            attempts: 0,
            makes: 0,
            freeplay: None,
            match_summary: None,
        }
    }

    fn ranked(
        id: i64,
        start: i64,
        result: &str,
        us: i64,
        them: i64,
        arena: &str,
        overtime: bool,
    ) -> ActivityRow {
        ActivityRow {
            match_summary: Some(MatchSummary {
                ranked: true,
                team_size: Some(2),
                arena: arena.into(),
                result: result.into(),
                team_score: us,
                opponent_score: them,
                overtime,
                score: 300 + us * 50,
                goals: us.min(2),
                assists: 1,
                saves: 2,
                match_id: id,
                history_place: id - 12,
                replay: (id == 13).then_some(ReplayStats {
                    tracked_seconds: 304.0,
                    pct_behind_ball: 54.0,
                    pct_in_front_of_ball: 23.0,
                    pct_defensive_third: 47.0,
                    pct_neutral_third: 44.0,
                    pct_offensive_third: 9.0,
                    secs_first_man: None,
                    secs_second_man: None,
                    secs_third_man: None,
                    became_first_man: None,
                    big_pads: 19,
                    small_pads: 31,
                    pads_stolen: 4,
                    touches: 67,
                    touches_low_boost: 33,
                    touches_zero_boost: 12,
                    fifty_wins: 15,
                    fifty_losses: 6,
                    fifty_low_boost: 4,
                    kickoff_wins: 2,
                    kickoff_losses: 3,
                    whiffs: 7,
                }),
            }),
            ..activity(id, Activity::Match, start, Some(start + 9))
        }
    }

    fn detail(open: bool) -> SessionDetail {
        let activities = vec![
            ActivityRow {
                freeplay: Some(FreeplaySummary {
                    touches: 64,
                    hard_hits: 9,
                    max_hit_speed: 118.0,
                }),
                ..activity(10, Activity::Freeplay, 0, Some(12))
            },
            ActivityRow {
                pack: Some(PackRef {
                    id: 11,
                    name: "Aerial Shots - Pass".into(),
                    code: "C7E0-9E0B-B739-A899".into(),
                    target_rate: 0.65,
                }),
                attempts: 62,
                makes: 41,
                ..activity(11, Activity::Training, 13, Some(37))
            },
            ActivityRow {
                attempts: 84,
                makes: 44,
                ..activity(12, Activity::Training, 38, Some(59))
            },
            ranked(13, 66, "win", 3, 1, "Stadium_P", false),
            ranked(14, 75, "loss", 1, 2, "EuroStadium_P", true),
            ActivityRow {
                ended_at: if open { None } else { Some(D0 + 98 * 60_000) },
                ..ranked(15, 84, "win", 4, 2, "UtopiaStadium_P", false)
            },
        ];
        SessionDetail {
            id: 12,
            started_at: D0,
            ended_at: if open { None } else { Some(D0 + 98 * 60_000) },
            notes: "Felt good on aerials, rotations sloppy in 2s.".into(),
            needs_review: !open,
            previous_id: Some(11),
            next_id: None,
            activities,
            mmr: vec![MmrRow {
                id: 7,
                team_size: 2,
                mmr: 912,
                recorded_at: D0 + 98 * 60_000,
                change: Some(13),
            }],
            totals: SessionTotals {
                attempts: 146,
                makes: 85,
                target_rate: Some(0.65),
                wins: 2,
                losses: 1,
                freeplay_ms: 720_000,
            },
        }
    }

    fn cards() -> Vec<SessionCard> {
        let card = |id: i64,
                    start: i64,
                    minutes: i64,
                    packs: i64,
                    ranked: i64,
                    freeplay: i64,
                    needs_review: bool| SessionCard {
            id,
            started_at: start,
            ended_at: Some(start + minutes * 60_000),
            packs,
            ranked_matches: ranked,
            casual_matches: 0,
            freeplay_activities: freeplay,
            needs_review,
        };
        vec![
            card(12, D0, 98, 2, 3, 1, false),
            card(11, D0 - 26 * 3_600_000, 68, 1, 2, 0, true),
            card(10, D0 - 30 * 3_600_000, 25, 0, 0, 1, false),
        ]
    }

    fn pack_stats(pack: PackRow, attempts: i64, makes: i64, last: Option<i64>) -> PackStats {
        PackStats {
            pack,
            attempts,
            makes,
            make_rate: (attempts > 0).then(|| makes as f64 / attempts as f64),
            last_trained_at: last,
        }
    }

    fn desktop_samples() -> Vec<(&'static str, serde_json::Value)> {
        let mut hidden = pack(6, "55C9-36FE-613D-7F12", "Consistency", &["shooting"]);
        hidden.hidden = true;
        let mut aerial_row = aerial();
        aerial_row.shot_count = Some(10);
        aerial_row.notes = "Focus on the first touch.".into();
        let packs = vec![
            pack_stats(aerial_row.clone(), 276, 166, Some(D0)),
            pack_stats(
                pack(
                    4,
                    "07E1-81BC-DD2E-BF8C",
                    "Backboard Reads",
                    &["defence", "backboard"],
                ),
                40,
                28,
                Some(D0 - 5 * 86_400_000),
            ),
            pack_stats(
                pack(
                    5,
                    "5BFE-60D6-0D59-79F2",
                    "Aerial Off Wall",
                    &["aerial", "wall"],
                ),
                0,
                0,
                None,
            ),
            pack_stats(hidden, 58, 0, Some(D0 - 9 * 86_400_000)),
        ];
        let history = |session_id: i64, days: i64, attempts: i64, makes: i64| PackSessionRow {
            session_id,
            started_at: D0 - days * 86_400_000,
            attempts,
            makes,
            make_rate: makes as f64 / attempts as f64,
        };
        let pack_detail = PackDetail {
            stats: packs[0].clone(),
            last_session_rate: Some(41.0 / 62.0),
            best_session_rate: Some(0.7),
            history: vec![
                history(12, 0, 62, 41),
                history(9, 3, 48, 29),
                history(7, 5, 71, 39),
                history(5, 7, 40, 28),
                history(3, 9, 55, 28),
            ],
        };
        let settings = Settings {
            phone_enabled: true,
            install_dir: None,
            ..Settings::default()
        };
        let backups = vec![
            BackupInfo {
                name: "pre-edit-20261008-215800-120-merge.db".into(),
                kind: BackupKind::PreEdit,
                size_bytes: 2_400_000,
                modified_ms: D0 + 104 * 60_000,
            },
            BackupInfo {
                name: "rl-trainer-2026-10-08.db".into(),
                kind: BackupKind::Daily,
                size_bytes: 2_300_000,
                modified_ms: D0 - 12 * 3_600_000,
            },
            BackupInfo {
                name: "rl-trainer-2026-10-07.db".into(),
                kind: BackupKind::Daily,
                size_bytes: 2_300_000,
                modified_ms: D0 - 36 * 3_600_000,
            },
            BackupInfo {
                name: "rl-trainer-2026-10-06.db".into(),
                kind: BackupKind::Daily,
                size_bytes: 2_200_000,
                modified_ms: D0 - 60 * 3_600_000,
            },
        ];
        let pairing = PairingInfo {
            url: "http://192.168.1.66:49321/#t=abcdef0123456789".into(),
            port: 49321,
            addresses: vec!["http://192.168.1.66:49321/".into()],
            qr_svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect width=\"10\" height=\"10\" fill=\"#fff\"/></svg>".into(),
        };
        let profiles = vec![ProfileRow {
            primary_id: "Epic|me|0".into(),
            display_name: "Wapeto".into(),
            is_active: true,
        }];
        let about = About {
            version: "0.1.0".into(),
            data_dir: "C:\\Users\\me\\AppData\\Local\\rl-trainer".into(),
        };
        let v = |x: serde_json::Result<serde_json::Value>| x.unwrap();
        vec![
            ("cards", v(serde_json::to_value(cards()))),
            ("detail", v(serde_json::to_value(detail(false)))),
            ("open-detail", v(serde_json::to_value(detail(true)))),
            ("pack-list", v(serde_json::to_value(&packs))),
            ("pack-detail", v(serde_json::to_value(pack_detail))),
            ("settings", v(serde_json::to_value(settings))),
            ("backups", v(serde_json::to_value(backups))),
            ("pairing", v(serde_json::to_value(pairing))),
            ("profiles", v(serde_json::to_value(profiles))),
            ("about", v(serde_json::to_value(about))),
            ("review", v(serde_json::to_value(coach_samples::review()))),
            ("to-save", v(serde_json::to_value(coach_samples::to_save()))),
            (
                "replay-status",
                v(serde_json::to_value(coach_samples::replay_status())),
            ),
            (
                "match-replay",
                v(serde_json::to_value(coach_samples::match_replay())),
            ),
        ]
    }

    #[test]
    fn desktop_fixtures_are_current() {
        let samples = desktop_samples()
            .into_iter()
            .chain(crate::progress_samples::progress_samples());
        for (name, value) in samples {
            check(&format!("fixtures/desktop/{name}.json"), &json(&value));
        }
    }
}
