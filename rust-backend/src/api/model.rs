//! Laravel-compatible JSON for database rows (Eloquent `toArray()` / `toJson()` semantics).
//!
//! Handlers select rows as JSON with `SELECT to_jsonb(t) FROM (...) t`, then pass each row through
//! [`to_laravel`] with the model's [`ModelSpec`] so hidden attributes, date formats and JSON columns
//! come out exactly as Laravel serializes them.

use serde_json::{Map, Value};

/// Per-model serialization rules, copied from the Eloquent model (`$hidden`, `$casts`).
#[derive(Clone, Copy, Default)]
pub struct ModelSpec {
    /// `$hidden` (and any `makeHidden` the controller always applies).
    pub hidden: &'static [&'static str],
    /// Columns cast to `datetime`/`immutable_datetime` besides `created_at`/`updated_at`
    /// (Carbon JSON: `2026-11-24T11:23:20.000000Z`).
    pub datetimes: &'static [&'static str],
    /// Columns cast to `date` (Carbon JSON at midnight UTC).
    pub dates: &'static [&'static str],
    /// `json`/`jsonb` columns WITHOUT an `array`/`json` cast: Laravel returns them as the raw string.
    pub json_strings: &'static [&'static str],
    /// The model has no `created_at`/`updated_at` timestamps (`$timestamps = false`).
    pub no_timestamps: bool,
}

/// Apply a model spec to one row produced by `to_jsonb`.
pub fn to_laravel(row: Value, spec: &ModelSpec) -> Value {
    let Value::Object(map) = row else { return row };
    let out: Map<String, Value> = map
        .into_iter()
        .filter(|(k, _)| !spec.hidden.contains(&k.as_str()))
        .map(|(k, v)| {
            let is_timestamp = !spec.no_timestamps && (k == "created_at" || k == "updated_at");
            let v = match v {
                Value::String(s) if is_timestamp || spec.datetimes.contains(&k.as_str()) => {
                    Value::String(carbon(&s))
                }
                Value::String(s) if spec.dates.contains(&k.as_str()) => {
                    Value::String(carbon(&format!("{}T00:00:00", &s[..s.len().min(10)])))
                }
                // Uncast timestamp columns come back from PDO as "Y-m-d H:i:s".
                Value::String(s) if looks_like_pg_timestamp(&s) => {
                    Value::String(s.replacen('T', " ", 1))
                }
                v @ (Value::Object(_) | Value::Array(_))
                    if spec.json_strings.contains(&k.as_str()) =>
                {
                    Value::String(v.to_string())
                }
                v => v,
            };
            (k, v)
        })
        .collect();
    Value::Object(out)
}

/// Apply a spec to every row.
pub fn all_to_laravel(rows: Vec<Value>, spec: &ModelSpec) -> Vec<Value> {
    rows.into_iter().map(|r| to_laravel(r, spec)).collect()
}

/// `to_jsonb` timestamp ("2026-11-24T11:23:20", optionally with a fraction) -> Carbon JSON
/// ("2026-11-24T11:23:20.000000Z").
pub fn carbon(s: &str) -> String {
    let s = s.replacen(' ', "T", 1);
    let (base, rest) = s.split_at(s.len().min(19));
    let frac: String = rest
        .strip_prefix('.')
        .map(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect())
        .unwrap_or_default();
    format!("{base}.{frac:0<6}Z")
}

/// `to_jsonb` renders `timestamp without time zone` as `YYYY-MM-DDTHH:MM:SS[.ffffff]`.
fn looks_like_pg_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 19
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[..4].iter().all(u8::is_ascii_digit)
        && !s.ends_with('Z')
        && !s[19..].contains('+')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn carbon_format_matches_laravel() {
        assert_eq!(carbon("2026-11-24T11:23:20"), "2026-11-24T11:23:20.000000Z");
        assert_eq!(
            carbon("2026-11-24T11:23:20.12"),
            "2026-11-24T11:23:20.120000Z"
        );
        assert_eq!(carbon("2026-11-24 11:23:20"), "2026-11-24T11:23:20.000000Z");
    }

    #[test]
    fn spec_hides_casts_and_stringifies() {
        const SPEC: ModelSpec = ModelSpec {
            hidden: &["secret"],
            datetimes: &["finished_at"],
            dates: &[],
            json_strings: &["raw"],
            no_timestamps: false,
        };
        let out = to_laravel(
            json!({
                "secret": "x", "created_at": "2026-01-02T03:04:05", "finished_at": "2026-01-02T03:04:05",
                "seen_at": "2026-01-02T03:04:05", "raw": {"a": 1}, "name": "n"
            }),
            &SPEC,
        );
        assert!(out.get("secret").is_none());
        assert_eq!(out["created_at"], "2026-01-02T03:04:05.000000Z");
        assert_eq!(out["finished_at"], "2026-01-02T03:04:05.000000Z");
        assert_eq!(out["seen_at"], "2026-01-02 03:04:05");
        assert_eq!(out["raw"], "{\"a\":1}");
        assert_eq!(out["name"], "n");
    }
}
