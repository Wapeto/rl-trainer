//! My own play in a match beyond the scoreboard (Plan 5, spec §4.2): touches by third, aerials,
//! hit power, car states, estimated boost use, demos taken and possession. Fed by
//! `MatchTracker` with play-time samples and events; pure, no I/O.

use crate::events::{HitBall, PlayerState};
use crate::facts::MatchMetrics;
use crate::interp::sampling::{pct, CarTime};

/// Ball height (unreal units) above which a touch counts as aerial: about one car jump.
pub const AERIAL_MIN_Z: f64 = 300.0;
/// The field is 10240 units long; thirds split at ±1/6 of it. Blue defends negative Y.
pub const THIRD_Y: f64 = 10_240.0 / 6.0;
/// A boost rise this big between two samples is a big pad (small pads give 12).
pub const BIG_PAD_RISE: f64 = 30.0;
pub const FULL_BOOST: f64 = 100.0;
/// The tracker's default; the setting overrides it (`MatchTracker::with_hard_hit_kmh`).
pub const DEFAULT_HARD_HIT_KMH: f64 = 80.0;
/// The feed sends several `BallHit`s for one contact (a dribble, a pinch); a hit this soon
/// after the last counted touch is the same touch. 40 ms matches the scoreboard's touch count
/// in the 7 Oct recording (70 vs 69 and 31 vs 31).
pub const TOUCH_GAP_MS: i64 = 40;

#[derive(Debug, Default)]
pub struct MatchMetricsAcc {
    hard_hit_kmh: f64,
    car: CarTime,
    speed_ms_sum: f64,
    boosting_ms: i64,
    full_boost_ms: i64,
    possession_ms: i64,
    /// Boost in the previous sample of an unbroken sequence.
    prev_boost: Option<f64>,
    boost_used: f64,
    big_pads: i32,
    touches: i32,
    last_touch_ms: Option<i64>,
    /// Speed counted for the last touch (raised when a merged hit was harder).
    last_touch_speed: f64,
    hit_speed_sum: f64,
    hard_hits: i32,
    aerial_touches: i32,
    /// Defensive, middle, attacking.
    thirds: [i32; 3],
    goals: i32,
    goal_speed_sum: f64,
    demos_taken: i32,
}

impl MatchMetricsAcc {
    pub fn new(hard_hit_kmh: f64) -> Self {
        Self {
            hard_hit_kmh,
            ..Self::default()
        }
    }

    /// One sample of my car worth `dt_ms` of play time; `our_touch` when my team touched the
    /// ball last.
    pub fn sample(&mut self, dt_ms: i64, me: &PlayerState, our_touch: bool) {
        self.car.add(dt_ms, me);
        self.speed_ms_sum += me.speed * dt_ms as f64;
        if me.boosting {
            self.boosting_ms += dt_ms;
        }
        if me.boost >= FULL_BOOST {
            self.full_boost_ms += dt_ms;
        }
        if our_touch {
            self.possession_ms += dt_ms;
        }
        self.track_boost(me);
    }

    fn track_boost(&mut self, me: &PlayerState) {
        if me.demolished {
            // Respawning refills boost to 33: not a pad.
            self.prev_boost = None;
            return;
        }
        if let Some(prev) = self.prev_boost {
            let change = me.boost - prev;
            if change < 0.0 && me.boosting {
                self.boost_used -= change;
            }
            if change >= BIG_PAD_RISE {
                self.big_pads += 1;
            }
        }
        self.prev_boost = Some(me.boost);
    }

    /// Goal replays, pauses and kickoffs reset boost: the next reading starts a new sequence.
    pub fn pause(&mut self) {
        self.prev_boost = None;
    }

    pub fn touch(&mut self, at_ms: i64, my_team: i32, ball: &HitBall) {
        let speed = ball.post_hit_speed;
        if self
            .last_touch_ms
            .is_some_and(|last| at_ms - last <= TOUCH_GAP_MS)
        {
            self.raise_last_touch(speed);
            return;
        }
        self.last_touch_ms = Some(at_ms);
        self.last_touch_speed = speed;
        self.touches += 1;
        self.hit_speed_sum += speed;
        if speed >= self.hard_hit_kmh {
            self.hard_hits += 1;
        }
        if let Some(at) = ball.location {
            if at.z > AERIAL_MIN_Z {
                self.aerial_touches += 1;
            }
            // Toward the opponents' goal is positive Y for blue (team 0).
            let forward = if my_team == 1 { -at.y } else { at.y };
            let third = if forward < -THIRD_Y {
                0
            } else if forward > THIRD_Y {
                2
            } else {
                1
            };
            self.thirds[third] += 1;
        }
    }

