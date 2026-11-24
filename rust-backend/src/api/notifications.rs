//! Native `/api/v1/notifications/{channel}` handlers (Laravel `NotificationsController`).
//!
//! GET returns the team's settings row (secrets only for tokens that may read them, decrypted);
//! PATCH validates against the channel rules, fills the allowed fields and records
//! `api.notifications.<channel>.updated`.

#[allow(unused_imports)]
use super::prelude::*;

use axum::{
    body::{to_bytes, Body},
    extract::Request,
};
use serde_json::{Map, Value};

/// One rule set from `NotificationsController::channelConfig()` (every rule is `sometimes`).
#[derive(Clone, Copy)]
enum Kind {
    /// `boolean`
    Bool,
    /// `nullable|string|max:n`
    Str(usize),
    /// `nullable|integer|min:m[|max:n]`
    Int(i64, Option<i64>),
    /// `nullable|string|in:...`
    In(&'static [&'static str]),
    /// `nullable|string|max:255` + `ValidHostname`
    Hostname,
    /// `email`, `SafeSmtpHost` or `SafeWebhookUrl` (DNS resolution and SSRF policy).
    /// ponytail: a non-null value sends the request to Laravel until those rules are ported (step 7).
    Delegated,
}

use Kind::*;

#[cfg(test)]
const CHANNELS: &[&str] = &[
    "email", "discord", "slack", "telegram", "pushover", "webhook",
];

const EVENTS: &[&str] = &[
    "deployment_success",
    "deployment_failure",
    "status_change",
    "restart_limit_reached",
    "backup_success",
    "backup_failure",
    "scheduled_task_success",
    "scheduled_task_failure",
    "docker_cleanup_success",
    "docker_cleanup_failure",
    "server_disk_usage",
    "server_reachable",
    "server_unreachable",
    "server_patch",
    "traefik_outdated",
    "image_vulnerabilities",
];

const EMAIL_FIELDS: &[(&str, Kind)] = &[
    ("smtp_enabled", Bool),
    ("smtp_from_address", Delegated),
    ("smtp_from_name", Str(255)),
    ("smtp_recipients", Str(1000)),
    ("smtp_host", Delegated),
    ("smtp_port", Int(1, Some(65535))),
    ("smtp_encryption", In(&["starttls", "tls", "none"])),
    ("smtp_username", Str(255)),
    ("smtp_password", Str(255)),
    ("smtp_timeout", Int(0, None)),
    ("smtp_ehlo_domain", Hostname),
    ("resend_enabled", Bool),
    ("resend_api_key", Str(255)),
    ("resend_from_address", Delegated),
    ("resend_from_name", Str(255)),
    ("use_instance_email_settings", Bool),
    ("deployment_success_email_notifications", Bool),
    ("deployment_failure_email_notifications", Bool),
    ("deployment_commit_details_email_notifications", Bool),
    ("status_change_email_notifications", Bool),
    ("restart_limit_reached_email_notifications", Bool),
    ("backup_success_email_notifications", Bool),
    ("backup_failure_email_notifications", Bool),
    ("scheduled_task_success_email_notifications", Bool),
    ("scheduled_task_failure_email_notifications", Bool),
    ("docker_cleanup_success_email_notifications", Bool),
    ("docker_cleanup_failure_email_notifications", Bool),
    ("server_disk_usage_email_notifications", Bool),
    ("server_reachable_email_notifications", Bool),
    ("server_unreachable_email_notifications", Bool),
    ("server_patch_email_notifications", Bool),
    ("traefik_outdated_email_notifications", Bool),
    ("image_vulnerabilities_email_notifications", Bool),
];

