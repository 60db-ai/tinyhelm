//! `throttle:api`: `API_RATE_LIMIT` (default 200) requests per minute per user, with Laravel's
//! 429 body and `X-RateLimit-*` / `Retry-After` headers.
//!
//! ponytail: in-process fixed window. Laravel keeps its own counter for the routes it still serves,
//! so a client can get up to 2x the limit across both until the limiter moves to shared storage.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::{
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

const WINDOW: Duration = Duration::from_secs(60);

pub struct Limit {
    pub allowed: bool,
    pub max: u32,
    pub remaining: u32,
    pub retry_after: u64,
}

fn max_per_minute() -> u32 {
    static MAX: OnceLock<u32> = OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("API_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200)
    })
}

static HITS: Mutex<Option<HashMap<String, (Instant, u32)>>> = Mutex::new(None);

pub fn check(key: &str) -> Limit {
    check_with(key, max_per_minute(), Instant::now())
}

fn check_with(key: &str, max: u32, now: Instant) -> Limit {
    let mut guard = HITS.lock().unwrap_or_else(|e| e.into_inner());
    let hits = guard.get_or_insert_with(HashMap::new);
    if hits.len() > 10_000 {
        hits.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
    }
    let entry = hits.entry(key.to_string()).or_insert((now, 0));
    if now.duration_since(entry.0) >= WINDOW {
        *entry = (now, 0);
    }
    let retry_after = WINDOW
        .saturating_sub(now.duration_since(entry.0))
        .as_secs()
        .max(1);
    if entry.1 >= max {
        return Limit {
            allowed: false,
            max,
            remaining: 0,
            retry_after,
        };
    }
    entry.1 += 1;
    Limit {
        allowed: true,
        max,
        remaining: max - entry.1,
        retry_after,
    }
}

pub fn too_many(limit: &Limit) -> Response {
    let mut res = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({"message": "Too Many Attempts."})),
    )
        .into_response();
    annotate(&mut res, limit);
    if let Ok(v) = HeaderValue::from_str(&limit.retry_after.to_string()) {
        res.headers_mut().insert("retry-after", v);
    }
    res
}

pub fn annotate(res: &mut Response, limit: &Limit) {
    let h = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&limit.max.to_string()) {
        h.insert("x-ratelimit-limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&limit.remaining.to_string()) {
        h.insert("x-ratelimit-remaining", v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_and_resets_after_window() {
        let t0 = Instant::now();
        for i in 0..3 {
            let l = check_with("throttle-test-key", 3, t0);
            assert!(l.allowed);
            assert_eq!(l.remaining, 2 - i);
        }
        assert!(!check_with("throttle-test-key", 3, t0).allowed);
        assert!(check_with("throttle-test-key", 3, t0 + WINDOW).allowed);
    }
}
