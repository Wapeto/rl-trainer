//! Request guards: Host check, per-address rate limit, bearer token with per-address lockout.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;

use crate::phone::api::ApiError;
use crate::phone::PhoneState;

pub const MAX_FAILURES: u32 = 10;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(60);
pub const LOCKOUT: Duration = Duration::from_secs(300);
pub const PER_ADDRESS_PER_SECOND: f64 = 20.0;
/// Entries kept before idle ones are dropped.
const PRUNE_ABOVE: usize = 256;
/// A rate bucket unused this long is full again, so it can be forgotten.
const IDLE: Duration = Duration::from_secs(60);

pub fn token_matches(expected: &str, presented: &str) -> bool {
    expected.len() == presented.len() && bool::from(expected.as_bytes().ct_eq(presented.as_bytes()))
}

/// Accepts only IP literals and `localhost`, so a web page using DNS rebinding cannot reach the
/// API from a browser on this PC.
pub fn host_allowed(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(""),
        None => host.split(':').next().unwrap_or(""),
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<IpAddr>().is_ok()
}

struct Failures {
    window_start: Instant,
    count: u32,
    locked_until: Option<Instant>,
}

#[derive(Default)]
pub struct AuthLimiter {
    failures: Mutex<HashMap<IpAddr, Failures>>,
}

impl AuthLimiter {
    pub fn is_locked(&self, ip: IpAddr, now: Instant) -> bool {
        self.failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&ip)
            .and_then(|f| f.locked_until)
            .is_some_and(|until| now < until)
    }

    pub fn record_failure(&self, ip: IpAddr, now: Instant) {
        let mut map = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        // Forget addresses that are neither locked nor inside their failure window.
        map.retain(|k, f| {
            *k == ip
                || f.locked_until.is_some_and(|until| now < until)
                || now.saturating_duration_since(f.window_start) <= FAILURE_WINDOW
        });
        let entry = map.entry(ip).or_insert(Failures {
            window_start: now,
            count: 0,
            locked_until: None,
        });
        if now.duration_since(entry.window_start) > FAILURE_WINDOW {
            entry.window_start = now;
            entry.count = 0;
        }
        entry.count += 1;
        if entry.count >= MAX_FAILURES {
            entry.locked_until = Some(now + LOCKOUT);
            entry.window_start = now;
            entry.count = 0;
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// A token bucket per client address: one busy phone does not slow another down.
pub struct RateLimiter {
    per_second: f64,
    burst: f64,
    buckets: Mutex<HashMap<IpAddr, (f64, Instant)>>,
}

impl RateLimiter {
    pub fn new(per_second: f64, burst: f64) -> Self {
        Self {
            per_second,
            burst,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn allow(&self, ip: IpAddr, now: Instant) -> bool {
        let mut map = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= PRUNE_ABOVE || map.keys().any(|k| *k != ip) {
            map.retain(|k, (_, last)| *k == ip || now.saturating_duration_since(*last) < IDLE);
        }
        let (tokens, last) = *map.get(&ip).unwrap_or(&(self.burst, now));
        let elapsed = now.saturating_duration_since(last).as_secs_f64();
        let tokens = (tokens + elapsed * self.per_second).min(self.burst);
        let last = if now > last { now } else { last };
        let allowed = tokens >= 1.0;
        map.insert(ip, (if allowed { tokens - 1.0 } else { tokens }, last));
        allowed
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.buckets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

pub async fn guard(
    State(s): State<PhoneState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok());
    if !host.is_some_and(host_allowed) {
        return ApiError::forbidden("address this PC by its IP address").into_response();
    }
    if !s.rate.allow(peer.ip(), Instant::now()) {
        return ApiError::rate_limited().into_response();
    }
    next.run(req).await
}

pub async fn require_token(
    State(s): State<PhoneState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let now = Instant::now();
    let ip = peer.ip();
    if s.auth.is_locked(ip, now) {
        return ApiError::rate_limited().into_response();
    }
    let presented = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let valid = presented.is_some_and(|t| token_matches(&s.token.borrow(), t));
    if !valid {
        s.auth.record_failure(ip, now);
        return ApiError::unauthorized().into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn hosts_must_be_ip_literals_or_localhost() {
        for ok in [
            "192.168.1.20:49321",
            "127.0.0.1",
            "localhost:49321",
            "LOCALHOST",
            "[::1]:49321",
        ] {
            assert!(host_allowed(ok), "{ok}");
        }
        for bad in [
            "evil.example",
            "evil.example:49321",
            "192.168.1.20.nip.io",
            "",
        ] {
            assert!(!host_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn tokens_compare_exactly() {
        assert!(token_matches("abc", "abc"));
        assert!(!token_matches("abc", "abd"));
        assert!(!token_matches("abc", "ab"));
    }

    #[test]
    fn ten_failures_in_a_minute_lock_the_address_for_five_minutes() {
        let limiter = AuthLimiter::default();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        let t0 = Instant::now();
        for i in 0..9 {
            limiter.record_failure(ip, t0 + Duration::from_secs(i));
        }
        assert!(!limiter.is_locked(ip, t0 + Duration::from_secs(9)));
        limiter.record_failure(ip, t0 + Duration::from_secs(9));
        assert!(limiter.is_locked(ip, t0 + Duration::from_secs(10)));
        assert!(!limiter.is_locked(ip, t0 + Duration::from_secs(10) + LOCKOUT));
        let other = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8));
        assert!(!limiter.is_locked(other, t0 + Duration::from_secs(10)));
    }

    #[test]
    fn failures_spread_over_more_than_a_minute_do_not_lock() {
        let limiter = AuthLimiter::default();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        let t0 = Instant::now();
        for i in 0..20 {
            limiter.record_failure(ip, t0 + Duration::from_secs(i * 7));
        }
        assert!(!limiter.is_locked(ip, t0 + Duration::from_secs(140)));
    }

    #[test]
    fn rate_limiter_allows_a_burst_then_refills() {
        let limiter = RateLimiter::new(20.0, 20.0);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let t0 = Instant::now();
        assert!((0..20).all(|_| limiter.allow(ip, t0)));
        assert!(!limiter.allow(ip, t0));
        assert!(
            limiter.allow(ip, t0 + Duration::from_millis(100)),
            "two tokens refilled"
        );
    }

    #[test]
    fn each_address_has_its_own_bucket() {
        let limiter = RateLimiter::new(20.0, 20.0);
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let t0 = Instant::now();
        assert!((0..20).all(|_| limiter.allow(a, t0)));
        assert!(!limiter.allow(a, t0));
        assert!(limiter.allow(b, t0), "another phone is not slowed down");
    }

    #[test]
    fn idle_addresses_are_forgotten() {
        let limiter = RateLimiter::new(20.0, 20.0);
        let auth = AuthLimiter::default();
        let t0 = Instant::now();
        for i in 0..300u32 {
            let ip = IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + i));
            limiter.allow(ip, t0);
            auth.record_failure(ip, t0);
        }
        let later = t0 + LOCKOUT + FAILURE_WINDOW + Duration::from_secs(1);
        limiter.allow(IpAddr::V4(Ipv4Addr::new(10, 9, 9, 9)), later);
        auth.record_failure(IpAddr::V4(Ipv4Addr::new(10, 9, 9, 9)), later);
        assert_eq!(limiter.len(), 1);
        assert_eq!(auth.len(), 1);
    }
}
