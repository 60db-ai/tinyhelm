//! Native `/api/v1` handlers: applications (`ApplicationsController@applications` / `@application_by_uuid`).
//!
//! Output matches Laravel's `removeSensitiveData()` + `serializeApiResponse()` for the callers the native
//! guard admits (admin/owner, full-team token): Eloquent attributes minus `$hidden`, casts and accessors
//! (`status`, `custom_network_aliases`, `custom_nginx_configuration`), the `server_status` append, the
//! relations its accessors load (`destination.server.settings`, `additional_servers`), `withCount` columns,
//! and the secrets a `read:sensitive`/`root` token may see.

use axum::extract::Request;
use serde_json::{Map, Value};

#[allow(unused_imports)]
use super::prelude::*;

/// `Application::COMMAND_AND_CONFIGURATION_ATTRIBUTES`.
const COMMAND_AND_CONFIGURATION: &[&str] = &[
    "install_command",
    "build_command",
    "start_command",
    "pre_deployment_command",
    "post_deployment_command",
    "health_check_command",
    "health_check_response_text",
    "custom_docker_run_options",
    "docker_compose_custom_start_command",
    "docker_compose_custom_build_command",
    "custom_nginx_configuration",
];

/// `Application::$hidden` (besides the command/configuration attributes) plus the controller's `makeHidden`.
const APP_HIDDEN: &[&str] = &[
    "http_basic_auth_password",
    "git_https_password",
    "manual_webhook_secret_github",
    "manual_webhook_secret_gitlab",
    "manual_webhook_secret_bitbucket",
    "manual_webhook_secret_gitea",
    "dockerfile",
    "docker_compose",
    "docker_compose_raw",
    "custom_labels",
    "domain_dns_statuses",
    "domain_port_overrides",
    "id",
    "resourceable",
    "resourceable_id",
    "resourceable_type",
];

/// Made visible by `removeSensitiveData()` for tokens that can read resource secrets.
const APP_SENSITIVE_VISIBLE: &[&str] = &[
    "custom_labels",
    "dockerfile",
    "docker_compose",
    "docker_compose_raw",
    "manual_webhook_secret_bitbucket",
    "manual_webhook_secret_gitea",
    "manual_webhook_secret_github",
    "manual_webhook_secret_gitlab",
    "http_basic_auth_password",
];

/// `encrypted` casts among the visible-for-sensitive attributes.
const APP_ENCRYPTED: &[&str] = &[
    "http_basic_auth_password",
    "manual_webhook_secret_github",
    "manual_webhook_secret_gitlab",
    "manual_webhook_secret_bitbucket",
    "manual_webhook_secret_gitea",
];

