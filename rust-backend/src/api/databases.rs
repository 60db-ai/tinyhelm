//! Native `/api/v1` handlers: databases (`DatabasesController::databases` / `database_by_uuid`).
//!
//! Output matches Laravel's Eloquent serialization of the nine `Standalone*` models: model `$hidden`,
//! casts, the `database_type` / `server_status` appends, the connection-URL appends for tokens that can
//! read secrets, and the `destination` relation (with its server and server settings) that the
//! `server_status` accessor loads. Cases whose output depends on PHP-only state are answered by Laravel
//! (see [`Fail::Delegate`]).

#[allow(unused_imports)]
use super::prelude::*;
use axum::extract::Request;
use serde_json::{Map, Value};

/// Why a native handler cannot answer.
pub(crate) enum Fail {
    /// Laravel must answer: the output depends on state not ported yet.
    Delegate,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for Fail {
    fn from(e: sqlx::Error) -> Self {
        Fail::Db(e)
    }
}

/// Map a [`Fail`] to the response: the Laravel answer for `Delegate`, a 502 for a query error.
pub(crate) async fn fail_response(
    state: crate::AppState,
    req: Request,
    fail: Fail,
    what: &str,
) -> Response {
    match fail {
        Fail::Delegate => crate::proxy(State(state), req).await,
        Fail::Db(e) => query_err(what, e),
    }
}

pub(crate) struct Engine {
    pub table: &'static str,
    /// `type()` of the model (`database_type` append).
    pub kind: &'static str,
    /// Model `$hidden`.
    pub hidden: &'static [&'static str],
    /// Columns with the `encrypted` cast.
    pub encrypted: &'static [&'static str],
    /// The model has the internal/external connection URL appends.
    pub has_urls: bool,
    /// Connection URLs read a runtime environment variable (Redis family): Laravel answers secret reads.
    pub env_password: bool,
}

/// `STANDALONE_DATABASE_MODELS` order, which is also `Project::databases()` order.
pub(crate) const ENGINES: &[Engine] = &[
    Engine {
        table: "standalone_postgresqls",
        kind: "standalone-postgresql",
        hidden: &[
            "postgres_password",
            "init_scripts",
            "postgres_conf",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
        ],
        encrypted: &["postgres_password"],
        has_urls: true,
        env_password: false,
    },
    Engine {
        table: "standalone_redis",
        kind: "standalone-redis",
        hidden: &[
            "redis_password",
            "redis_conf",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
            "legacy_password_quoting",
        ],
        encrypted: &[],
        has_urls: true,
        env_password: true,
    },
    Engine {
        table: "standalone_mongodbs",
        kind: "standalone-mongodb",
        hidden: &[
            "mongo_initdb_root_password",
            "mongo_conf",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
        ],
        encrypted: &["mongo_initdb_root_password"],
        has_urls: true,
        env_password: false,
    },
    Engine {
        table: "standalone_mysqls",
        kind: "standalone-mysql",
        hidden: &[
            "mysql_password",
            "mysql_root_password",
            "mysql_conf",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
        ],
        encrypted: &["mysql_password", "mysql_root_password"],
        has_urls: true,
        env_password: false,
    },
    Engine {
        table: "standalone_mariadbs",
        kind: "standalone-mariadb",
        hidden: &[
            "mariadb_password",
            "mariadb_root_password",
            "mariadb_conf",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
        ],
        encrypted: &["mariadb_password", "mariadb_root_password"],
        has_urls: true,
        env_password: false,
    },
    Engine {
        table: "standalone_keydbs",
        kind: "standalone-keydb",
        hidden: &[
            "keydb_password",
            "keydb_conf",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
            "legacy_password_quoting",
        ],
        encrypted: &["keydb_password"],
        has_urls: true,
        env_password: true,
    },
    Engine {
        table: "standalone_dragonflies",
        kind: "standalone-dragonfly",
        hidden: &[
            "dragonfly_password",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
            "legacy_password_quoting",
        ],
        encrypted: &["dragonfly_password"],
        has_urls: true,
        env_password: true,
    },
    Engine {
        table: "standalone_clickhouses",
        kind: "standalone-clickhouse",
        hidden: &[
            "clickhouse_admin_password",
            "custom_docker_run_options",
            "internal_db_url",
            "external_db_url",
        ],
        encrypted: &["clickhouse_admin_password"],
        has_urls: true,
        env_password: false,
    },
    Engine {
        table: "standalone_sqlites",
        kind: "standalone-sqlite",
        hidden: &["custom_docker_run_options"],
        encrypted: &[],
        has_urls: false,
        env_password: false,
    },
];