/// The model's `$fillable` minus `team_id` (model order) with each field's rule.
fn fields(channel: &str) -> Vec<(String, Kind)> {
    let own = |list: &[(&str, Kind)]| {
        list.iter()
            .map(|(f, k)| (f.to_string(), *k))
            .collect::<Vec<_>>()
    };
    let mut out = match channel {
        "email" => return own(EMAIL_FIELDS),
        "discord" => own(&[
            ("discord_enabled", Bool),
            ("discord_webhook_url", Delegated),
        ]),
        "slack" => own(&[("slack_enabled", Bool), ("slack_webhook_url", Delegated)]),
        "telegram" => own(&[
            ("telegram_enabled", Bool),
            ("telegram_token", Str(255)),
            ("telegram_chat_id", Str(255)),
        ]),
        "pushover" => own(&[
            ("pushover_enabled", Bool),
            ("pushover_user_key", Str(255)),
            ("pushover_api_token", Str(255)),
        ]),
        _ => own(&[("webhook_enabled", Bool), ("webhook_url", Delegated)]),
    };
    out.extend(
        EVENTS
            .iter()
            .map(|e| (format!("{e}_{channel}_notifications"), Bool)),
    );
    match channel {
        "discord" => out.push(("discord_ping_enabled".into(), Bool)),
        "telegram" => out.extend(
            EVENTS
                .iter()
                .map(|e| (format!("telegram_notifications_{e}_thread_id"), Str(255))),
        ),
        _ => {}
    }
    out
}

/// Columns that are both `$hidden` and `encrypted` (the same set on every channel model).
fn secrets(channel: &str) -> Vec<String> {
    let list: &[&str] = match channel {
        "email" => &[
            "smtp_from_address",
            "smtp_from_name",
            "smtp_recipients",
            "smtp_host",
            "smtp_username",
            "smtp_password",
            "resend_api_key",
            "resend_from_address",
            "resend_from_name",
        ],
        "discord" => &["discord_webhook_url"],
        "slack" => &["slack_webhook_url"],
        "pushover" => &["pushover_user_key", "pushover_api_token"],
        "webhook" => &["webhook_url"],
        _ => &["telegram_token", "telegram_chat_id"],
    };
    let mut out: Vec<String> = list.iter().map(|s| s.to_string()).collect();
    if channel == "telegram" {
        out.extend(
            EVENTS
                .iter()
                .map(|e| format!("telegram_notifications_{e}_thread_id")),
        );
    }
    out
}

fn table(channel: &str) -> String {
    format!("{channel}_notification_settings")
}

async fn fetch(
    pool: &PgPool,
    channel: &str,
    team_id: i64,
) -> Result<Option<Map<String, Value>>, sqlx::Error> {
    let sql = format!(
        "SELECT to_jsonb(t) FROM {} t WHERE team_id = $1 LIMIT 1",
        table(channel)
    );
    Ok(sqlx::query_scalar::<_, Value>(&sql)
        .bind(team_id)
        .fetch_optional(pool)
        .await?
        .map(as_map))
}

/// `firstOrCreate(['team_id' => ...])`. A newly created model holds only `team_id` (the token's
/// string) and `id`, which is what Laravel serializes for it.
async fn resolve(
    pool: &PgPool,
    channel: &str,
    team_id: i64,
) -> Result<(Map<String, Value>, bool), sqlx::Error> {
    if let Some(row) = fetch(pool, channel, team_id).await? {
        return Ok((row, false));
    }
    let sql = format!(
        "INSERT INTO {} (team_id) VALUES ($1) RETURNING id",
        table(channel)
    );
    let id: i64 = sqlx::query_scalar(&sql)
        .bind(team_id)
        .fetch_one(pool)
        .await?;
    Ok((
        as_map(json!({"team_id": team_id.to_string(), "id": id})),
        true,
    ))
}

