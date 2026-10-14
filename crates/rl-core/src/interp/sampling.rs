//! Time-weighting for `UpdateState` samples.

use crate::events::PlayerState;

/// Longest interval credited to one sample, so connection gaps don't inflate totals.
pub const MAX_SAMPLE_GAP_MS: i64 = 1_000;

#[derive(Debug, Default)]
pub struct SampleClock {
    prev_ms: Option<i64>,
}

impl SampleClock {
    /// Milliseconds since the previous sample (0 for the first one after a pause).
    pub fn tick(&mut self, at_ms: i64) -> i64 {
        let dt = self
            .prev_ms
            .map_or(0, |prev| (at_ms - prev).clamp(0, MAX_SAMPLE_GAP_MS));
        self.prev_ms = Some(at_ms);
        dt
    }

    pub fn pause(&mut self) {
        self.prev_ms = None;
    }
}

pub fn pct(part_ms: i64, total_ms: i64) -> f64 {
    if total_ms <= 0 {
        0.0
    } else {
        part_ms as f64 * 100.0 / total_ms as f64
    }
}

/// Time spent in each car state. A car on the wall is not airborne even if it is off the ground.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CarTime {
    pub total_ms: i64,
    pub ground_ms: i64,
    pub wall_ms: i64,
    pub air_ms: i64,
    pub supersonic_ms: i64,
}

impl CarTime {
    pub fn add(&mut self, dt_ms: i64, me: &PlayerState) {
        // No car while demolished: that time is neither air nor ground.
        if me.demolished {
            return;
        }
        self.total_ms += dt_ms;
        if me.supersonic {
            self.supersonic_ms += dt_ms;
        }
        if me.on_wall {
            self.wall_ms += dt_ms;
        } else if me.on_ground {
            self.ground_ms += dt_ms;
        } else {
            self.air_ms += dt_ms;
        }
    }

    pub fn pct_ground(&self) -> f64 {
        pct(self.ground_ms, self.total_ms)
    }

    pub fn pct_wall(&self) -> f64 {
        pct(self.wall_ms, self.total_ms)
    }

    pub fn pct_air(&self) -> f64 {
        pct(self.air_ms, self.total_ms)
    }

    pub fn pct_supersonic(&self) -> f64 {
        pct(self.supersonic_ms, self.total_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_returns_elapsed_time_capped_for_gaps() {
        let mut c = SampleClock::default();
        assert_eq!(c.tick(0), 0);
        assert_eq!(c.tick(100), 100);
        assert_eq!(c.tick(5_100), MAX_SAMPLE_GAP_MS);
        c.pause();
        assert_eq!(c.tick(9_000), 0);
    }

    #[test]
    fn pct_handles_zero_total() {
        assert_eq!(pct(50, 200), 25.0);
        assert_eq!(pct(5, 0), 0.0);
    }

    #[test]
    fn car_time_splits_ground_wall_air_and_supersonic() {
        let car = |on_ground, on_wall, supersonic| PlayerState {
            on_ground,
            on_wall,
            supersonic,
            ..Default::default()
        };
        let mut t = CarTime::default();
        t.add(100, &car(true, false, false));
        t.add(100, &car(false, true, true));
        t.add(200, &car(false, false, false));
        assert_eq!(
            (
                t.pct_ground(),
                t.pct_wall(),
                t.pct_air(),
                t.pct_supersonic()
            ),
            (25.0, 25.0, 50.0, 25.0)
        );
    }

    #[test]
    fn demolished_time_is_left_out_of_the_movement_shares() {
        let mut t = CarTime::default();
        let ground = PlayerState {
            on_ground: true,
            ..PlayerState::default()
        };
        let dead = PlayerState {
            demolished: true,
            ..PlayerState::default()
        };
        t.add(1_000, &ground);
        t.add(3_000, &dead);
        assert!(
            (t.pct_ground() - 100.0).abs() < 1e-9,
            "the 3 s without a car are not airborne"
        );
        assert_eq!(t.pct_air(), 0.0);
    }
}
