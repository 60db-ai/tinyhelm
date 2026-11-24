//! Port of `App\Http\Middleware\ThrottleFailedApiRequests` (global middleware on every `api/*` request).
//!
//! Counts 401/403 answers (and 404 answers to requests without a bearer token) per token (sha256 of the
//! raw bearer value) or per client IP. When `API_FAILED_REQUEST_RATE_LIMIT` (default 30, `<= 0` disables)
//! failures happen within 60 seconds, the key is blocked for 60s, then 5min, then 15min on repeated
//! strikes (strikes are remembered for an hour). Blocked keys get Laravel's 429.
//!
//! Responses that Laravel produced (`crate::Proxied`) are not counted here: Laravel counted them.
//! ponytail: in-process state; Laravel's counter lives in its cache, so each side enforces the limit for
//! the requests it answers. Move to the shared Redis cache when the cache layer is ported.
//! Not ported: `ApiTokenFailingNotification` to the token's team (the notification system is unported).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const DECAY: Duration = Duration::from_secs(60);
const BLOCK_SECONDS: [u64; 3] = [60, 300, 900];
const STRIKE_MEMORY: Duration = Duration::from_secs(3600);

#[derive(Default)]
struct Entry {
    window_start: Option<Instant>,
    hits: u32,
    blocked_until: Option<Instant>,
    strikes: u32,
    strikes_until: Option<Instant>,
}

static STATE: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

fn limit() -> i64 {
    static LIMIT: OnceLock<i64> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        std::env::var("API_FAILED_REQUEST_RATE_LIMIT")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(30)
    })
}

/// `api-failed-requests:token:<sha256>` or `api-failed-requests:ip:<ip>`; None when disabled.
pub fn key(headers: &HeaderMap, ip: Option<&str>) -> Option<String> {
    if limit() <= 0 {
        return None;
    }
    Some(match bearer_token(headers) {
        Some(token) => format!("token:{}", hex::encode(Sha256::digest(token.as_bytes()))),
        None => format!("ip:{}", ip.unwrap_or("")),
    })
}

/// Laravel `Request::bearerToken()`: the value after the first "Bearer " in the Authorization header.
pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let header = headers.get("authorization")?.to_str().ok()?;
    let pos = header.find("Bearer ")?;
    let token = header[pos + 7..].split(',').next().unwrap_or("").trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// The 429 to send when the key is blocked.
pub fn blocked_response(key: &str) -> Option<Response> {
    blocked_response_at(key, Instant::now())
}

fn blocked_response_at(key: &str, now: Instant) -> Option<Response> {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let entry = guard.get_or_insert_with(HashMap::new).get(key)?;
    let until = entry.blocked_until.filter(|u| *u > now)?;
    let retry_after = until.duration_since(now).as_secs().max(1);
    let mut res = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "message": format!("Too many failed API requests. Retry after {retry_after} seconds.")
        })),
    )
        .into_response();
    if let Ok(v) = HeaderValue::from_str(&retry_after.to_string()) {
        res.headers_mut().insert("retry-after", v);
    }
    Some(res)
}

/// Record the outcome of a request answered by Rust.
pub fn record(key: &str, status: StatusCode, had_bearer: bool) {
    record_at(key, status, had_bearer, Instant::now(), limit());
}

fn record_at(key: &str, status: StatusCode, had_bearer: bool, now: Instant, max: i64) {
    let counted =
        matches!(status.as_u16(), 401 | 403) || (status == StatusCode::NOT_FOUND && !had_bearer);
    if !counted || max <= 0 {
        return;
    }
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if map.len() > 50_000 {
        map.retain(|_, e| {
            e.strikes_until.is_some_and(|u| u > now)
                || e.window_start
                    .is_some_and(|s| now.duration_since(s) < DECAY)
        });
    }
    let e = map.entry(key.to_string()).or_default();
    // RateLimiter::hit($key, 60): the window starts at the first hit and expires after 60 seconds.
    if e.window_start
        .map_or(true, |s| now.duration_since(s) >= DECAY)
    {
        e.window_start = Some(now);
        e.hits = 0;
    }
    e.hits += 1;
    if i64::from(e.hits) >= max {
        if e.strikes_until.map_or(true, |u| u <= now) {
            e.strikes = 0;
        }
        e.strikes += 1;
        e.strikes_until = Some(now + STRIKE_MEMORY);
        let seconds = BLOCK_SECONDS[(e.strikes as usize).min(BLOCK_SECONDS.len()) - 1];
        e.blocked_until = Some(now + Duration::from_secs(seconds));
        e.window_start = None;
        e.hits = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_limit_with_escalating_strikes() {
        let k = "token:test-escalation";
        let t0 = Instant::now();
        for _ in 0..3 {
            record_at(k, StatusCode::FORBIDDEN, true, t0, 3);
        }
        let blocked = blocked_response_at(k, t0).expect("blocked after 3 failures");
        assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(blocked.headers()["retry-after"], "60");
        assert!(blocked_response_at(k, t0 + Duration::from_secs(61)).is_none());

        let t1 = t0 + Duration::from_secs(61);
        for _ in 0..3 {
            record_at(k, StatusCode::UNAUTHORIZED, true, t1, 3);
        }
        assert_eq!(
            blocked_response_at(k, t1).unwrap().headers()["retry-after"],
            "300",
            "second strike"
        );
    }

    #[test]
    fn only_auth_failures_and_anonymous_404_count() {
        let k = "ip:test-counting";
        let t0 = Instant::now();
        for _ in 0..5 {
            record_at(k, StatusCode::NOT_FOUND, true, t0, 2); // a token holder's 404 does not count
            record_at(k, StatusCode::UNPROCESSABLE_ENTITY, false, t0, 2);
        }
        assert!(blocked_response_at(k, t0).is_none());
        record_at(k, StatusCode::NOT_FOUND, false, t0, 2);
        record_at(k, StatusCode::NOT_FOUND, false, t0, 2);
        assert!(blocked_response_at(k, t0).is_some());
    }

    #[test]
    fn bearer_parsing_matches_laravel() {
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer 2|abc".parse().unwrap());
        assert_eq!(bearer_token(&h).as_deref(), Some("2|abc"));
        h.insert("authorization", "Basic x".parse().unwrap());
        assert_eq!(bearer_token(&h), None);
    }
}