/// `DatabasesController::removeSensitiveData` makes these visible for tokens that can read secrets.
const SENSITIVE_VISIBLE: &[&str] = &[
    "internal_db_url",
    "external_db_url",
    "init_scripts",
    "postgres_password",
    "dragonfly_password",
    "redis_password",
    "mongo_initdb_root_password",
    "keydb_password",
    "clickhouse_admin_password",
    "mysql_password",
    "mysql_root_password",
    "mariadb_password",
    "mariadb_root_password",
    "postgres_conf",
    "mysql_conf",
    "mariadb_conf",
    "mongo_conf",
    "redis_conf",
    "keydb_conf",
    "custom_docker_run_options",
];

const DATABASE_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &["last_restart_at", "deleted_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// Server and ServerSetting secrets that `exposeNestedServerSecrets` shows to server-secret readers.
const SERVER_SECRETS: &[&str] = &["logdrain_axiom_api_key", "logdrain_newrelic_license_key"];
const SERVER_HIDDEN: &[&str] = &[
    "logdrain_axiom_api_key",
    "logdrain_newrelic_license_key",
    "proxy_error_page",
];
const SERVER_ENCRYPTED: &[&str] = &["logdrain_axiom_api_key", "logdrain_newrelic_license_key"];
const SETTINGS_SECRETS: &[&str] = &[
    "sentinel_token",
    "sentinel_custom_url",
    "logdrain_newrelic_license_key",
    "logdrain_axiom_api_key",
    "logdrain_custom_config",
    "logdrain_custom_config_parser",
];
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
const SETTINGS_ENCRYPTED: &[&str] = &[
    "sentinel_token",
    "geoip_maxmind_license_key",
    "logdrain_custom_config",
];
const SERVER_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &["sentinel_waiting_since"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};
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
const PLACEHOLDER_IPS: &[&str] = &["1.2.3.4", "0.0.0.0", "::"];

/// Remove hidden keys; decrypt visible encrypted keys (a failed decrypt is a Laravel 500: delegate).
pub(crate) fn apply_visibility(
    map: &mut Map<String, Value>,
    hidden: &[&str],
    visible: &[&str],
    encrypted: &[&str],
    app_key: Option<&[u8; 32]>,
) -> Result<(), Fail> {
    for key in hidden {
        if !visible.contains(key) {
            map.remove(*key);
        }
    }
    for key in encrypted {
        if let Some(Value::String(cipher)) = map.get(*key) {
            let plain = app_key
                .and_then(|k| crate::crypto::decrypt(cipher, k))
                .ok_or(Fail::Delegate)?;
            map.insert((*key).to_string(), Value::String(plain));
        }
    }
    Ok(())
}

/// A server with its `settings` relation, serialized like `Server::toArray()` after `isFunctional()`.
/// Returns the JSON and whether the server is functional (`server_status`).
pub(crate) async fn server_json(
    pool: &PgPool,
    server_id: i64,
    expose_secrets: bool,
    app_key: Option<&[u8; 32]>,
) -> Result<Option<(Value, bool)>, Fail> {
    let Some(server) =
        sqlx::query_scalar::<_, Value>("SELECT to_jsonb(s) FROM servers s WHERE s.id = $1")
            .bind(server_id)
            .fetch_optional(pool)
            .await?
    else {
        return Ok(None);
    };
    let settings = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(ss) FROM server_settings ss WHERE ss.server_id = $1 LIMIT 1",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await?;

    let ip = server
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let functional = settings.as_ref().is_some_and(|s| {
        s.get("is_reachable") == Some(&Value::Bool(true))
            && s.get("is_usable") == Some(&Value::Bool(true))
            && s.get("force_disabled") == Some(&Value::Bool(false))
    }) && !ip.is_empty()
        && !PLACEHOLDER_IPS.contains(&ip.as_str());

    let Value::Object(mut server_map) = to_laravel(server, &SERVER_SPEC) else {
        return Ok(None);
    };
    let server_visible: &[&str] = if expose_secrets { SERVER_SECRETS } else { &[] };
    apply_visibility(
        &mut server_map,
        SERVER_HIDDEN,
        server_visible,
        SERVER_ENCRYPTED,
        app_key,
    )?;
    let is_host = server_map.get("id").and_then(Value::as_i64) == Some(0);
    server_map.insert("is_tinyhelm_host".into(), Value::Bool(is_host));

    let settings_json = match settings {
        Some(s) => {
            let Value::Object(mut m) = to_laravel(s, &SETTINGS_SPEC) else {
                return Ok(None);
            };
            let visible: &[&str] = if expose_secrets {
                SETTINGS_SECRETS
            } else {
                &[]
            };
            apply_visibility(
                &mut m,
                SETTINGS_HIDDEN,
                visible,
                SETTINGS_ENCRYPTED,
                app_key,
            )?;
            Value::Object(m)
        }
        None => Value::Null,
    };
    server_map.insert("settings".into(), settings_json);
    Ok(Some((Value::Object(server_map), functional)))
}

/// The `destination` morph relation (StandaloneDocker / SwarmDocker) with its `server`, plus `server_status`.
pub(crate) async fn destination_json(
    pool: &PgPool,
    row: &Map<String, Value>,
    expose_secrets: bool,
    app_key: Option<&[u8; 32]>,
) -> Result<(Value, bool), Fail> {
    let table = match row.get("destination_type").and_then(Value::as_str) {
        Some(r"App\Models\StandaloneDocker") => "standalone_dockers",
        Some(r"App\Models\SwarmDocker") => "swarm_dockers",
        _ => return Ok((Value::Null, false)),
    };
    let Some(id) = row.get("destination_id").and_then(Value::as_i64) else {
        return Ok((Value::Null, false));
    };
    let Some(dest) = sqlx::query_scalar::<_, Value>(&format!(
        "SELECT to_jsonb(d) FROM {table} d WHERE d.id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok((Value::Null, false));
    };
    let Value::Object(mut dest) = to_laravel(dest, &ModelSpec::default()) else {
        return Ok((Value::Null, false));
    };
    let (server, functional) = match dest.get("server_id").and_then(Value::as_i64) {
        Some(sid) => server_json(pool, sid, expose_secrets, app_key)
            .await?
            .unwrap_or((Value::Null, false)),
        None => (Value::Null, false),
    };
    dest.insert("server".into(), server);
    Ok((Value::Object(dest), functional))
}

/// PHP `rawurlencode` (RFC 3986 unreserved characters stay as they are).
pub(crate) fn rawurlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn text<'a>(m: &'a Map<String, Value>, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The `internal_db_url` accessor (the row already has decrypted passwords).
fn internal_url(kind: &str, m: &Map<String, Value>) -> Option<String> {
    let uuid = text(m, "uuid");
    let ssl = m.get("enable_ssl") == Some(&Value::Bool(true));
    let mode = text(m, "ssl_mode");
    let url = match kind {
        "standalone-postgresql" => {
            let mut url = format!(
                "postgres://{}:{}@{uuid}:5432/{}",
                rawurlencode(text(m, "postgres_user")),
                rawurlencode(text(m, "postgres_password")),
                text(m, "postgres_db")
            );
            if ssl {
                url.push_str(&format!("?sslmode={mode}"));
                if ["verify-ca", "verify-full"].contains(&mode) {
                    url.push_str("&sslrootcert=/etc/ssl/certs/tinyhelm-ca.crt");
                }
            }
            url
        }
        "standalone-mysql" => {
            let mut url = format!(
                "mysql://{}:{}@{uuid}:3306/{}",
                rawurlencode(text(m, "mysql_user")),
                rawurlencode(text(m, "mysql_password")),
                text(m, "mysql_database")
            );
            if ssl {
                url.push_str(&format!("?ssl-mode={mode}"));
                if ["VERIFY_CA", "VERIFY_IDENTITY"].contains(&mode) {
                    url.push_str("&ssl-ca=/etc/ssl/certs/tinyhelm-ca.crt");
                }
            }
            url
        }
        "standalone-mariadb" => format!(
            "mysql://{}:{}@{uuid}:3306/{}",
            rawurlencode(text(m, "mariadb_user")),
            rawurlencode(text(m, "mariadb_password")),
            text(m, "mariadb_database")
        ),
        "standalone-mongodb" => {
            let mut url = format!(
                "mongodb://{}:{}@{uuid}:27017/?directConnection=true",
                rawurlencode(text(m, "mongo_initdb_root_username")),
                rawurlencode(text(m, "mongo_initdb_root_password"))
            );
            if ssl {
                url.push_str("&tls=true&tlsCAFile=/etc/mongo/certs/ca.pem");
                if mode == "verify-full" {
                    url.push_str("&tlsCertificateKeyFile=/etc/mongo/certs/server.pem");
                }
            }
            url
        }
        "standalone-clickhouse" => {
            let db = m
                .get("clickhouse_db")
                .and_then(Value::as_str)
                .unwrap_or("default");
            format!(
                "clickhouse://{}:{}@{uuid}:9000/{db}",
                rawurlencode(text(m, "clickhouse_admin_user")),
                rawurlencode(text(m, "clickhouse_admin_password"))
            )
        }
        _ => return None,
    };
    Some(url)
}

/// The `status` accessor of every `Standalone*` model: "running (healthy)" / "running:healthy" / "exited"
/// become "status:health", with "unhealthy" as the default health.
pub(crate) fn database_status(raw: &str) -> String {
    let after = |s: &str, sep: char| {
        s.split_once(sep)
            .map_or(s.to_string(), |(_, r)| r.to_string())
    };
    let before = |s: &str, sep: char| {
        s.split_once(sep)
            .map_or(s.to_string(), |(l, _)| l.to_string())
    };
    let (status, health) = if raw.contains('(') {
        (
            before(raw, '(').trim().to_string(),
            before(&after(raw, '('), ')').trim().to_string(),
        )
    } else if raw.contains(':') {
        (
            before(raw, ':').trim().to_string(),
            after(raw, ':').trim().to_string(),
        )
    } else {
        (raw.to_string(), "unhealthy".to_string())
    };
    format!("{status}:{health}")
}

/// Serialize one database row like `removeSensitiveData` (+ `serializeApiResponse`).
async fn database_json(
    pool: &PgPool,
    engine: &Engine,
    row: Value,
    ctx: &Ctx,
    app_key: Option<&[u8; 32]>,
) -> Result<Value, Fail> {
    let sensitive = ctx.can_read_sensitive();
    let Value::Object(mut m) = to_laravel(row, &DATABASE_SPEC) else {
        return Err(Fail::Delegate);
    };
    if sensitive && engine.env_password {
        // ponytail: Redis-family connection URLs read (possibly locked or remote) env vars; Laravel answers.
        return Err(Fail::Delegate);
    }
    let public = m.get("is_public") == Some(&Value::Bool(true))
        && m.get("public_port").is_some_and(|p| !p.is_null());
    if sensitive && engine.has_urls && public {
        // ponytail: the external URL depends on isDev()/base_ip(); Laravel answers until those are ported.
        return Err(Fail::Delegate);
    }

    let visible: &[&str] = if sensitive { SENSITIVE_VISIBLE } else { &[] };
    apply_visibility(&mut m, engine.hidden, visible, engine.encrypted, app_key)?;
    if sensitive && engine.has_urls {
        m.insert(
            "internal_db_url".into(),
            internal_url(engine.kind, &m).map_or(Value::Null, Value::String),
        );
        m.insert("external_db_url".into(), Value::Null);
    }
    m.remove("id");
    let status = database_status(m.get("status").and_then(Value::as_str).unwrap_or(""));
    m.insert("status".into(), Value::String(status));
    m.insert("database_type".into(), Value::String(engine.kind.into()));

    let (destination, functional) = destination_json(pool, &m, sensitive, app_key).await?;
    m.insert("server_status".into(), Value::Bool(functional));
    m.insert("destination".into(), destination);

    let mut out = Value::Object(m);
    if !sensitive {
        remove_server_proxy_secrets(&mut out);
    }
    Ok(out)
}

/// `removeServerProxySecrets`: drop the saved proxy configuration and validation logs of every
/// serialized server in the payload (for tokens that cannot read server secrets).
pub(crate) fn remove_server_proxy_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(proxy)) = map.get_mut("proxy") {
                proxy.remove("last_saved_proxy_configuration");
                map.remove("validation_logs");
            }
            for v in map.values_mut() {
                remove_server_proxy_secrets(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(remove_server_proxy_secrets),
        _ => {}
    }
}

/// Team databases of one engine as `(project_id, row)` (soft-deleted rows excluded, like the model scope).
pub(crate) async fn engine_rows(
    pool: &PgPool,
    engine: &Engine,
    team_id: i64,
    uuid: Option<&str>,
) -> Result<Vec<(i64, Value)>, sqlx::Error> {
    let sql = format!(
        "SELECT p.id, to_jsonb(t) FROM {} t JOIN environments e ON e.id = t.environment_id \
         JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND t.deleted_at IS NULL AND ($2::text IS NULL OR t.uuid = $2) \
         ORDER BY p.id, t.id",
        engine.table
    );
    sqlx::query_as::<_, (i64, Value)>(&sql)
        .bind(team_id)
        .bind(uuid)
        .fetch_all(pool)
        .await
}

/// GET /api/v1/databases
pub async fn list_databases(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match list_json(pool, &ctx, team_id, state.app_key.as_ref()).await {
        Ok(v) => ok_json(v),
        Err(fail) => fail_response(state.clone(), req, fail, "list_databases").await,
    }
}

async fn list_json(
    pool: &PgPool,
    ctx: &Ctx,
    team_id: i64,
    app_key: Option<&[u8; 32]>,
) -> Result<Vec<Value>, Fail> {
    // Laravel loops projects, then engines (in ENGINES order), then rows.
    let mut tagged: Vec<(i64, usize, i64, &Engine, Value)> = Vec::new();
    for (rank, engine) in ENGINES.iter().enumerate() {
        for (project_id, row) in engine_rows(pool, engine, team_id, None).await? {
            let id = row.get("id").and_then(Value::as_i64).unwrap_or(0);
            tagged.push((project_id, rank, id, engine, row));
        }
    }
    tagged.sort_by_key(|(pid, rank, id, _, _)| (*pid, *rank, *id));

    // ponytail: backup configs (latest log, S3 storages, redacted messages) are serialized by Laravel.
    let ids: Vec<i64> = tagged.iter().map(|t| t.2).collect();
    let has_backups: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM scheduled_database_backups WHERE team_id = $1 AND database_id = ANY($2))")
            .bind(team_id)
            .bind(&ids)
            .fetch_one(pool)
            .await?;
    if has_backups {
        return Err(Fail::Delegate);
    }

    let mut out = Vec::with_capacity(tagged.len());
    for (_, _, _, engine, row) in tagged {
        let mut json = database_json(pool, engine, row, ctx, app_key).await?;
        if let Value::Object(m) = &mut json {
            m.insert("backup_configs".into(), Value::Array(Vec::new()));
        }
        out.push(json);
    }
    Ok(out)
}

