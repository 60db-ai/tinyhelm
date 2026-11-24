//! Five-field cron expressions and `shouldRunCronNow` (bootstrap/helpers/shared.php).
//!
//! Semantics of dragonmantank/cron-expression: `*`, numbers, `a-b`, `*/n`, `a-b/n`, lists; day-of-week
//! 0-7 (0 and 7 are Sunday); when both day-of-month and day-of-week are restricted, either may match.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

/// `VALID_CRON_STRINGS` (bootstrap/helpers/constants.php).
pub fn resolve_alias(expr: &str) -> &str {
    match expr {
        "every_minute" => "* * * * *",
        "hourly" | "@hourly" => "0 * * * *",
        "daily" | "@daily" => "0 0 * * *",
        "weekly" | "@weekly" => "0 0 * * 0",
        "monthly" | "@monthly" => "0 0 1 * *",
        "yearly" | "@yearly" => "0 0 1 1 *",
        other => other,
    }
}

#[derive(Debug, Clone)]
pub struct Cron {
    minute: Vec<bool>,
    hour: Vec<bool>,
    dom: Vec<bool>,
    month: Vec<bool>,
    dow: Vec<bool>,
    dom_star: bool,
    dow_star: bool,
}

fn field(spec: &str, min: u32, max: u32) -> Option<Vec<bool>> {
    let mut set = vec![false; (max + 1) as usize];
    for part in spec.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (r, s.parse::<u32>().ok().filter(|s| *s > 0)?),
            None => (part, 1),
        };
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (a.parse().ok()?, b.parse().ok()?)
        } else {
            let v: u32 = range.parse().ok()?;
            // "5/15" means from 5 to the end, stepping 15.
            (v, if part.contains('/') { max } else { v })
        };
        if lo < min || hi > max || lo > hi {
            return None;
        }
        let mut v = lo;
        while v <= hi {
            set[v as usize] = true;
            v += step;
        }
    }
    Some(set)
}

impl Cron {
    pub fn parse(expr: &str) -> Option<Self> {
        let parts: Vec<&str> = resolve_alias(expr.trim()).split_whitespace().collect();
        let [m, h, dom, mon, dow] = parts.as_slice() else {
            return None;
        };
        let mut dow_set = field(dow, 0, 7)?;
        if dow_set[7] {
            dow_set[0] = true;
        }
        Some(Self {
            minute: field(m, 0, 59)?,
            hour: field(h, 0, 23)?,
            dom: field(dom, 1, 31)?,
            month: field(mon, 1, 12)?,
            dow: dow_set,
            dom_star: *dom == "*",
            dow_star: *dow == "*",
        })
    }

    pub fn is_due<T: TimeZone>(&self, t: &DateTime<T>) -> bool {
        let dom = self.dom[t.day() as usize];
        let dow = self.dow[t.weekday().num_days_from_sunday() as usize];
        let day = match (self.dom_star, self.dow_star) {
            (true, true) => true,
            (false, true) => dom,
            (true, false) => dow,
            (false, false) => dom || dow,
        };
        day && self.minute[t.minute() as usize]
            && self.hour[t.hour() as usize]
            && self.month[t.month() as usize]
    }
}

/// Parse an IANA time zone; invalid names fall back to UTC (Laravel: `config('app.timezone')`).
pub fn tz(name: &str) -> Tz {
    name.parse().unwrap_or(chrono_tz::UTC)
}

static LAST_FIRED: Mutex<Option<HashMap<String, DateTime<Utc>>>> = Mutex::new(None);

/// `shouldRunCronNow($frequency, $timezone, $dedupKey, $executionTime)`: due now on first sight; later,
/// due when a scheduled minute passed since the last dispatch (a missed run catches up once).
// ponytail: dedup lives in process memory (one scheduler holds the onOneServer lock); Laravel keeps it in its cache.
pub fn should_run_now(expr: &str, timezone: &str, dedup_key: &str, now: DateTime<Utc>) -> bool {
    let Some(cron) = Cron::parse(expr) else {
        return false;
    };
    let zone = tz(timezone);
    let now = now
        .with_second(0)
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(now);
    let mut guard = LAST_FIRED.lock().unwrap_or_else(|e| e.into_inner());
    let last = guard.get_or_insert_with(HashMap::new);
    let fire = match last.get(dedup_key) {
        None => cron.is_due(&now.with_timezone(&zone)),
        Some(prev) => {
            // A due minute in (prev, now]; bounded by the last dispatch, normally a minute ago.
            let mut t = now;
            let mut due = false;
            while t > *prev && now - t <= Duration::days(31) {
                if cron.is_due(&t.with_timezone(&zone)) {
                    due = true;
                    break;
                }
                t -= Duration::minutes(1);
            }
            due
        }
    };
    if fire || !last.contains_key(dedup_key) {
        last.insert(dedup_key.to_string(), now);
    }
    fire
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn parses_and_matches() {
        let c = Cron::parse("*/15 2 * * *").unwrap();
        assert!(c.is_due(&utc(2026, 1, 5, 2, 30)));
        assert!(!c.is_due(&utc(2026, 1, 5, 2, 31)));
        assert!(!c.is_due(&utc(2026, 1, 5, 3, 30)));
        // 2026-01-04 is a Sunday: 7 and 0 both mean Sunday.
        assert!(Cron::parse("0 4 * * 7")
            .unwrap()
            .is_due(&utc(2026, 1, 4, 4, 0)));
        assert!(Cron::parse("weekly")
            .unwrap()
            .is_due(&utc(2026, 1, 4, 0, 0)));
        // dom and dow both restricted: either matches (2026-01-05 is a Monday).
        assert!(Cron::parse("0 0 1 * 1")
            .unwrap()
            .is_due(&utc(2026, 1, 5, 0, 0)));
        assert!(Cron::parse("bad").is_none());
        assert!(Cron::parse("61 * * * *").is_none());
    }

    #[test]
    fn time_zone_shifts_the_due_minute() {
        // 23:00 in Asia/Kolkata is 17:30 UTC.
        let c = Cron::parse("0 23 * * *").unwrap();
        assert!(c.is_due(&utc(2026, 1, 5, 17, 30).with_timezone(&tz("Asia/Kolkata"))));
        assert!(!c.is_due(&utc(2026, 1, 5, 23, 0).with_timezone(&tz("Asia/Kolkata"))));
    }

    #[test]
    fn dedup_fires_once_and_catches_up_a_missed_run() {
        let key = "cron-test-dedup";
        assert!(!should_run_now(
            "0 * * * *",
            "UTC",
            key,
            utc(2026, 1, 5, 9, 59)
        ));
        // 10:00 was missed; the next tick still fires once.
        assert!(should_run_now(
            "0 * * * *",
            "UTC",
            key,
            utc(2026, 1, 5, 10, 1)
        ));
        assert!(!should_run_now(
            "0 * * * *",
            "UTC",
            key,
            utc(2026, 1, 5, 10, 2)
        ));
        assert!(should_run_now(
            "* * * * *",
            "UTC",
            "cron-test-every",
            utc(2026, 1, 5, 10, 2)
        ));
        assert!(!should_run_now(
            "* * * * *",
            "UTC",
            "cron-test-every",
            utc(2026, 1, 5, 10, 2)
        ));
    }
}
