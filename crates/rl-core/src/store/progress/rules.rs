//! The change and averaging rules of spec §3, shared by every progress read.

/// Sessions on each side of a change.
pub const RECENT_SESSIONS: usize = 3;
/// A change is shown only with at least this many sessions.
pub const MIN_SESSIONS_FOR_CHANGE: usize = 4;

/// One session's value of a stat and what it weighs (attempts, matches, play time…).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weighted {
    pub value: f64,
    pub weight: f64,
}

/// Weighted mean; `None` when nothing weighs anything.
pub fn mean(points: &[Weighted]) -> Option<f64> {
    let weight: f64 = points.iter().map(|p| p.weight).sum();
    (weight > 0.0).then(|| points.iter().map(|p| p.value * p.weight).sum::<f64>() / weight)
}

/// The last 3 sessions minus the first 3 (input oldest first); `None` under 4 sessions.
pub fn rate_change(points: &[Weighted]) -> Option<f64> {
    if points.len() < MIN_SESSIONS_FOR_CHANGE {
        return None;
    }
    let n = RECENT_SESSIONS;
    Some(mean(&points[points.len() - n..])? - mean(&points[..n])?)
}

/// "Packs that moved": (from, to). With 2–5 sessions the first and the last session; with 6 or
/// more, the first 3 and the last 3.
pub fn mover_span(points: &[Weighted]) -> Option<(f64, f64)> {
    let n = points.len();
    match n {
        0 | 1 => None,
        _ if n < 2 * RECENT_SESSIONS => Some((points[0].value, points[n - 1].value)),
        _ => Some((
            mean(&points[..RECENT_SESSIONS])?,
            mean(&points[n - RECENT_SESSIONS..])?,
        )),
    }
}

/// Latest MMR in the range minus the last one before it, or minus the first one in it.
pub fn mmr_change(before: Option<i64>, in_range: &[i64]) -> Option<i64> {
    let latest = *in_range.last()?;
    match (before, in_range) {
        (Some(b), _) => Some(latest - b),
        (None, [first, _, ..]) => Some(latest - first),
        (None, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(value: f64, weight: f64) -> Weighted {
        Weighted { value, weight }
    }

    #[test]
    fn the_mean_is_weighted_and_empty_weights_give_none() {
        assert_eq!(mean(&[w(0.5, 10.0), w(0.8, 30.0)]), Some(0.725));
        assert_eq!(mean(&[w(0.5, 0.0)]), None);
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn a_change_needs_four_sessions_and_compares_last_three_with_first_three() {
        let three = [w(0.4, 10.0), w(0.5, 10.0), w(0.6, 10.0)];
        assert_eq!(rate_change(&three), None);
        let four = [w(0.4, 10.0), w(0.5, 10.0), w(0.6, 10.0), w(0.9, 10.0)];
        let change = rate_change(&four).unwrap();
        assert!((change - (2.0 / 3.0 - 0.5)).abs() < 1e-9);
    }

    #[test]
    fn movers_use_first_and_last_session_until_six_sessions() {
        assert_eq!(mover_span(&[w(0.5, 10.0)]), None);
        assert_eq!(
            mover_span(&[w(0.5, 10.0), w(0.9, 5.0), w(0.7, 10.0)]),
            Some((0.5, 0.7))
        );
        let six: Vec<Weighted> = [0.1, 0.2, 0.3, 0.7, 0.8, 0.9]
            .iter()
            .map(|v| w(*v, 10.0))
            .collect();
        let (from, to) = mover_span(&six).unwrap();
        assert!((from - 0.2).abs() < 1e-9 && (to - 0.8).abs() < 1e-9);
    }

    #[test]
    fn mmr_change_uses_the_entry_before_the_range_or_the_first_in_it() {
        assert_eq!(mmr_change(Some(880), &[900, 915]), Some(35));
        assert_eq!(mmr_change(None, &[900, 915]), Some(15));
        assert_eq!(mmr_change(None, &[900]), None);
        assert_eq!(mmr_change(Some(880), &[]), None);
    }
}