/// GET /api/v1/databases/:uuid
pub async fn get_database(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match show_json(pool, &ctx, team_id, &uuid, state.app_key.as_ref()).await {
        Ok(Some(v)) => (StatusCode::OK, Json(v)).into_response(),
        Ok(None) => not_found("Database not found."),
        Err(fail) => fail_response(state.clone(), req, fail, "get_database").await,
    }
}

async fn show_json(
    pool: &PgPool,
    ctx: &Ctx,
    team_id: i64,
    uuid: &str,
    app_key: Option<&[u8; 32]>,
) -> Result<Option<Value>, Fail> {
    // `queryDatabaseByUuidWithinTeam`: first engine with the uuid; the instance database (id 0) is hidden.
    for engine in ENGINES {
        let Some((_, row)) = engine_rows(pool, engine, team_id, Some(uuid))
            .await?
            .into_iter()
            .next()
        else {
            continue;
        };
        if engine.table == "standalone_postgresqls"
            && row.get("id").and_then(Value::as_i64) == Some(0)
        {
            return Ok(None);
        }
        return database_json(pool, engine, row, ctx, app_key)
            .await
            .map(Some);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rawurlencode_matches_php() {
        assert_eq!(rawurlencode("p@ss w/rd~_.-"), "p%40ss%20w%2Frd~_.-");
    }

    #[test]
    fn postgres_internal_url() {
        let m = json!({"uuid": "u1", "postgres_user": "postgres", "postgres_password": "a b", "postgres_db": "app",
                       "enable_ssl": true, "ssl_mode": "verify-full"});
        assert_eq!(
            internal_url("standalone-postgresql", m.as_object().unwrap()).unwrap(),
            "postgres://postgres:a%20b@u1:5432/app?sslmode=verify-full&sslrootcert=/etc/ssl/certs/tinyhelm-ca.crt"
        );
    }

    #[test]
    fn status_accessor_matches_laravel() {
        assert_eq!(database_status("exited"), "exited:unhealthy");
        assert_eq!(database_status("running:healthy"), "running:healthy");
        assert_eq!(database_status("running (healthy)"), "running:healthy");
        assert_eq!(
            database_status("running:healthy:excluded"),
            "running:healthy:excluded"
        );
    }

    #[test]
    fn proxy_secrets_removed_recursively() {
        let mut v = json!({"destination": {"server": {
            "proxy": {"type": "traefik", "last_saved_proxy_configuration": "x"}, "validation_logs": "l"}}});
        remove_server_proxy_secrets(&mut v);
        assert_eq!(
            v["destination"]["server"]["proxy"],
            json!({"type": "traefik"})
        );
        assert!(v["destination"]["server"].get("validation_logs").is_none());
    }
}
