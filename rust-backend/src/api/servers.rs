//! Native `/api/v1` handlers: servers (port of `ServersController::servers` / `server_by_uuid`).

#[allow(unused_imports)]
use super::prelude::*;

use base64::{
    engine::{general_purpose::GeneralPurpose, DecodePaddingMode, GeneralPurposeConfig},
    Engine,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Flat server row for the SPA dashboard aggregate (`/api/v1/dashboard`), not a Laravel shape.
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct ServerRow {
    uuid: String,
    name: String,
    description: Option<String>,
    ip: String,
    port: i32,
    #[sqlx(rename = "user")]
    user: String,
    is_reachable: Option<bool>,
    is_usable: Option<bool>,
}

pub(crate) const SERVER_SELECT: &str = r#"SELECT s.uuid, s.name, s.description, s.ip, s.port, s."user",
    ss.is_reachable, ss.is_usable
    FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id"#;

/// `Server` model: `$hidden` plus the controller's `makeHidden(['id'])`; SoftDeletes `deleted_at`.
const SERVER_SPEC: ModelSpec = ModelSpec {
    hidden: &[
        "id",
        "logdrain_axiom_api_key",
        "logdrain_newrelic_license_key",
        "proxy_error_page",
    ],
    datetimes: &["deleted_at", "sentinel_waiting_since"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// `ServerSetting` model `$hidden` that no token sees.
const SETTINGS_ALWAYS_HIDDEN: &[&str] = &["is_build_server", "geoip_maxmind_license_key"];
/// `ServerSetting` `$hidden` that the controller makes visible for `read:sensitive` tokens.
const SETTINGS_SENSITIVE: &[&str] = &[
    "sentinel_token",
    "sentinel_custom_url",
    "logdrain_newrelic_license_key",
    "logdrain_axiom_api_key",
    "logdrain_custom_config",
    "logdrain_custom_config_parser",
];
/// Columns with the `encrypted` cast on `ServerSetting`.
const SETTINGS_ENCRYPTED: &[&str] = &["sentinel_token", "logdrain_custom_config"];
const SETTINGS_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &[
        "docker_version_checked_at",
        "compose_version_checked_at",
        "patch_checked_at",
        "ssh_host_key_trusted_at",
    ],
    dates: &["image_candidate_scans_date"],
    json_strings: &[],
    no_timestamps: false,
};

const UNREACHABLE_NOTIFICATION: &str = r"App\Notifications\Server\Unreachable";

/// GET /api/v1/servers
pub async fn list_servers(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    // The controller selects only these columns; settings come from the relation.
    let rows = sqlx::query_scalar::<_, Value>(
        r#"SELECT jsonb_build_object('id', s.id, 'uuid', s.uuid, 'name', s.name, 'description', s.description,
                  'ip', s.ip, 'user', s."user", 'port', s.port, 'settings', to_jsonb(ss))
           FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id
           WHERE s.team_id = $1 AND s.deleted_at IS NULL ORDER BY s.id"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await;
    let rows = match rows {
        Ok(r) => r,
        Err(e) => return query_err("list_servers", e),
    };
    let sensitive = ctx.can_read_sensitive();
    let out: Vec<Value> = rows
        .into_iter()
        .map(|mut row| {
            let settings = row
                .as_object_mut()
                .and_then(|m| m.remove("settings"))
                .unwrap_or(Value::Null);
            let mut server = serialize_server(row, None, sensitive);
            if let Value::Object(m) = &mut server {
                for key in ["is_reachable", "is_usable"] {
                    m.insert(
                        key.into(),
                        settings.get(key).cloned().unwrap_or(Value::Null),
                    );
                }
                m.insert(
                    "settings".into(),
                    serialize_settings(settings, sensitive, state.app_key.as_ref()),
                );
            }
            server
        })
        .collect();
    ok_json(out)
}

/// GET /api/v1/servers/:uuid
pub async fn get_server(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: axum::extract::Request,
) -> Response {
    // ponytail: `?resources=` (definedResources across apps/dbs/services) is not ported yet; Laravel answers it.
    let wants_resources = req
        .uri()
        .query()
        .is_some_and(|q| q.split('&').any(|p| p.starts_with("resources=")));
    if wants_resources {
        return crate::proxy(State(state), req).await;
    }
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let row = sqlx::query_as::<_, (Value, Option<Value>, bool)>(
        r#"SELECT to_jsonb(s), to_jsonb(ss),
                  EXISTS (SELECT 1 FROM notification_throttles nt
                          WHERE nt.notifiable_type = 'App\Models\Server' AND nt.notifiable_id = s.id
                            AND nt.notification = $3)
           FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id
           WHERE s.team_id = $1 AND s.uuid = $2 AND s.deleted_at IS NULL LIMIT 1"#,
    )
    .bind(team_id)
    .bind(&uuid)
    .bind(UNREACHABLE_NOTIFICATION)
    .fetch_optional(pool)
    .await;
    let (server, settings, unreachable_sent) = match row {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Server not found."),
        Err(e) => return query_err("get_server", e),
    };
    let sensitive = ctx.can_read_sensitive();
    let mut out = serialize_server(server, Some(unreachable_sent), sensitive);
    if let Value::Object(m) = &mut out {
        m.insert(
            "settings".into(),
            serialize_settings(
                settings.unwrap_or(Value::Null),
                sensitive,
                state.app_key.as_ref(),
            ),
        );
    }
    ok_json(out)
}

/// `Server` -> `removeSensitiveData()` -> `serializeApiResponse()` -> `removeServerProxySecretsFromApiPayload()`.
fn serialize_server(
    row: Value,
    unreachable_notification_sent: Option<bool>,
    sensitive: bool,
) -> Value {
    let id = row.get("id").and_then(Value::as_i64);
    let mut out = to_laravel(row, &SERVER_SPEC);
    let Value::Object(m) = &mut out else {
        return out;
    };

    // Accessors (`ip()`, `user()`) sanitize the stored values.
    if let Some(Value::String(ip)) = m.get_mut("ip") {
        ip.retain(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '%' | '-'));
    }
    if let Some(Value::String(user)) = m.get_mut("user") {
        user.retain(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    }
    m.insert("is_tinyhelm_host".into(), Value::Bool(id == Some(0)));
    if let Some(sent) = unreachable_notification_sent {
        m.insert("unreachable_notification_sent".into(), Value::Bool(sent));
    }

    // `retrieved` hook: proxy.redirect_enabled defaults to true; an unselected/null proxy is `{}` plus that.
    let mut proxy = match m.remove("proxy") {
        Some(Value::Object(p)) => p,
        _ => Map::new(),
    };
    if proxy.get("redirect_enabled").map_or(true, Value::is_null) {
        proxy.insert("redirect_enabled".into(), Value::Bool(true));
    }
    if !sensitive {
        // The saved proxy configuration often holds DNS provider tokens; validation logs leak host details.
        proxy.remove("last_saved_proxy_configuration");
        m.remove("validation_logs");
    }
    m.insert("proxy".into(), Value::Object(proxy));
    out
}

/// `removeSensitiveDataFromSettings()`: hidden fields, decrypted secrets for `read:sensitive`,
/// the effective build role and SSH host key fingerprints.
fn serialize_settings(row: Value, sensitive: bool, app_key: Option<&[u8; 32]>) -> Value {
    if !row.is_object() {
        return Value::Null;
    }
    // `effectiveServerRole() === BUILD`: server_role ?? (is_build_server ? build : both).
    let is_build = match row.get("server_role").and_then(Value::as_str) {
        Some(role) => role == "build",
        None => row
            .get("is_build_server")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let fingerprints = |key: &str| {
        Value::from(ssh_host_key_fingerprints(
            row.get(key).and_then(Value::as_str),
        ))
    };
    let (current, pending) = (
        fingerprints("ssh_host_key"),
        fingerprints("ssh_host_key_pending"),
    );

    let mut out = to_laravel(row, &SETTINGS_SPEC);
    let Value::Object(m) = &mut out else {
        return out;
    };
    for k in SETTINGS_ALWAYS_HIDDEN {
        m.remove(*k);
    }
    for k in SETTINGS_SENSITIVE {
        if !sensitive {
            m.remove(*k);
        } else if SETTINGS_ENCRYPTED.contains(k) {
            // ponytail: an undecryptable value is a 500 in Laravel; here it serializes as null.
            if let Some(v) = m.get_mut(*k) {
                *v = match (v.as_str(), app_key) {
                    (Some(enc), Some(key)) => crate::crypto::decrypt(enc, key)
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                    _ => Value::Null,
                };
            }
        }
    }
    m.insert("is_build_server".into(), Value::Bool(is_build));
    m.insert("ssh_host_key_fingerprints".into(), current);
    m.insert("ssh_host_key_pending_fingerprints".into(), pending);
    out
}

/// `ServerSetting::sshHostKeyFingerprints`: "SHA256:<unpadded b64 sha256 of the key blob> (<type>)" per line.
fn ssh_host_key_fingerprints(keys: Option<&str>) -> Vec<String> {
    // PHP's non-strict base64_decode: skip characters outside the alphabet, accept missing padding.
    const LENIENT: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    keys.unwrap_or_default()
        .split('\n')
        .filter(|line| !line.is_empty() && *line != "0")
        .map(|line| {
            let (kind, blob) = line.split_once(' ').unwrap_or((line, ""));
            let clean: String = blob
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/'))
                .collect();
            let decoded = LENIENT.decode(&clean).unwrap_or_default();
            let digest =
                base64::engine::general_purpose::STANDARD_NO_PAD.encode(Sha256::digest(decoded));
            format!("SHA256:{digest} ({kind})")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_shape_and_line_split() {
        let fps = ssh_host_key_fingerprints(Some(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA\n\nssh-rsa AAAAB3",
        ));
        assert_eq!(fps.len(), 2);
        assert!(fps[0].starts_with("SHA256:") && fps[0].ends_with(" (ssh-ed25519)"));
        assert!(!fps[0].contains('='));
        assert!(fps[1].ends_with(" (ssh-rsa)"));
        assert!(ssh_host_key_fingerprints(None).is_empty());
    }

    #[test]
    fn server_secrets_follow_sensitivity() {
        let row = json!({"id": 3, "ip": "1.2.3.4\n", "user": "root;", "validation_logs": "x",
                         "proxy": {"last_saved_proxy_configuration": "t"}});
        let public = serialize_server(row.clone(), Some(false), false);
        assert!(public.get("id").is_none());
        assert!(public.get("validation_logs").is_none());
        assert_eq!(public["proxy"], json!({"redirect_enabled": true}));
        assert_eq!(public["ip"], "1.2.3.4");
        assert_eq!(public["user"], "root");
        assert_eq!(public["is_tinyhelm_host"], false);
        let full = serialize_server(row, None, true);
        assert_eq!(full["validation_logs"], "x");
        assert_eq!(full["proxy"]["last_saved_proxy_configuration"], "t");
    }

    #[test]
    fn settings_hide_secrets_and_derive_role() {
        let row = json!({"server_role": null, "is_build_server": true, "sentinel_token": "enc",
                         "geoip_maxmind_license_key": "k", "ssh_host_key": null, "ssh_host_key_pending": null});
        let public = serialize_settings(row.clone(), false, None);
        assert!(public.get("sentinel_token").is_none());
        assert!(public.get("geoip_maxmind_license_key").is_none());
        assert_eq!(public["is_build_server"], true);
        assert_eq!(public["ssh_host_key_fingerprints"], json!([]));
        let full = serialize_settings(row, true, None);
        assert!(full.get("sentinel_token").is_some());
    }
}