/// `exposeSensitiveFields` + `serializeApiResponse`: secrets only for tokens that may read them.
fn serialize(row: Map<String, Value>, channel: &str, ctx: &Ctx, state: &crate::AppState) -> Value {
    let reveal = ctx.can_read_sensitive();
    let secrets = secrets(channel);
    // `HasSmtpServers` adds `smtp_servers` to the email model: hidden, `encrypted:array`.
    let is_array_secret = |k: &str| channel == "email" && k == "smtp_servers";
    let decrypt = |s: &str| {
        state
            .app_key
            .as_ref()
            .and_then(|key| crate::crypto::decrypt(s, key))
    };
    let out: Map<String, Value> = row
        .into_iter()
        .filter(|(k, _)| reveal || !(secrets.contains(k) || is_array_secret(k)))
        .map(|(k, v)| {
            let v = match v {
                // ponytail: an undecryptable value becomes null; Laravel answers 500 for it.
                Value::String(s) if is_array_secret(&k) => decrypt(&s)
                    .and_then(|plain| serde_json::from_str(&plain).ok())
                    .unwrap_or(Value::Null),
                Value::String(s) if secrets.contains(&k) => json!(decrypt(&s)),
                v => v,
            };
            (k, v)
        })
        .collect();
    Value::Object(out)
}

async fn show(state: crate::AppState, ctx: Ctx, channel: &str) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match resolve(pool, channel, team_id).await {
        Ok((row, _)) => ok_json(serialize(row, channel, &ctx, &state)),
        Err(e) => query_err("show_notifications", e),
    }
}

/// PHP `filter_var($v, FILTER_VALIDATE_INT)` (the `integer` rule).
fn php_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)),
        Value::Bool(true) => Some(1),
        Value::String(s) => {
            let t = s.trim();
            let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
            let ok = !digits.is_empty()
                && digits.bytes().all(|c| c.is_ascii_digit())
                && (digits == "0" || !digits.starts_with('0'));
            ok.then(|| t.parse().ok()).flatten()
        }
        _ => None,
    }
}

/// `App\Rules\ValidHostname`; `:attribute` is replaced by the caller.
fn hostname_error(value: &str) -> Option<&'static str> {
    const INVALID: &str = "The :attribute contains invalid characters. Only letters (a-z, A-Z), numbers (0-9), hyphens (-), and dots (.) are allowed.";
    const DANGEROUS: &[char] = &[
        ';', '|', '&', '$', '`', '(', ')', '{', '}', '<', '>', '\n', '\r', '\0', '"', '\'', '\\',
        '!', '*', '?', '[', ']', '~', '^', ':', '#', '@', '%', '=', '+', ',', ' ',
    ];
    if value.is_empty() || value == "0" {
        return None;
    }
    let h = value.trim_matches([' ', '\t', '\n', '\r', '\0', '\x0B']);
    if h.len() > 253 {
        return Some("The :attribute must not exceed 253 characters.");
    }
    if h.bytes().any(|b| b < 0x20 || b == 0x7f) || h.contains(DANGEROUS) {
        return Some(INVALID);
    }
    let h = h.to_lowercase();
    if h.starts_with('.') || h.ends_with('.') {
        return Some("The :attribute cannot start or end with a dot.");
    }
    if h.contains("..") {
        return Some("The :attribute cannot contain consecutive dots.");
    }
    for label in h.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Some(
                "The :attribute contains an invalid label. Each segment must be 1-63 characters.",
            );
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Some("The :attribute contains an invalid label. Labels cannot start or end with a hyphen.");
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Some(INVALID);
        }
    }
    None
}