const APP_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &["last_restart_at", "deleted_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

const SERVER_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &["sentinel_waiting_since", "deleted_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

const SERVER_SETTINGS_SPEC: ModelSpec = ModelSpec {
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

const PLAIN_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

const SERVER_HIDDEN: &[&str] = &[
    "logdrain_axiom_api_key",
    "logdrain_newrelic_license_key",
    "proxy_error_page",
];
const SERVER_EXPOSED: &[&str] = &["logdrain_axiom_api_key", "logdrain_newrelic_license_key"];
const SETTINGS_HIDDEN: &[&str] = &[
    "is_build_server",
    "sentinel_token",
    "sentinel_custom_url",
    "logdrain_newrelic_license_key",
    "logdrain_axiom_api_key",
    "logdrain_custom_config",
    "logdrain_custom_config_parser",
    "geoip_maxmind_license_key",
];
const SETTINGS_EXPOSED: &[&str] = &[
    "sentinel_token",
    "sentinel_custom_url",
    "logdrain_newrelic_license_key",
    "logdrain_axiom_api_key",
    "logdrain_custom_config",
    "logdrain_custom_config_parser",
];
const SETTINGS_ENCRYPTED: &[&str] = &[
    "sentinel_token",
    "geoip_maxmind_license_key",
    "logdrain_custom_config",
];
const SERVER_ENCRYPTED: &[&str] = &["logdrain_axiom_api_key", "logdrain_newrelic_license_key"];
const PLACEHOLDER_IPS: &[&str] = &["1.2.3.4", "0.0.0.0", "::"];

const APP_SELECT: &str = r#"SELECT to_jsonb(a) || jsonb_build_object(
        'additional_servers_count', (SELECT count(*) FROM additional_destinations ad
            JOIN servers s ON s.id = ad.server_id AND s.deleted_at IS NULL WHERE ad.application_id = a.id),
        'additional_networks_count', (SELECT count(*) FROM additional_destinations ad
            JOIN standalone_dockers sd ON sd.id = ad.standalone_docker_id WHERE ad.application_id = a.id))
    FROM applications a
    JOIN environments e ON e.id = a.environment_id
    JOIN projects p ON p.id = e.project_id
    WHERE p.team_id = $1 AND a.deleted_at IS NULL AND ($2::text IS NULL OR a.uuid = $2)
    ORDER BY a.name"#;

/// What the token may see (`api.sensitive`). Native callers are admins of the whole team, so
/// resource secrets and server secrets go together.
#[derive(Clone, Copy)]
struct Access<'a> {
    sensitive: bool,
    app_key: Option<&'a [u8; 32]>,
}

/// GET /api/v1/applications[?tag=name]
pub async fn list_applications(State(state): State<crate::AppState>, req: Request) -> Response {
    let Some(ctx) = req.extensions().get::<Ctx>().cloned() else {
        return crate::proxy(State(state), req).await;
    };
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let tag = req.uri().query().and_then(|q| url_query_value(q, "tag"));
    let rows = match sqlx::query_scalar::<_, Value>(APP_SELECT)
        .bind(team_id)
        .bind(None::<String>)
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_applications", e),
    };
    let access = Access {
        sensitive: ctx.can_read_sensitive(),
        app_key: state.app_key.as_ref(),
    };
    if access.sensitive && rows.iter().any(has_generated_compose) {
        // ponytail: shown-once value redaction in the generated compose stays in Laravel
        // (redactLockedValuesInGeneratedCompose); port it with the env-var lock logic.
        return crate::proxy(State(state), req).await;
    }

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(tag) = tag.as_deref() {
            let app_id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
            match has_tag(pool, app_id, tag).await {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => return query_err("list_applications tags", e),
            }
        }
        match serialize_application(pool, row, access, false).await {
            Ok(v) => out.push(v),
            Err(e) => return query_err("list_applications relations", e),
        }
    }
    ok_json(out)
}

/// GET /api/v1/applications/:uuid
pub async fn get_application(
    State(state): State<crate::AppState>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let Some(ctx) = req.extensions().get::<Ctx>().cloned() else {
        return crate::proxy(State(state), req).await;
    };
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let row = match sqlx::query_scalar::<_, Value>(APP_SELECT)
        .bind(team_id)
        .bind(&uuid)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Application not found."),
        Err(e) => return query_err("get_application", e),
    };
    let access = Access {
        sensitive: ctx.can_read_sensitive(),
        app_key: state.app_key.as_ref(),
    };
    if access.sensitive && has_generated_compose(&row) {
        // ponytail: see list_applications.
        return crate::proxy(State(state), req).await;
    }
    match serialize_application(pool, row, access, true).await {
        Ok(v) => ok_json(v),
        Err(e) => query_err("get_application relations", e),
    }
}

fn has_generated_compose(row: &Value) -> bool {
    row.get("docker_compose")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
}

/// Value of one `key=value` pair in a query string (percent-decoded, `+` as space).
fn url_query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == key).then(|| percent_decode(&v.replace('+', " ")))
    })
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn has_tag(pool: &PgPool, app_id: i64, tag: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r"SELECT EXISTS (SELECT 1 FROM taggables tg JOIN tags t ON t.id = tg.tag_id
           WHERE tg.taggable_type = 'App\Models\Application' AND tg.taggable_id = $1 AND t.name = $2)",
    )
    .bind(app_id)
    .bind(tag)
    .fetch_one(pool)
    .await
}

