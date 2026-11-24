//! Audit events, written to the same `audit_events` table with the same derived columns as Laravel's
//! `auditLog()` -> `AuditEvent::attributesFor()`, so the Monitoring page sees native and PHP actions alike.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::auth::Ctx;

const SENSITIVE_VALUE_FIELDS: &[&str] = &[
    "git_full_url",
    "install_command",
    "build_command",
    "start_command",
    "health_check_command",
    "health_check_response_text",
    "custom_docker_run_options",
    "pre_deployment_command",
    "post_deployment_command",
    "docker_compose_custom_start_command",
    "docker_compose_custom_build_command",
    "custom_nginx_configuration",
    "postgres_conf",
    "mysql_conf",
    "mariadb_conf",
    "mongo_conf",
    "redis_conf",
    "keydb_conf",
    "internal_db_url",
    "external_db_url",
    "init_scripts",
    "dockerfile",
    "docker_compose",
    "docker_compose_raw",
    "custom_labels",
    "last_saved_proxy_configuration",
    "environment_variables",
    "environment_variables_preview",
    "validation_logs",
    "server_metadata",
    "logs",
    "configuration_snapshot",
    "configuration_diff",
    "content",
    "file_storage_content",
];

const SENSITIVE_KEY_PARTS: &[&str] = &[
    "password",
    "secret",
    "token",
    "private_key",
    "signature",
    "credential",
    "invitation_email",
    "api_key",
    "access_key",
    "authorization",
    "cookie",
    "license_key",
];

/// Record an audit event. Failures are logged and never interrupt the request (as in Laravel).
pub async fn record(pool: &PgPool, ctx: &Ctx, event: &str, context: Value, level: &str) {
    let context = match context {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    if let Some(window) = dedup_window(event) {
        if !first_in_window(event, ctx, &context, window) {
            return;
        }
    }

    let a = attributes(event, &context, level);
    let result = sqlx::query(
        "INSERT INTO audit_events (team_id, event, source, action, level, actor_type, actor_id, actor_name, \
         actor_email, actor_token_id, actor_token_name, resource_type, resource_uuid, resource_name, description, \
         metadata, ip_address, user_agent, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16::json,$17,$18,now())",
    )
    .bind(ctx.team_id)
    .bind(event)
    .bind(&a.source)
    .bind(&a.action)
    .bind(&a.level)
    .bind(&a.actor_type)
    .bind(ctx.user_id)
    .bind(&ctx.user_name)
    .bind(&ctx.user_email)
    .bind(ctx.token_id)
    .bind(&ctx.token_name)
    .bind(&a.resource_type)
    .bind(&a.resource_uuid)
    .bind(&a.resource_name)
    .bind(&a.description)
    .bind(a.metadata.to_string())
    .bind(&ctx.ip)
    .bind(&ctx.user_agent)
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::warn!("audit event persistence failed for {event}: {e}");
    }
}

struct Attributes {
    source: String,
    action: String,
    level: String,
    actor_type: String,
    resource_type: Option<String>,
    resource_uuid: Option<String>,
    resource_name: Option<String>,
    description: String,
    metadata: Value,
}

fn attributes(event: &str, context: &Map<String, Value>, level: &str) -> Attributes {
    let parts: Vec<&str> = event.split('.').collect();
    let source = parts.first().copied().unwrap_or("system").to_string();
    let resource_type =
        str_field(context, "resource").or_else(|| parts.get(1).map(|s| s.to_string()));
    let action = str_field(context, "action")
        .or_else(|| {
            parts
                .last()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "event".into());
    let resource_uuid = first_context_value(
        context,
        resource_type.as_deref().map(|t| format!("{t}_uuid")),
        "_uuid",
    );
    let resource_name = first_context_value(
        context,
        resource_type.as_deref().map(|t| format!("{t}_name")),
        "_name",
    );
    let actor_type = if ["mcp", "webhook", "system", "scheduler"].contains(&source.as_str()) {
        source.clone()
    } else {
        "api_token".into()
    };
    let description = str_field(context, "audit_description").unwrap_or_else(|| {
        let subject = resource_name
            .clone()
            .unwrap_or_else(|| headline(resource_type.as_deref().unwrap_or("")));
        format!("{subject} {}", headline(&action))
            .trim()
            .to_string()
    });

    let mut metadata = context.clone();
    for k in [
        "audit_changes",
        "actor_email",
        "actor_ip",
        "actor_user_agent",
    ] {
        metadata.remove(k);
    }

    Attributes {
        source,
        action,
        level: if ["info", "warning", "error"].contains(&level) {
            level.into()
        } else {
            "info".into()
        },
        actor_type,
        resource_type,
        resource_uuid,
        resource_name,
        description,
        metadata: redact(Value::Object(metadata), None),
    }
}

fn str_field(context: &Map<String, Value>, key: &str) -> Option<String> {
    match context.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::String(_) | Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// `AuditEvent::firstContextValue`: the preferred key when filled, else the first key with the suffix.
fn first_context_value(
    context: &Map<String, Value>,
    preferred: Option<String>,
    suffix: &str,
) -> Option<String> {
    if let Some(v) = preferred.as_deref().and_then(|k| str_field(context, k)) {
        return Some(v);
    }
    let key = context.keys().find(|k| k.ends_with(suffix))?;
    str_field(context, key)
}

/// Laravel `Str::headline`: "shared_env" -> "Shared Env", "abilityDenied" -> "Ability Denied".
pub fn headline(s: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch.is_whitespace() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
        current.push(ch);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
        .iter()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    SENSITIVE_VALUE_FIELDS.contains(&key) || SENSITIVE_KEY_PARTS.iter().any(|p| lower.contains(p))
}

/// `AuditEvent::redact`: values under sensitive keys become "[REDACTED]", recursively.
pub fn redact(value: Value, key: Option<&str>) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String("[REDACTED]".into());
    }
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let red = redact(v, Some(&k));
                    (k, red)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| redact(v, None)).collect()),
        other => other,
    }
}