fn validate<'a>(input: &'a Map<String, Value>, rules: &[(String, Kind)]) -> Validator<'a> {
    let mut v = Validator::new(input, &[]);
    for (field, kind) in rules {
        let attr = field.replace('_', " ");
        let value = input.get(field.as_str());
        match kind {
            Bool => {
                v.field(field, &[Rule::Boolean]);
            }
            Str(max) => {
                v.field(field, &[Rule::Nullable, Rule::Str, Rule::Max(*max)]);
            }
            In(list) => {
                v.field(field, &[Rule::Nullable, Rule::Str]);
                let valid = match value {
                    None | Some(Value::Null) => true,
                    Some(Value::String(s)) => list.contains(&s.as_str()),
                    Some(_) => false,
                };
                if !valid {
                    v.add(field, &format!("The selected {attr} is invalid."));
                }
            }
            Hostname => {
                v.field(field, &[Rule::Nullable, Rule::Str, Rule::Max(255)]);
                if let Some(msg) = value.and_then(Value::as_str).and_then(hostname_error) {
                    v.add(field, &msg.replace(":attribute", &attr));
                }
            }
            Int(min, max) => match value {
                None | Some(Value::Null) => {}
                Some(val) => match php_int(val) {
                    Some(n) => {
                        if n < *min {
                            v.add(field, &format!("The {attr} field must be at least {min}."));
                        }
                        if let Some(mx) = max.filter(|mx| n > *mx) {
                            v.add(
                                field,
                                &format!("The {attr} field must not be greater than {mx}."),
                            );
                        }
                    }
                    None => {
                        v.add(field, &format!("The {attr} field must be an integer."));
                        // Not numeric: min/max fall back to the string length.
                        let mut size = vec![Rule::Min(*min as usize)];
                        if let Some(mx) = max {
                            size.push(Rule::Max(*mx as usize));
                        }
                        v.field(field, &size);
                    }
                },
            },
            Delegated => {}
        }
    }
    v
}

/// The stored form of a validated value (Eloquent casts on `fill`).
fn cast(kind: Kind, value: &Value) -> Value {
    match (kind, value) {
        (_, Value::Null) => Value::Null,
        (Bool, v) => Value::Bool(matches!(v, Value::Bool(true)) || php_int(v) == Some(1)),
        (Int(..), v) => php_int(v).map_or(Value::Null, |n| json!(n)),
        (_, v) => v.clone(),
    }
}

fn server_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "message": msg })),
    )
        .into_response()
}

async fn update(state: crate::AppState, ctx: Ctx, channel: &str, req: Request) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (parts, body) = req.into_parts();
    let Ok(bytes) = to_bytes(body, usize::MAX).await else {
        return server_error("Unreadable body.");
    };
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let rules = fields(channel);
    let delegated = rules.iter().any(|(f, k)| {
        matches!(k, Delegated) && input.get(f.as_str()).is_some_and(|v| !v.is_null())
    });
    if delegated {
        return crate::proxy(State(state), Request::from_parts(parts, Body::from(bytes))).await;
    }

    let allowed: Vec<&str> = rules.iter().map(|(f, _)| f.as_str()).collect();
    let mut v = validate(&input, &rules);
    v.reject_extra(&allowed);
    if v.fails() {
        return v.response();
    }

    let (old, created) = match resolve(pool, channel, team_id).await {
        Ok(r) => r,
        Err(e) => return query_err("update_notifications", e),
    };
    let secrets = secrets(channel);
    let mut sets: Vec<(String, Value)> = Vec::new();
    for (field, kind) in &rules {
        let Some(raw) = input.get(field.as_str()) else {
            continue;
        };
        let new = cast(*kind, raw);
        if secrets.contains(field) {
            // `encrypted` cast: dirty only when the plaintext differs.
            let old_plain = old
                .get(field.as_str())
                .and_then(Value::as_str)
                .and_then(|s| {
                    state
                        .app_key
                        .as_ref()
                        .and_then(|k| crate::crypto::decrypt(s, k))
                });
            let new_plain = new.as_str().map(str::to_string);
            if !created && old_plain == new_plain {
                continue;
            }
            let stored = match (&new_plain, state.app_key.as_ref()) {
                (None, _) => Value::Null,
                (Some(p), Some(k)) => match crate::crypto::encrypt_string(p, k) {
                    Some(enc) => Value::String(enc),
                    None => return server_error("Encryption failed."),
                },
                (Some(_), None) => return server_error("APP_KEY not configured."),
            };
            sets.push((field.clone(), stored));
        } else if created || old.get(field.as_str()) != Some(&new) {
            sets.push((field.clone(), new));
        }
    }
    if !sets.is_empty() {
        let mut assignments: Vec<String> = sets
            .iter()
            .enumerate()
            .map(|(i, (f, _))| format!("{f} = ${}", i + 1))
            .collect();
        if !created {
            // TracksDeliveryFailures: any real change resets the failure counter.
            assignments.push("failure_count = 0".into());
        }
        let id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
        let sql = format!(
            "UPDATE {} SET {} WHERE id = ${}",
            table(channel),
            assignments.join(", "),
            sets.len() + 1
        );
        let mut q = sqlx::query(&sql);
        for (_, value) in &sets {
            q = match value {
                Value::Bool(b) => q.bind(*b),
                Value::Number(n) => q.bind(n.as_i64()),
                Value::String(s) => q.bind(s.clone()),
                _ => q.bind(Option::<String>::None),
            };
        }
        if let Err(e) = q.bind(id).execute(pool).await {
            return query_err("update_notifications", e);
        }
    }

    let changed: Vec<&str> = allowed
        .iter()
        .copied()
        .filter(|f| input.contains_key(*f))
        .collect();
    audit::record(
        pool,
        &ctx,
        &format!("api.notifications.{channel}.updated"),
        json!({"team_id": team_id, "changed_fields": changed}),
        "info",
    )
    .await;

    match fetch(pool, channel, team_id).await {
        Ok(Some(row)) => ok_json(serialize(row, channel, &ctx, &state)),
        Ok(None) => not_found("Not found."),
        Err(e) => query_err("update_notifications", e),
    }
}