    /// The same contact reported again: keep its hardest speed.
    fn raise_last_touch(&mut self, speed: f64) {
        if speed <= self.last_touch_speed {
            return;
        }
        if self.last_touch_speed < self.hard_hit_kmh && speed >= self.hard_hit_kmh {
            self.hard_hits += 1;
        }
        self.hit_speed_sum += speed - self.last_touch_speed;
        self.last_touch_speed = speed;
    }

    pub fn goal(&mut self, goal_speed: f64) {
        self.goals += 1;
        self.goal_speed_sum += goal_speed;
    }

    pub fn demolished(&mut self) {
        self.demos_taken += 1;
    }

    /// `None` when no play time was sampled.
    pub fn finish(&self) -> Option<MatchMetrics> {
        let total = self.car.total_ms;
        if total <= 0 {
            return None;
        }
        Some(MatchMetrics {
            play_seconds: total as f64 / 1000.0,
            hit_avg_speed: (self.touches > 0).then(|| self.hit_speed_sum / f64::from(self.touches)),
            hard_hits: self.hard_hits,
            aerial_touches: self.aerial_touches,
            touches_def: self.thirds[0],
            touches_mid: self.thirds[1],
            touches_att: self.thirds[2],
            pct_on_ground: self.car.pct_ground(),
            pct_on_wall: self.car.pct_wall(),
            pct_airborne: self.car.pct_air(),
            avg_speed: self.speed_ms_sum / total as f64,
            pct_boosting: pct(self.boosting_ms, total),
            pct_full_boost: pct(self.full_boost_ms, total),
            boost_used: self.boost_used,
            big_pads: self.big_pads,
            goal_speed_avg: (self.goals > 0).then(|| self.goal_speed_sum / f64::from(self.goals)),
            demos_taken: self.demos_taken,
            pct_possession: pct(self.possession_ms, total),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Vec3;

    fn car(boost: f64, boosting: bool) -> PlayerState {
        PlayerState {
            boost,
            boosting,
            on_ground: true,
            speed: 50.0,
            ..Default::default()
        }
    }

    fn hit(speed: f64, y: f64, z: f64) -> HitBall {
        HitBall {
            pre_hit_speed: 0.0,
            post_hit_speed: speed,
            location: Some(Vec3 { x: 0.0, y, z }),
        }
    }

    #[test]
    fn nothing_sampled_gives_no_metrics() {
        assert!(MatchMetricsAcc::new(80.0).finish().is_none());
    }

    #[test]
    fn time_shares_are_weighted_by_play_time() {
        let mut a = MatchMetricsAcc::new(80.0);
        let fast = PlayerState {
            speed: 100.0,
            boost: 100.0,
            boosting: true,
            on_ground: true,
            ..Default::default()
        };
        let slow = PlayerState {
            speed: 20.0,
            boost: 40.0,
            ..Default::default()
        };
        a.sample(1_000, &fast, true);
        a.sample(3_000, &slow, false);
        let m = a.finish().unwrap();
        assert_eq!(m.play_seconds, 4.0);
        assert_eq!((m.pct_on_ground, m.pct_airborne), (25.0, 75.0));
        assert_eq!(
            (m.pct_boosting, m.pct_full_boost, m.pct_possession),
            (25.0, 25.0, 25.0)
        );
        assert_eq!(m.avg_speed, 40.0);
    }

    #[test]
    fn boost_used_counts_drops_while_boosting_only() {
        let mut a = MatchMetricsAcc::new(80.0);
        for (boost, boosting) in [(100.0, false), (80.0, true), (60.0, true), (50.0, false)] {
            a.sample(100, &car(boost, boosting), false);
        }
        assert_eq!(a.finish().unwrap().boost_used, 40.0);
    }

    #[test]
    fn big_pads_are_rises_of_30_or_more_but_not_respawns_or_kickoffs() {
        let mut a = MatchMetricsAcc::new(80.0);
        for boost in [10.0, 22.0, 34.0, 100.0] {
            a.sample(100, &car(boost, false), false); // two small pads, then a big one
        }
        a.pause(); // goal replay; the kickoff sets boost to 33
        a.sample(100, &car(33.0, false), false);
        let wrecked = PlayerState {
            demolished: true,
            ..car(0.0, false)
        };
        a.sample(100, &wrecked, false);
        for boost in [33.0, 100.0] {
            a.sample(100, &car(boost, false), false); // respawn at 33, then a big pad
        }
        assert_eq!(a.finish().unwrap().big_pads, 2);
    }

    #[test]
    fn touches_are_split_by_third_from_my_side_and_aerials_counted() {
        let mut blue = MatchMetricsAcc::new(80.0);
        blue.sample(100, &car(50.0, false), false);
        blue.touch(0, 0, &hit(90.0, -3000.0, 50.0)); // own third, hard
        blue.touch(1_000, 0, &hit(40.0, 0.0, 400.0)); // middle, aerial
        blue.touch(2_000, 0, &hit(60.0, 3000.0, 50.0)); // attacking third
        let m = blue.finish().unwrap();
        assert_eq!((m.touches_def, m.touches_mid, m.touches_att), (1, 1, 1));
        assert_eq!((m.hard_hits, m.aerial_touches), (1, 1));
        assert!((m.hit_avg_speed.unwrap() - 190.0 / 3.0).abs() < 1e-9);

        let mut orange = MatchMetricsAcc::new(80.0);
        orange.sample(100, &car(50.0, false), false);
        orange.touch(0, 1, &hit(40.0, -3000.0, 50.0));
        let m = orange.finish().unwrap();
        assert_eq!(
            (m.touches_def, m.touches_att),
            (0, 1),
            "orange attacks toward negative Y"
        );
    }

    #[test]
    fn contacts_within_the_touch_gap_of_the_last_counted_touch_are_one_touch() {
        let mut a = MatchMetricsAcc::new(80.0);
        a.sample(100, &car(50.0, false), false);
        // One dribble seen as five BallHit events; the game's scoreboard counts 3.
        for at in [0, 17, 34, 50, 95] {
            a.touch(at, 0, &hit(50.0, 0.0, 100.0));
        }
        let m = a.finish().unwrap();
        assert_eq!(
            m.touches_mid, 3,
            "0, 50 and 95 count; 17 and 34 are the same contact"
        );
    }

    #[test]
    fn a_touch_without_a_location_counts_but_has_no_third() {
        let mut a = MatchMetricsAcc::new(80.0);
        a.sample(100, &car(50.0, false), false);
        a.touch(
            0,
            0,
            &HitBall {
                post_hit_speed: 30.0,
                ..Default::default()
            },
        );
        let m = a.finish().unwrap();
        assert_eq!(m.hit_avg_speed, Some(30.0));
        assert_eq!((m.touches_def, m.touches_mid, m.touches_att), (0, 0, 0));
    }

    #[test]
    fn goal_speed_averages_my_goals_and_demos_taken_add_up() {
        let mut a = MatchMetricsAcc::new(80.0);
        a.sample(100, &car(50.0, false), false);
        assert_eq!(a.finish().unwrap().goal_speed_avg, None);
        a.goal(90.0);
        a.goal(110.0);
        a.demolished();
        a.demolished();
        let m = a.finish().unwrap();
        assert_eq!((m.goal_speed_avg, m.demos_taken), (Some(100.0), 2));
    }

    #[test]
    fn hits_merged_into_one_touch_keep_the_hardest_speed() {
        let mut a = MatchMetricsAcc::new(80.0);
        a.sample(1_000, &car(50.0, false), false);
        a.touch(1_000, 0, &hit(30.0, 0.0, 100.0));
        a.touch(1_020, 0, &hit(90.0, 0.0, 100.0));
        let m = a.finish().unwrap();
        assert_eq!(m.hard_hits, 1);
        assert_eq!(m.hit_avg_speed, Some(90.0));
    }
}