/// One application, as `removeSensitiveData()` returns it.
async fn serialize_application(
    pool: &PgPool,
    row: Value,
    access: Access<'_>,
    with_settings: bool,
) -> Result<Value, sqlx::Error> {
    let raw = match row {
        Value::Object(m) => m,
        other => return Ok(other),
    };
    let app_id = raw.get("id").and_then(Value::as_i64).unwrap_or_default();

    // Relations loaded by the `status` / `server_status` accessors during serialization.
    let destination = load_destination(pool, &raw).await?;
    let main_functional = destination
        .as_ref()
        .and_then(|d| d.server.as_ref())
        .is_some_and(|s| s.functional);
    let additional = load_additional_servers(pool, app_id, main_functional).await?;
    let server_status = main_functional && additional.iter().all(|s| s.functional);

    let mut hidden: Vec<&str> = COMMAND_AND_CONFIGURATION
        .iter()
        .chain(APP_HIDDEN)
        .copied()
        .collect();
    if access.sensitive {
        hidden.retain(|k| {
            !COMMAND_AND_CONFIGURATION.contains(k) && !APP_SENSITIVE_VISIBLE.contains(k)
        });
    } else {
        hidden.push("private_key_id");
    }

    let mut app = match to_laravel(Value::Object(raw), &APP_SPEC) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    for k in &hidden {
        app.remove(*k);
    }
    for k in APP_ENCRYPTED {
        if let Some(v) = app.get_mut(*k) {
            *v = decrypt_value(v, access.app_key);
        }
    }
    if let Some(v) = app.get_mut("custom_nginx_configuration") {
        if let Some(s) = v.as_str() {
            *v = base64_decode_lossy(s)
                .map(Value::String)
                .unwrap_or(Value::Null);
        }
    }
    if let Some(v) = app.get_mut("custom_network_aliases") {
        *v = custom_network_aliases(v);
    }
    if let Some(v) = app.get_mut("status") {
        let pivots: Vec<Option<String>> =
            additional.iter().map(|s| s.pivot_status.clone()).collect();
        *v = Value::String(status_accessor(v.as_str().unwrap_or(""), &pivots));
    }

    app.insert("server_status".into(), Value::Bool(server_status));
    app.insert(
        "destination".into(),
        destination
            .map(|d| d.into_json(access, true))
            .unwrap_or(Value::Null),
    );
    app.insert(
        "additional_servers".into(),
        Value::Array(
            additional
                .into_iter()
                .map(|s| s.into_json(access, false))
                .collect(),
        ),
    );
    if with_settings {
        app.insert("settings".into(), load_settings(pool, app_id).await?);
    }

    let mut out = Value::Object(app);
    if !access.sensitive {
        remove_server_proxy_secrets(&mut out);
    }
    Ok(out)
}

/// `Application::status` getter.
fn status_accessor(value: &str, additional_statuses: &[Option<String>]) -> String {
    if additional_statuses.is_empty() {
        let (status, health) = if value.contains('(') {
            let status = value.split('(').next().unwrap_or("").trim();
            let health = value
                .split_once('(')
                .map(|(_, r)| r.split(')').next().unwrap_or("").trim())
                .unwrap_or("");
            (status.to_string(), health.to_string())
        } else if let Some((s, h)) = value.split_once(':') {
            (s.trim().to_string(), h.trim().to_string())
        } else {
            (value.to_string(), "unhealthy".to_string())
        };
        return format!("{status}:{health}");
    }
    let main_status = value.split(':').next().unwrap_or("").to_string();
    let main_health = value
        .split_once(':')
        .map(|(_, h)| h)
        .unwrap_or(value)
        .to_string();
    let (mut status, mut health) = (main_status.clone(), main_health.clone());
    for pivot in additional_statuses {
        let pivot = pivot.as_deref().unwrap_or("");
        let server_status = pivot.split(':').next().unwrap_or("");
        let server_health = pivot.split_once(':').map(|(_, h)| h);
        if main_status != server_status {
            status = "degraded".into();
        }
        if server_health.is_some_and(|h| h != main_health) {
            health = "unhealthy".into();
        }
    }
    format!("{status}:{health}")
}

