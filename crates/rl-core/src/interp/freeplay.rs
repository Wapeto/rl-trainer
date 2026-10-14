//! Free-play block aggregates: touches, hit power and car state percentages.

use crate::events::PlayerState;
use crate::facts::FreeplayStats;
use crate::interp::sampling::{CarTime, SampleClock};

/// Below this car speed (km/h, the unit of `Players[].Speed`) the player is not moving.
pub const MOVING_KMH: f64 = 5.0;

#[derive(Debug, Default)]
pub struct FreeplayTracker {
    hard_hit_kmh: f64,
    clock: SampleClock,
    paused: bool,
    touches: u32,
    hard_hits: u32,
    hit_speed_sum: f64,
    max_hit_speed: f64,
    goals: u32,
    crossbar_hits: u32,
    car: CarTime,
}

impl FreeplayTracker {
    pub fn new(hard_hit_kmh: f64) -> Self {
        Self {
            hard_hit_kmh,
            ..Self::default()
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            self.clock.pause();
        }
    }

    /// Credits the time since the previous sample to the car state in this sample. A still car
    /// is not play time (the player may be away from the keyboard).
    pub fn sample(&mut self, at_ms: i64, me: &PlayerState, replay: bool) {
        if replay || self.paused || me.speed < MOVING_KMH {
            self.clock.pause();
            return;
        }
        let dt = self.clock.tick(at_ms);
        self.car.add(dt, me);
    }

    pub fn ball_hit(&mut self, post_hit_speed: f64) {
        self.touches += 1;
        self.hit_speed_sum += post_hit_speed;
        self.max_hit_speed = self.max_hit_speed.max(post_hit_speed);
        if post_hit_speed >= self.hard_hit_kmh {
            self.hard_hits += 1;
        }
    }

    pub fn goal(&mut self) {
        self.goals += 1;
    }

    pub fn crossbar(&mut self) {
        self.crossbar_hits += 1;
    }

    pub fn live(&self) -> crate::live::LiveFreeplay {
        crate::live::LiveFreeplay {
            touches: self.touches,
            hard_hits: self.hard_hits,
            max_hit_speed: self.max_hit_speed,
            goals: self.goals,
            pct_supersonic: self.car.pct_supersonic(),
            pct_on_wall: self.car.pct_wall(),
            pct_airborne: self.car.pct_air(),
            best_today_hit: self.max_hit_speed,
        }
    }

    pub fn finish(&self) -> FreeplayStats {
        let avg_hit_speed = if self.touches == 0 {
            0.0
        } else {
            self.hit_speed_sum / f64::from(self.touches)
        };
        FreeplayStats {
            touches: self.touches,
            hard_hits: self.hard_hits,
            avg_hit_speed,
            max_hit_speed: self.max_hit_speed,
            goals: self.goals,
            crossbar_hits: self.crossbar_hits,
            pct_supersonic: self.car.pct_supersonic(),
            pct_on_wall: self.car.pct_wall(),
            pct_airborne: self.car.pct_air(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me(on_ground: bool, on_wall: bool, supersonic: bool) -> PlayerState {
        PlayerState {
            on_ground,
            on_wall,
            supersonic,
            speed: 40.0,
            ..Default::default()
        }
    }

    #[test]
    fn counts_hits_against_the_threshold() {
        let mut f = FreeplayTracker::new(80.0);
        for speed in [50.0, 85.0, 112.0] {
            f.ball_hit(speed);
        }
        f.goal();
        f.crossbar();
        let s = f.finish();
        assert_eq!(
            (s.touches, s.hard_hits, s.goals, s.crossbar_hits),
            (3, 2, 1, 1)
        );
        assert_eq!(s.max_hit_speed, 112.0);
        assert!((s.avg_hit_speed - 82.333).abs() < 0.01);
    }

    #[test]
    fn weights_car_state_by_time() {
        let mut f = FreeplayTracker::new(80.0);
        f.sample(0, &me(true, false, false), false);
        f.sample(100, &me(false, false, true), false); // air + supersonic
        f.sample(200, &me(false, false, false), false); // air
        f.sample(300, &me(true, true, false), false); // wall
        let s = f.finish();
        assert!((s.pct_airborne - 66.666).abs() < 0.01);
        assert!((s.pct_on_wall - 33.333).abs() < 0.01);
        assert!((s.pct_supersonic - 33.333).abs() < 0.01);
    }

    #[test]
    fn replay_samples_are_ignored() {
        let mut f = FreeplayTracker::new(80.0);
        f.sample(0, &me(true, false, false), false);
        f.sample(100, &me(false, false, false), true);
        f.sample(5_000, &me(false, false, false), false);
        assert_eq!(f.finish().pct_airborne, 0.0);
    }

    #[test]
    fn empty_block_has_zeroed_stats() {
        assert_eq!(
            FreeplayTracker::new(80.0).finish(),
            FreeplayStats::default()
        );
    }

    #[test]
    fn paused_time_is_not_weighted() {
        let mut f = FreeplayTracker::new(80.0);
        f.sample(0, &me(false, false, false), false);
        f.set_paused(true);
        f.sample(1_000, &me(false, false, false), false);
        f.set_paused(false);
        f.sample(1_100, &me(false, false, false), false);
        assert_eq!(f.finish().pct_airborne, 0.0);
    }

    #[test]
    fn still_samples_are_not_play_time() {
        let mut f = FreeplayTracker::new(80.0);
        let still = PlayerState {
            speed: 0.0,
            ..me(true, false, false)
        };
        f.sample(0, &me(true, false, false), false);
        f.sample(1_000, &me(false, false, false), false); // 1 s in the air
        f.sample(2_000, &still, false);
        f.sample(3_000, &still, false);
        f.sample(4_000, &me(true, false, false), false); // first sample after stillness credits 0
        assert_eq!(f.finish().pct_airborne, 100.0);
    }

    #[test]
    fn live_view_includes_goals_and_car_state_shares() {
        let mut f = FreeplayTracker::new(80.0);
        f.sample(0, &me(true, false, false), false);
        f.sample(1_000, &me(false, true, true), false);
        f.ball_hit(90.0);
        f.goal();
        let live = f.live();
        assert_eq!((live.goals, live.touches, live.hard_hits), (1, 1, 1));
        assert_eq!(
            (live.pct_on_wall, live.pct_supersonic, live.pct_airborne),
            (100.0, 100.0, 0.0)
        );
        assert_eq!(live.best_today_hit, 90.0);
    }
}