/// `auditFailureDeduplicationWindow` for the events native routes emit.
fn dedup_window(event: &str) -> Option<Duration> {
    let rest = event
        .strip_prefix("api.auth.")
        .or_else(|| event.strip_prefix("mcp.auth."));
    match rest {
        Some("ability_denied" | "scope_denied" | "unauthenticated") => {
            Some(Duration::from_secs(300))
        }
        _ => None,
    }
}

static SEEN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

/// `Cache::add` on a fingerprint of event + token + reason + abilities.
// ponytail: per-process memory; it joins Laravel's cache only when the cache layer is ported.
fn first_in_window(event: &str, ctx: &Ctx, context: &Map<String, Value>, window: Duration) -> bool {
    let fingerprint = hex::encode(Sha256::digest(
        serde_json::json!([
            event,
            ctx.token_id,
            context.get("reason"),
            context.get("required_abilities")
        ])
        .to_string(),
    ));
    let mut guard = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let seen = guard.get_or_insert_with(HashMap::new);
    let now = Instant::now();
    seen.retain(|_, until| *until > now);
    if seen.contains_key(&fingerprint) {
        return false;
    }
    seen.insert(fingerprint, now + window);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::auth::tests_support::ctx;
    use serde_json::json;

    #[test]
    fn headline_matches_laravel() {
        assert_eq!(headline("shared_env"), "Shared Env");
        assert_eq!(headline("environment_created"), "Environment Created");
        assert_eq!(headline("tag"), "Tag");
        assert_eq!(headline("abilityDenied"), "Ability Denied");
    }

    #[test]
    fn redacts_sensitive_keys_recursively() {
        let v = redact(
            json!({"tag_name": "x", "value": "keep", "nested": {"api_key": "s", "private_key": "k"}, "password": "p"}),
            None,
        );
        assert_eq!(v["tag_name"], "x");
        assert_eq!(v["value"], "keep");
        assert_eq!(v["nested"]["api_key"], "[REDACTED]");
        assert_eq!(v["nested"]["private_key"], "[REDACTED]");
        assert_eq!(v["password"], "[REDACTED]");
    }

    #[test]
    fn resource_columns_derive_from_event_and_context() {
        let context = json!({"team_id": 1, "tag_uuid": "t1", "tag_name": "prod"});
        let a = attributes("api.tag.created", context.as_object().unwrap(), "info");
        assert_eq!(a.source, "api");
        assert_eq!(a.action, "created");
        assert_eq!(a.resource_type.as_deref(), Some("tag"));
        assert_eq!(a.resource_uuid.as_deref(), Some("t1"));
        assert_eq!(a.description, "prod Created");
        assert_eq!(a.actor_type, "api_token");
    }

    #[test]
    fn denial_events_are_deduplicated_per_token() {
        let c = ctx();
        let context = json!({"reason": "r-dedup", "required_abilities": ["write"]});
        let m = context.as_object().unwrap();
        let w = Duration::from_secs(300);
        assert!(first_in_window("api.auth.ability_denied", &c, m, w));
        assert!(!first_in_window("api.auth.ability_denied", &c, m, w));
    }
}