macro_rules! channel_handlers {
    ($($ch:literal => $get:ident, $patch:ident;)*) => {
        $(
            pub async fn $get(State(state): State<crate::AppState>, Extension(ctx): Extension<Ctx>) -> Response {
                show(state, ctx, $ch).await
            }
            pub async fn $patch(
                State(state): State<crate::AppState>,
                Extension(ctx): Extension<Ctx>,
                req: Request,
            ) -> Response {
                update(state, ctx, $ch, req).await
            }
        )*
    };
}

channel_handlers! {
    "email" => get_email_notifications, update_email_notifications;
    "discord" => get_discord_notifications, update_discord_notifications;
    "slack" => get_slack_notifications, update_slack_notifications;
    "telegram" => get_telegram_notifications, update_telegram_notifications;
    "pushover" => get_pushover_notifications, update_pushover_notifications;
    "webhook" => get_webhook_notifications, update_webhook_notifications;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_int_matches_filter_validate_int() {
        assert_eq!(php_int(&json!(80)), Some(80));
        assert_eq!(php_int(&json!("80")), Some(80));
        assert_eq!(php_int(&json!(" 80 ")), Some(80));
        assert_eq!(php_int(&json!("080")), None);
        assert_eq!(php_int(&json!("8a")), None);
        assert_eq!(php_int(&json!(true)), Some(1));
        assert_eq!(php_int(&json!(false)), None);
    }

    #[test]
    fn valid_hostname_rule() {
        assert_eq!(hostname_error("mail.example.com"), None);
        assert!(hostname_error("bad host").is_some());
        assert!(hostname_error("-a.com").is_some());
        assert!(hostname_error("a..com").is_some());
    }

    #[test]
    fn field_lists_match_the_models() {
        let count = |c: &str| fields(c).len();
        assert_eq!(count("email"), 33);
        assert_eq!(count("discord"), 19);
        assert_eq!(count("slack"), 18);
        assert_eq!(count("telegram"), 35);
        assert_eq!(count("pushover"), 19);
        assert_eq!(count("webhook"), 18);
        assert_eq!(secrets("telegram").len(), 18);
        for c in CHANNELS {
            let names: Vec<String> = fields(c).into_iter().map(|(f, _)| f).collect();
            for s in secrets(c) {
                assert!(names.contains(&s), "{c}: secret {s} is not fillable");
            }
        }
    }
}