/// `Application::customNetworkAliases` getter: a JSON array string becomes a comma-separated string.
fn custom_network_aliases(v: &Value) -> Value {
    match v.as_str() {
        Some(s) => match serde_json::from_str::<Value>(s) {
            Ok(Value::Array(items)) => Value::String(
                items
                    .iter()
                    .map(|i| {
                        i.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| i.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            _ => Value::String(s.to_string()),
        },
        None => v.clone(),
    }
}

fn base64_decode_lossy(s: &str) -> Option<String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn decrypt_value(v: &Value, key: Option<&[u8; 32]>) -> Value {
    match (v.as_str(), key) {
        (Some(s), Some(k)) => crate::crypto::decrypt(s, k)
            .map(Value::String)
            .unwrap_or(Value::Null),
        _ => v.clone(),
    }
}

/// `removeServerProxySecrets`: drop the saved proxy configuration and validation logs of every
/// serialized server in the payload.
fn remove_server_proxy_secrets(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if let Some(Value::Object(proxy)) = map.get_mut("proxy") {
                proxy.remove("last_saved_proxy_configuration");
                map.remove("validation_logs");
            }
            for item in map.values_mut() {
                remove_server_proxy_secrets(item);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(remove_server_proxy_secrets),
        _ => {}
    }
}

async fn load_settings(pool: &PgPool, app_id: i64) -> Result<Value, sqlx::Error> {
    let row: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM application_settings s WHERE s.application_id = $1 LIMIT 1",
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;
    Ok(match row.map(|r| to_laravel(r, &PLAIN_SPEC)) {
        Some(Value::Object(mut m)) => {
            for k in ["id", "application_id", "created_at", "updated_at"] {
                m.remove(k);
            }
            Value::Object(m)
        }
        _ => Value::Null,
    })
}

/// A server as serialized inside an application payload (with its `settings` when loaded).
struct ServerJson {
    row: Map<String, Value>,
    settings: Option<Map<String, Value>>,
    functional: bool,
    pivot: Option<Value>,
    pivot_status: Option<String>,
}

impl ServerJson {
    /// `expose`: `exposeNestedServerSecrets()` applies only to the destination server.
    fn into_json(self, access: Access<'_>, expose: bool) -> Value {
        let reveal = access.sensitive && expose;
        let id = self.row.get("id").and_then(Value::as_i64);
        let mut server = match to_laravel(Value::Object(self.row), &SERVER_SPEC) {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        for k in SERVER_HIDDEN {
            if !(reveal && SERVER_EXPOSED.contains(k)) {
                server.remove(*k);
            }
        }
        for k in SERVER_ENCRYPTED {
            if let Some(v) = server.get_mut(*k) {
                *v = decrypt_value(v, access.app_key);
            }
        }
        // SchemalessAttributes cast: a missing proxy serializes as an empty array.
        if server.get("proxy").is_some_and(Value::is_null) {
            server.insert("proxy".into(), Value::Array(Vec::new()));
        }
        server.insert("is_tinyhelm_host".into(), Value::Bool(id == Some(0)));
        if let Some(settings) = self.settings {
            let mut s = match to_laravel(Value::Object(settings), &SERVER_SETTINGS_SPEC) {
                Value::Object(m) => m,
                _ => Map::new(),
            };
            for k in SETTINGS_HIDDEN {
                if !(reveal && SETTINGS_EXPOSED.contains(k)) {
                    s.remove(*k);
                }
            }
            for k in SETTINGS_ENCRYPTED {
                if let Some(v) = s.get_mut(*k) {
                    *v = decrypt_value(v, access.app_key);
                }
            }
            server.insert("settings".into(), Value::Object(s));
        }
        if let Some(pivot) = self.pivot {
            server.insert("pivot".into(), pivot);
        }
        Value::Object(server)
    }
}

/// `Server::isFunctional()`.
fn is_functional(server: &Map<String, Value>, settings: &Map<String, Value>) -> bool {
    let truthy = |k: &str| settings.get(k).is_some_and(|v| v.as_bool() == Some(true));
    let ip = server.get("ip").and_then(Value::as_str).unwrap_or("");
    truthy("is_reachable")
        && truthy("is_usable")
        && settings.get("force_disabled").and_then(Value::as_bool) == Some(false)
        && !PLACEHOLDER_IPS.contains(&ip)
}

async fn server_with_settings(
    pool: &PgPool,
    server: Map<String, Value>,
) -> Result<ServerJson, sqlx::Error> {
    let id = server.get("id").and_then(Value::as_i64).unwrap_or_default();
    let settings: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(ss) FROM server_settings ss WHERE ss.server_id = $1 LIMIT 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let settings = match settings {
        Some(Value::Object(m)) => Some(m),
        _ => None,
    };
    let functional = settings.as_ref().is_some_and(|s| is_functional(&server, s));
    Ok(ServerJson {
        row: server,
        settings,
        functional,
        pivot: None,
        pivot_status: None,
    })
}

struct DestinationJson {
    row: Map<String, Value>,
    server: Option<ServerJson>,
}

impl DestinationJson {
    fn into_json(self, access: Access<'_>, expose: bool) -> Value {
        let mut d = match to_laravel(Value::Object(self.row), &PLAIN_SPEC) {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        d.insert(
            "server".into(),
            self.server
                .map(|s| s.into_json(access, expose))
                .unwrap_or(Value::Null),
        );
        Value::Object(d)
    }
}

/// The polymorphic `destination` (StandaloneDocker / SwarmDocker) with `server.settings`, as loaded by
/// the `server_status` accessor.
async fn load_destination(
    pool: &PgPool,
    app: &Map<String, Value>,
) -> Result<Option<DestinationJson>, sqlx::Error> {
    let table = match app.get("destination_type").and_then(Value::as_str) {
        Some(r"App\Models\StandaloneDocker") => "standalone_dockers",
        Some(r"App\Models\SwarmDocker") => "swarm_dockers",
        _ => return Ok(None),
    };
    let Some(id) = app.get("destination_id").and_then(Value::as_i64) else {
        return Ok(None);
    };
    let row: Option<Value> = sqlx::query_scalar(&format!(
        "SELECT to_jsonb(d) FROM {table} d WHERE d.id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some(Value::Object(row)) = row else {
        return Ok(None);
    };
    let server = match row.get("server_id").and_then(Value::as_i64) {
        Some(sid) => {
            let s: Option<Value> = sqlx::query_scalar(
                "SELECT to_jsonb(s) FROM servers s WHERE s.id = $1 AND s.deleted_at IS NULL",
            )
            .bind(sid)
            .fetch_optional(pool)
            .await?;
            match s {
                Some(Value::Object(m)) => Some(server_with_settings(pool, m).await?),
                _ => None,
            }
        }
        None => None,
    };
    Ok(Some(DestinationJson { row, server }))
}

/// `additional_servers` with their pivot. `server_status` loads each server's settings while it checks
/// them, stopping at the first non-functional one, and only when the main server is functional.
async fn load_additional_servers(
    pool: &PgPool,
    app_id: i64,
    main_functional: bool,
) -> Result<Vec<ServerJson>, sqlx::Error> {
    let rows: Vec<(Value, Value)> = sqlx::query_as(
        "SELECT to_jsonb(s), jsonb_build_object('application_id', ad.application_id, 'server_id', ad.server_id, \
         'standalone_docker_id', ad.standalone_docker_id, 'status', ad.status) \
         FROM additional_destinations ad JOIN servers s ON s.id = ad.server_id AND s.deleted_at IS NULL \
         WHERE ad.application_id = $1 ORDER BY ad.id",
    )
    .bind(app_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    let mut checking = main_functional;
    for (server, pivot) in rows {
        let Value::Object(server) = server else {
            continue;
        };
        let pivot_status = pivot
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut s = if checking {
            let s = server_with_settings(pool, server).await?;
            checking = s.functional;
            s
        } else {
            ServerJson {
                row: server,
                settings: None,
                functional: false,
                pivot: None,
                pivot_status: None,
            }
        };
        s.pivot = Some(pivot);
        s.pivot_status = pivot_status;
        out.push(s);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_getter_matches_laravel() {
        assert_eq!(status_accessor("exited", &[]), "exited:unhealthy");
        assert_eq!(status_accessor("running (healthy)", &[]), "running:healthy");
        assert_eq!(status_accessor("running:healthy", &[]), "running:healthy");
        assert_eq!(
            status_accessor("running:healthy", &[Some("exited:unhealthy".into())]),
            "degraded:unhealthy"
        );
        assert_eq!(
            status_accessor("running:healthy", &[Some("running".into())]),
            "running:healthy"
        );
    }

    #[test]
    fn network_aliases_join_json_arrays() {
        assert_eq!(
            custom_network_aliases(&json!("[\"a\",\"b\"]")),
            json!("a,b")
        );
        assert_eq!(custom_network_aliases(&json!("a,b")), json!("a,b"));
        assert_eq!(custom_network_aliases(&Value::Null), Value::Null);
    }

    #[test]
    fn proxy_secrets_are_removed_recursively() {
        let mut v = json!({"destination": {"server": {"proxy": {"type": "traefik", "last_saved_proxy_configuration": "x"}, "validation_logs": "l"}}});
        remove_server_proxy_secrets(&mut v);
        assert_eq!(
            v["destination"]["server"]["proxy"],
            json!({"type": "traefik"})
        );
        assert!(v["destination"]["server"].get("validation_logs").is_none());
    }

    #[test]
    fn query_value_decodes() {
        assert_eq!(
            url_query_value("tag=my%20tag&x=1", "tag").as_deref(),
            Some("my tag")
        );
        assert_eq!(url_query_value("x=1", "tag"), None);
    }
}
