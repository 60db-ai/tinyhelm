//! Native `/api/v1` handlers: environment variables of applications, services and databases.
//!
//! Mirrors `ApplicationsController`, `ServicesController` and `DatabasesController` (`envs`, `create_env`,
//! `update_env_by_uuid`, `delete_env_by_uuid`) together with the `EnvironmentVariable` model they rely on:
//! the global `TrimStrings` / `ConvertEmptyStringsToNull` middleware, validation messages, the
//! serialized-`encrypt()` value accessor, `$hidden` / `$appends`, dirty tracking, model events (the preview
//! copy and the `Auditable` rows) and the controller audit events.

use std::sync::OnceLock;

use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::HeaderMap,
};
use rand::Rng;
use serde_json::{Map, Value};

#[allow(unused_imports)]
use super::prelude::*;

/// Kept for `team.rs` (team shared variables).
#[derive(Deserialize)]
pub struct EnvInput {
    pub(crate) key: String,
    pub(crate) value: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) is_build_time: bool,
}

const SHARED_VARIABLE_TYPES: [&str; 4] = ["team", "project", "environment", "server"];
const GENERATED_MAGIC_VARIABLE_COMMANDS: &[&str] = &[
    "PASSWORD",
    "PASSWORD_64",
    "PASSWORDWITHSYMBOLS",
    "PASSWORDWITHSYMBOLS_64",
    "BASE64",
    "BASE64_32",
    "BASE64_64",
    "BASE64_128",
    "REALBASE64",
    "REALBASE64_32",
    "REALBASE64_64",
    "REALBASE64_128",
    "HEX_32",
    "HEX_64",
    "HEX_128",
    "USER",
    "LOWERCASEUSER",
    "SUPABASEANON",
    "SUPABASESERVICE",
];
/// Columns of `environment_variables` in table order (Eloquent reports dirty fields in this order).
const COLUMN_ORDER: &[&str] = &[
    "id",
    "key",
    "value",
    "is_preview",
    "created_at",
    "updated_at",
    "is_shown_once",
    "is_multiline",
    "version",
    "is_literal",
    "uuid",
    "order",
    "is_required",
    "is_shared",
    "resourceable_type",
    "resourceable_id",
    "is_runtime",
    "is_buildtime",
    "comment",
    "uses_legacy_escaping",
    "preserve_compose_null",
];
/// `$casts` boolean columns; other boolean columns are uncast and keep the raw request value.
const BOOL_CASTS: &[&str] = &[
    "preserve_compose_null",
    "is_multiline",
    "is_literal",
    "is_preview",
    "is_runtime",
    "is_buildtime",
    "uses_legacy_escaping",
];
/// Model `$hidden` plus the controllers' `makeHidden` (value/real_value are re-added when visible).
const HIDDEN: &[&str] = &[
    "id",
    "resourceable_id",
    "resourceable_type",
    "uses_legacy_escaping",
    "value",
    "real_value",
];
const DATABASE_TABLES: &[(&str, &str)] = &[
    ("standalone_postgresqls", r"App\Models\StandalonePostgresql"),
    ("standalone_redis", r"App\Models\StandaloneRedis"),
    ("standalone_mongodbs", r"App\Models\StandaloneMongodb"),
    ("standalone_mysqls", r"App\Models\StandaloneMysql"),
    ("standalone_mariadbs", r"App\Models\StandaloneMariadb"),
    ("standalone_keydbs", r"App\Models\StandaloneKeydb"),
    ("standalone_dragonflies", r"App\Models\StandaloneDragonfly"),
    ("standalone_clickhouses", r"App\Models\StandaloneClickhouse"),
    ("standalone_sqlites", r"App\Models\StandaloneSqlite"),
];
/// Server of a resource through its `destination` morph (StandaloneDocker / SwarmDocker).
const DESTINATION_SERVER: &str = r"(SELECT sd.server_id FROM standalone_dockers sd WHERE sd.id = r.destination_id
     AND r.destination_type = 'App\Models\StandaloneDocker'
     UNION ALL SELECT sw.server_id FROM swarm_dockers sw WHERE sw.id = r.destination_id
     AND r.destination_type = 'App\Models\SwarmDocker' LIMIT 1)";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Application,
    Service,
    Database,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    List,
    Create,
    Update,
    Delete,
}

impl Kind {
    fn audit_name(self) -> &'static str {
        match self {
            Kind::Application => "application",
            Kind::Service => "service",
            Kind::Database => "database",
        }
    }

    /// The controllers word these differently (the application ones lack the final period, except delete).
    fn not_found(self, op: Op) -> &'static str {
        match (self, op) {
            (Kind::Application, Op::Delete) => "Application not found.",
            (Kind::Application, _) => "Application not found",
            (Kind::Service, _) => "Service not found.",
            (Kind::Database, _) => "Database not found.",
        }
    }
}

/// The resource that owns the variables, with what reference resolution needs.
struct Owner {
    rtype: String,
    id: i64,
    uuid: String,
    team_id: i64,
    environment_id: Option<i64>,
    project_id: Option<i64>,
    server_id: Option<i64>,
    build_pack: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------------------------

type St = State<crate::AppState>;
type Cx = Extension<Ctx>;

pub async fn list_app_envs(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    list(st, c, p, req, Kind::Application).await
}
pub async fn list_service_envs(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    list(st, c, p, req, Kind::Service).await
}
pub async fn list_database_envs(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    list(st, c, p, req, Kind::Database).await
}
pub async fn create_app_env(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    create(st, c, p, req, Kind::Application).await
}
pub async fn create_service_env(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    create(st, c, p, req, Kind::Service).await
}
pub async fn create_database_env(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    create(st, c, p, req, Kind::Database).await
}
pub async fn update_app_env(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    update(st, c, p, req, Kind::Application).await
}
pub async fn update_service_env(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    update(st, c, p, req, Kind::Service).await
}
pub async fn update_database_env(st: St, c: Cx, p: Path<String>, req: Request) -> Response {
    update(st, c, p, req, Kind::Database).await
}
pub async fn delete_app_env(st: St, c: Cx, p: Path<(String, String)>) -> Response {
    delete(st, c, p, Kind::Application).await
}
pub async fn delete_service_env(st: St, c: Cx, p: Path<(String, String)>) -> Response {
    delete(st, c, p, Kind::Service).await
}
pub async fn delete_database_env(st: St, c: Cx, p: Path<(String, String)>) -> Response {
    delete(st, c, p, Kind::Database).await
}

// ---------------------------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------------------------

async fn list(
    State(state): St,
    Extension(ctx): Cx,
    Path(uuid): Path<String>,
    req: Request,
    kind: Kind,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(app_key) = state.app_key else {
        return no_key();
    };
    let owner = match find_owner(pool, kind, team_id, &uuid).await {
        Ok(Some(o)) => o,
        Ok(None) => return message(StatusCode::NOT_FOUND, kind.not_found(Op::List)),
        Err(e) => return query_err("list_envs", e),
    };

    // Applications: regular variables by id, then preview ones by id. Others: the relation's unordered query.
    let order = if kind == Kind::Application {
        " ORDER BY ev.is_preview, ev.id"
    } else {
        ""
    };
    let sql = format!(
        "SELECT to_jsonb(ev) FROM environment_variables ev \
         WHERE ev.resourceable_type = $1 AND ev.resourceable_id = $2{order}"
    );
    let rows: Vec<Value> = match sqlx::query_scalar(&sql)
        .bind(&owner.rtype)
        .bind(owner.id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_envs", e),
    };
    if rows
        .iter()
        .any(|r| r["uses_legacy_escaping"] == Value::Bool(true))
    {
        // ponytail: legacy escaping (HasSecretManager::useExactEscaping) is not ported; Laravel answers.
        return crate::proxy(State(state), req).await;
    }

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let attrs = match decrypt_row(row, &app_key) {
            Ok(a) => a,
            Err(r) => return r,
        };
        out.push(present(pool, &owner, &attrs, ctx.can_read_sensitive()).await);
    }
    (StatusCode::OK, Json(Value::Array(out))).into_response()
}

async fn create(
    State(state): St,
    Extension(ctx): Cx,
    Path(uuid): Path<String>,
    req: Request,
    kind: Kind,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(app_key) = state.app_key else {
        return no_key();
    };
    let (parts, bytes) = match split_request(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = parse_input(&parts.headers, parts.uri.query(), &bytes);
    let owner = match find_owner(pool, kind, team_id, &uuid).await {
        Ok(Some(o)) => o,
        Ok(None) => return message(StatusCode::NOT_FOUND, kind.not_found(Op::Create)),
        Err(e) => return query_err("create_env", e),
    };
    let input = normalize_key(input);
    if let Some(errors) = validate(&input, kind) {
        return validation_failed(errors);
    }

    let key =
        php_trim(input.get("key").and_then(Value::as_str).unwrap_or_default()).replace(' ', "_");
    let is_preview = kind == Kind::Application && truthy(input.get("is_preview"));
    let preview_filter = (kind == Kind::Application).then_some(is_preview);
    match env_by_key(pool, &owner, &key, preview_filter).await {
        Ok(Some(_)) => return already_exists(),
        Ok(None) => {}
        Err(e) => return query_err("create_env", e),
    }

    let value = input
        .get("value")
        .and_then(Value::as_str)
        .map(str::to_string);
    if value.as_deref().is_some_and(|v| v.contains('\0')) {
        return nul_byte_error();
    }

    // The attributes the new model holds in memory; the services/databases response serializes exactly these.
    let now = utc_now();
    let stamp = Value::String(now.format("%Y-%m-%dT%H:%M:%S").to_string());
    let mut attrs = Map::new();
    attrs.insert("is_runtime".into(), Value::Bool(true));
    attrs.insert("is_buildtime".into(), Value::Bool(true));
    attrs.insert("key".into(), Value::String(key.clone()));
    attrs.insert(
        "value".into(),
        value.clone().map(Value::String).unwrap_or(Value::Null),
    );
    if kind == Kind::Application {
        attrs.insert(
            "is_preview".into(),
            raw_or(input.get("is_preview"), Value::Bool(false)),
        );
    }
    for field in ["is_literal", "is_multiline", "is_shown_once"] {
        attrs.insert(field.into(), raw_or(input.get(field), Value::Bool(false)));
    }
    if kind == Kind::Application {
        attrs.insert(
            "is_runtime".into(),
            raw_or(input.get("is_runtime"), Value::Bool(true)),
        );
        attrs.insert(
            "is_buildtime".into(),
            raw_or(input.get("is_buildtime"), Value::Bool(true)),
        );
    }
    attrs.insert(
        "comment".into(),
        input.get("comment").cloned().unwrap_or(Value::Null),
    );
    attrs.insert(
        "resourceable_type".into(),
        Value::String(owner.rtype.clone()),
    );
    attrs.insert("resourceable_id".into(), Value::from(owner.id));
    attrs.insert("version".into(), Value::String(version().to_string()));
    attrs.insert("uuid".into(), Value::String(new_public_id()));
    attrs.insert(
        "is_shared".into(),
        Value::Bool(is_shared_reference(value.as_deref())),
    );
    attrs.insert("updated_at".into(), stamp.clone());
    attrs.insert("created_at".into(), stamp);

    let id = match insert_env(pool, &attrs, &app_key, now).await {
        Ok(id) => id,
        Err(InsertError::Duplicate) => return already_exists(),
        Err(InsertError::Encrypt) => return encrypt_failed(),
        Err(InsertError::Db(e)) => return query_err("create_env", e),
    };
    attrs.insert("id".into(), Value::from(id));
    let env_uuid = attrs["uuid"].as_str().unwrap_or_default().to_string();

    // EnvironmentVariable::created -> Auditable row (this model records no "created" changes).
    audit::record(
        pool,
        &ctx,
        "api.environment_variable.created",
        json!({
            "team_id": owner.team_id,
            "environment_variable_uuid": env_uuid,
            "environment_variable_name": key,
            "changed_fields": [],
        }),
        "info",
    )
    .await;
    // EnvironmentVariable::created -> preview copy of a new application variable (no audit rows of its own).
    if kind == Kind::Application && !truthy(attrs.get("is_preview")) {
        if let Err(e) = create_preview_copy(pool, &owner, &attrs, &app_key).await {
            tracing::error!("preview copy of {env_uuid} failed: {e}");
        }
    }

    let mut context = Map::new();
    context.insert("team_id".into(), Value::from(team_id));
    context.insert(
        format!("{}_uuid", kind.audit_name()),
        Value::String(owner.uuid.clone()),
    );
    context.insert("env_uuid".into(), Value::String(env_uuid.clone()));
    context.insert("env_key".into(), Value::String(key));
    if kind == Kind::Application {
        context.insert("is_preview".into(), Value::Bool(is_preview));
    }
    audit::record(
        pool,
        &ctx,
        &format!("api.{}.env_created", kind.audit_name()),
        Value::Object(context),
        "info",
    )
    .await;

    if kind == Kind::Application {
        return (StatusCode::CREATED, Json(json!({ "uuid": env_uuid }))).into_response();
    }
    let body = present(pool, &owner, &attrs, ctx.can_read_sensitive()).await;
    (StatusCode::CREATED, Json(body)).into_response()
}

async fn update(
    State(state): St,
    Extension(ctx): Cx,
    Path(uuid): Path<String>,
    req: Request,
    kind: Kind,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(app_key) = state.app_key else {
        return no_key();
    };
    let (parts, bytes) = match split_request(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if kind == Kind::Application {
        if let Some(r) = validate_incoming_request(&parts.headers, &bytes) {
            return r;
        }
    }
    let input = parse_input(&parts.headers, parts.uri.query(), &bytes);
    let owner = match find_owner(pool, kind, team_id, &uuid).await {
        Ok(Some(o)) => o,
        Ok(None) => return message(StatusCode::NOT_FOUND, kind.not_found(Op::Update)),
        Err(e) => return query_err("update_env", e),
    };
    let input = normalize_key(input);
    if let Some(errors) = validate(&input, kind) {
        return validation_failed(errors);
    }

    let key =
        php_trim(input.get("key").and_then(Value::as_str).unwrap_or_default()).replace(' ', "_");
    let is_preview = kind == Kind::Application && truthy(input.get("is_preview"));
    let preview_filter = (kind == Kind::Application).then_some(is_preview);
    let row = match env_by_key(pool, &owner, &key, preview_filter).await {
        Ok(Some(r)) => r,
        Ok(None) => return message(StatusCode::NOT_FOUND, "Environment variable not found."),
        Err(e) => return query_err("update_env", e),
    };
    if row["uses_legacy_escaping"] == Value::Bool(true) {
        // ponytail: legacy escaping is not ported; Laravel performs the update and answers.
        return crate::proxy(State(state), Request::from_parts(parts, Body::from(bytes))).await;
    }
    // The stored plaintext before withoutEdgeLineBreaks: Laravel's dirty check compares decrypted strings.
    let stored_plain = match row
        .get("value")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    {
        Some(cipher) => match crate::crypto::decrypt(cipher, &app_key) {
            Some(p) => Some(p),
            None => return decrypt_failed(),
        },
        None => None,
    };
    let original = match decrypt_row(row, &app_key) {
        Ok(a) => a,
        Err(r) => return r,
    };

    // Apply the request like the controllers do.
    let mut attrs = original.clone();
    if input.contains_key("value") {
        let v = input["value"]
            .as_str()
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null);
        attrs.insert("value".into(), v);
    }
    let fillable: &[&str] = if kind == Kind::Application {
        &[
            "is_literal",
            "is_multiline",
            "is_shown_once",
            "is_runtime",
            "is_buildtime",
            "comment",
        ]
    } else {
        &["is_literal", "is_multiline", "is_shown_once", "comment"]
    };
    for field in fillable {
        if let Some(v) = input.get(*field) {
            attrs.insert((*field).into(), v.clone());
        }
    }

    // EnvironmentVariable::saving hooks.
    let value_dirty = value_is_dirty(stored_plain.as_deref(), attrs.get("value"));
    if truthy(original.get("is_shown_once")) {
        attrs.insert("is_shown_once".into(), Value::Bool(true));
    }
    if truthy(original.get("preserve_compose_null")) && value_dirty && !attrs["value"].is_null() {
        attrs.insert("preserve_compose_null".into(), Value::Bool(false));
    }
    if value_dirty && attrs["value"].as_str().is_some_and(|v| v.contains('\0')) {
        return nul_byte_error();
    }
    let shared = is_shared_reference(attrs["value"].as_str());
    attrs.insert("is_shared".into(), Value::Bool(shared));

    let changed: Vec<&str> = COLUMN_ORDER
        .iter()
        .copied()
        .filter(|field| match *field {
            "value" => value_dirty,
            f => !equivalent(f, original.get(f), attrs.get(f)),
        })
        .collect();

    if !changed.is_empty() {
        let now = utc_now();
        attrs.insert(
            "updated_at".into(),
            Value::String(now.format("%Y-%m-%dT%H:%M:%S").to_string()),
        );
        let env_id = original["id"].as_i64().unwrap_or_default();
        if let Err(r) = save_changes(pool, env_id, &attrs, &changed, &app_key, now).await {
            return r;
        }
        let mut context = Map::new();
        context.insert("team_id".into(), Value::from(team_id));
        context.insert(
            format!("{}_uuid", kind.audit_name()),
            Value::String(owner.uuid.clone()),
        );
        context.insert("env_uuid".into(), attrs["uuid"].clone());
        context.insert("env_key".into(), attrs["key"].clone());
        if kind == Kind::Application {
            context.insert("is_preview".into(), Value::Bool(is_preview));
        }
        context.insert("changed_fields".into(), json!(changed));
        audit::record(
            pool,
            &ctx,
            &format!("api.{}.env_updated", kind.audit_name()),
            Value::Object(context),
            "info",
        )
        .await;
    }

    let body = present(pool, &owner, &attrs, ctx.can_read_sensitive()).await;
    (StatusCode::CREATED, Json(body)).into_response()
}

async fn delete(
    State(state): St,
    Extension(ctx): Cx,
    Path((uuid, env_uuid)): Path<(String, String)>,
    kind: Kind,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let owner = match find_owner(pool, kind, team_id, &uuid).await {
        Ok(Some(o)) => o,
        Ok(None) => return message(StatusCode::NOT_FOUND, kind.not_found(Op::Delete)),
        Err(e) => return query_err("delete_env", e),
    };
    let row = sqlx::query(
        "DELETE FROM environment_variables WHERE id = (SELECT id FROM environment_variables \
         WHERE uuid = $1 AND resourceable_type = $2 AND resourceable_id = $3 LIMIT 1) RETURNING uuid, key",
    )
    .bind(&env_uuid)
    .bind(&owner.rtype)
    .bind(owner.id)
    .fetch_optional(pool)
    .await;
    let (deleted_uuid, deleted_key): (Option<String>, String) = match row {
        Ok(Some(r)) => (
            r.try_get("uuid").ok().flatten(),
            r.try_get("key").unwrap_or_default(),
        ),
        Ok(None) => return message(StatusCode::NOT_FOUND, "Environment variable not found."),
        Err(e) => return query_err("delete_env", e),
    };

    // EnvironmentVariable::deleted -> Auditable row.
    // ponytail: Laravel also stores the old field values in `changes`; audit::record has no changes column yet.
    audit::record(
        pool,
        &ctx,
        "api.environment_variable.deleted",
        json!({
            "team_id": owner.team_id,
            "environment_variable_uuid": deleted_uuid,
            "environment_variable_name": deleted_key,
            "changed_fields": [],
        }),
        "info",
    )
    .await;
    let mut context = Map::new();
    context.insert("team_id".into(), Value::from(team_id));
    context.insert(
        format!("{}_uuid", kind.audit_name()),
        Value::String(owner.uuid.clone()),
    );
    context.insert("env_uuid".into(), json!(deleted_uuid));
    context.insert("env_key".into(), Value::String(deleted_key));
    audit::record(
        pool,
        &ctx,
        &format!("api.{}.env_deleted", kind.audit_name()),
        Value::Object(context),
        "info",
    )
    .await;
    (
        StatusCode::OK,
        Json(json!({"message": "Environment variable deleted."})),
    )
        .into_response()
}

// ---------------------------------------------------------------------------------------------
// Resource lookup
// ---------------------------------------------------------------------------------------------

async fn find_owner(
    pool: &PgPool,
    kind: Kind,
    team_id: i64,
    uuid: &str,
) -> Result<Option<Owner>, sqlx::Error> {
    match kind {
        Kind::Application => {
            let sql = format!(
                "SELECT r.id, r.uuid, r.environment_id, e.project_id, r.build_pack, {DESTINATION_SERVER} AS server_id \
                 FROM applications r JOIN environments e ON e.id = r.environment_id \
                 JOIN projects p ON p.id = e.project_id JOIN teams t ON t.id = p.team_id \
                 WHERE t.id = $1 AND r.uuid = $2 AND r.deleted_at IS NULL ORDER BY r.name LIMIT 1"
            );
            let row = sqlx::query(&sql)
                .bind(team_id)
                .bind(uuid)
                .fetch_optional(pool)
                .await?;
            Ok(row.map(|r| owner_from(&r, r"App\Models\Application", team_id)))
        }
        Kind::Service => {
            let row = sqlx::query(
                "SELECT r.id, r.uuid, r.environment_id, e.project_id, NULL::text AS build_pack, r.server_id \
                 FROM services r JOIN environments e ON e.id = r.environment_id \
                 JOIN projects p ON p.id = e.project_id JOIN teams t ON t.id = p.team_id \
                 WHERE t.id = $1 AND r.uuid = $2 AND r.deleted_at IS NULL LIMIT 1",
            )
            .bind(team_id)
            .bind(uuid)
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|r| owner_from(&r, r"App\Models\Service", team_id)))
        }
        Kind::Database => {
            // queryDatabaseByUuidWithinTeam: the first engine whose row has the uuid and is in the team.
            for (table, class) in DATABASE_TABLES {
                let sql = format!(
                    "SELECT r.id, r.uuid, r.environment_id, e.project_id, p.team_id, NULL::text AS build_pack, \
                     {DESTINATION_SERVER} AS server_id FROM {table} r \
                     LEFT JOIN environments e ON e.id = r.environment_id LEFT JOIN projects p ON p.id = e.project_id \
                     WHERE r.uuid = $1 AND r.deleted_at IS NULL LIMIT 1"
                );
                let Some(row) = sqlx::query(&sql).bind(uuid).fetch_optional(pool).await? else {
                    continue;
                };
                let id: i64 = row.try_get("id").unwrap_or(-1);
                if *table == "standalone_postgresqls" && id == 0 {
                    return Ok(None); // the instance database is never exposed
                }
                if row.try_get::<Option<i64>, _>("team_id").ok().flatten() == Some(team_id) {
                    return Ok(Some(owner_from(&row, class, team_id)));
                }
            }
            Ok(None)
        }
    }
}

fn owner_from(row: &sqlx::postgres::PgRow, rtype: &str, team_id: i64) -> Owner {
    Owner {
        rtype: rtype.to_string(),
        id: row.try_get("id").unwrap_or_default(),
        uuid: row.try_get("uuid").unwrap_or_default(),
        team_id,
        environment_id: row.try_get("environment_id").ok().flatten(),
        project_id: row.try_get("project_id").ok().flatten(),
        server_id: row.try_get("server_id").ok().flatten(),
        build_pack: row.try_get("build_pack").ok().flatten(),
    }
}

/// The variable with the key (applications: within the preview or the regular set).
async fn env_by_key(
    pool: &PgPool,
    owner: &Owner,
    key: &str,
    preview: Option<bool>,
) -> Result<Option<Value>, sqlx::Error> {
    let filter = if preview.is_some() {
        " AND ev.is_preview = $4"
    } else {
        ""
    };
    let sql = format!(
        "SELECT to_jsonb(ev) FROM environment_variables ev \
         WHERE ev.resourceable_type = $1 AND ev.resourceable_id = $2 AND ev.key = $3{filter} LIMIT 1"
    );
    let mut q = sqlx::query_scalar::<_, Value>(&sql)
        .bind(&owner.rtype)
        .bind(owner.id)
        .bind(key);
    if let Some(p) = preview {
        q = q.bind(p);
    }
    q.fetch_optional(pool).await
}

// ---------------------------------------------------------------------------------------------
// Serialization (toArray + removeSensitiveData + serializeApiResponse)
// ---------------------------------------------------------------------------------------------

/// Row from `to_jsonb` with the encrypted `value` replaced by the accessor's plaintext.
fn decrypt_row(row: Value, app_key: &[u8; 32]) -> Result<Map<String, Value>, Response> {
    let Value::Object(mut map) = row else {
        return Ok(Map::new());
    };
    let plain = match map
        .get("value")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    {
        Some(cipher) => match crate::crypto::decrypt(cipher, app_key) {
            Some(p) => Value::String(without_edge_line_breaks(&p)),
            None => return Err(decrypt_failed()),
        },
        None => Value::Null,
    };
    map.insert("value".into(), plain);
    Ok(map)
}

async fn present(
    pool: &PgPool,
    owner: &Owner,
    attrs: &Map<String, Value>,
    can_read_sensitive: bool,
) -> Value {
    let value = attrs.get("value").and_then(Value::as_str);
    let mut out = Map::new();
    for (k, v) in attrs {
        if HIDDEN.contains(&k.as_str()) {
            continue;
        }
        let v = match (k.as_str(), v) {
            ("created_at" | "updated_at", Value::String(s)) => {
                Value::String(super::model::carbon(s))
            }
            (f, v) if BOOL_CASTS.contains(&f) && !v.is_null() => Value::Bool(truthy(Some(v))),
            (_, v) => v.clone(),
        };
        out.insert(k.clone(), v);
    }
    let key = attrs.get("key").and_then(Value::as_str).unwrap_or_default();
    out.insert("is_shared".into(), Value::Bool(is_shared_reference(value)));
    let really_required = truthy(attrs.get("is_required"))
        && resolve(pool, owner, value, true)
            .await
            .unwrap_or_default()
            .is_empty();
    out.insert("is_really_required".into(), Value::Bool(really_required));
    let buildpack = key.starts_with("NIXPACKS_") || key.starts_with("RAILPACK_");
    out.insert("is_buildpack_control".into(), Value::Bool(buildpack));
    out.insert(
        "is_tinyhelm".into(),
        Value::Bool(key.starts_with("SERVICE_")),
    );
    if can_read_sensitive && !truthy(attrs.get("is_shown_once")) {
        out.insert(
            "value".into(),
            value
                .map(|v| Value::String(v.to_string()))
                .unwrap_or(Value::Null),
        );
        let display = resolve(pool, owner, value, false).await;
        out.insert(
            "real_value".into(),
            display.map(Value::String).unwrap_or(Value::Null),
        );
    }
    Value::Object(out)
}

/// `get_real_environment_variables_internal`: replace `{{type.KEY}}` references with shared variable values.
/// `reveal_locked` false keeps references to locked (shown once) shared variables, like `displayRealValue`.
async fn resolve(
    pool: &PgPool,
    owner: &Owner,
    value: Option<&str>,
    reveal_locked: bool,
) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return Some(String::new());
    }
    let mut out = without_edge_line_breaks(value);
    for (reference, kind, key) in shared_references(&out.clone()) {
        let Some(column) = SHARED_VARIABLE_TYPES.iter().find(|t| **t == kind) else {
            continue;
        };
        let id = match kind.as_str() {
            "environment" => owner.environment_id,
            "project" => owner.project_id,
            "team" => Some(owner.team_id),
            _ => owner.server_id,
        };
        let Some(id) = id else { continue };
        let sql = format!(
            "SELECT value, is_shown_once FROM shared_environment_variables \
             WHERE type = $1 AND key = $2 AND team_id = $3 AND {column}_id = $4 LIMIT 1"
        );
        let found = sqlx::query(&sql)
            .bind(&kind)
            .bind(&key)
            .bind(owner.team_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
        let Some(found) = found else { continue };
        let locked = found
            .try_get::<Option<bool>, _>("is_shown_once")
            .ok()
            .flatten()
            .unwrap_or(false);
        if !reveal_locked && locked {
            continue;
        }
        let replacement = found
            .try_get::<Option<String>, _>("value")
            .ok()
            .flatten()
            .and_then(|c| shared_key().and_then(|k| crate::crypto::decrypt(&c, &k)))
            .unwrap_or_default();
        out = out.replace(&reference, &replacement);
    }
    Some(out)
}

fn shared_key() -> Option<[u8; 32]> {
    static KEY: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    *KEY.get_or_init(|| {
        std::env::var("APP_KEY")
            .ok()
            .and_then(|k| crate::crypto::parse_key(&k))
    })
}

/// `SHARED_VARIABLE_REFERENCE_PATTERN` = `/{{\s*(\w+)\.([^}\s]+)\s*}}/`, all matches: (full, type, key).
fn shared_references(s: &str) -> Vec<(String, String, String)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < b.len() {
        if !(b[i] == b'{' && b[i + 1] == b'{') {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        while j < b.len() && is_pcre_space(b[j]) {
            j += 1;
        }
        let t0 = j;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
            j += 1;
        }
        let t1 = j;
        if t1 == t0 || j >= b.len() || b[j] != b'.' {
            i += 1;
            continue;
        }
        j += 1;
        let k0 = j;
        while j < b.len() && b[j] != b'}' && !is_pcre_space(b[j]) {
            j += 1;
        }
        let k1 = j;
        while j < b.len() && is_pcre_space(b[j]) {
            j += 1;
        }
        if k1 == k0 || j + 1 >= b.len() || b[j] != b'}' || b[j + 1] != b'}' {
            i += 1;
            continue;
        }
        out.push((
            s[i..j + 2].to_string(),
            s[t0..t1].to_string(),
            s[k0..k1].to_string(),
        ));
        i = j + 2;
    }
    out
}

fn is_pcre_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// `isSharedReference`: the whole (trimmed) value is one `{{team|project|environment|server.…}}` reference.
fn is_shared_reference(value: Option<&str>) -> bool {
    let Some(v) = value else { return false };
    let t = php_trim(v);
    if t.len() < 4 || !t.starts_with("{{") || !t.ends_with("}}") {
        return false;
    }
    let rest = t[2..].trim_start_matches(|c: char| c.is_ascii() && is_pcre_space(c as u8));
    SHARED_VARIABLE_TYPES.iter().any(|ty| {
        rest.strip_prefix(ty)
            .and_then(|r| r.strip_prefix('.'))
            .is_some_and(|after| after.len() >= 2)
    })
}

// ---------------------------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------------------------

enum InsertError {
    Duplicate,
    Encrypt,
    Db(sqlx::Error),
}

/// `performInsert`: serialize inserts of one key with an advisory lock and refuse a second row.
async fn insert_env(
    pool: &PgPool,
    attrs: &Map<String, Value>,
    app_key: &[u8; 32],
    now: chrono::NaiveDateTime,
) -> Result<i64, InsertError> {
    let cipher = match attrs.get("value").and_then(Value::as_str) {
        Some(v) => Some(
            crate::crypto::encrypt(&without_edge_line_breaks(v), app_key)
                .ok_or(InsertError::Encrypt)?,
        ),
        None => None,
    };
    let rtype = attrs["resourceable_type"].as_str().unwrap_or_default();
    let rid = attrs["resourceable_id"].as_i64().unwrap_or_default();
    let key = attrs["key"].as_str().unwrap_or_default();
    let preview = truthy(attrs.get("is_preview"));

    let mut tx = pool.begin().await.map_err(InsertError::Db)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "environment_variables:{rtype}:{rid}:{}:{key}",
            i32::from(preview)
        ))
        .execute(&mut *tx)
        .await
        .map_err(InsertError::Db)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM environment_variables WHERE resourceable_type = $1 AND resourceable_id = $2 \
         AND is_preview = $3 AND key = $4)",
    )
    .bind(rtype)
    .bind(rid)
    .bind(preview)
    .bind(key)
    .fetch_one(&mut *tx)
    .await
    .map_err(InsertError::Db)?;
    if exists {
        return Err(InsertError::Duplicate);
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO environment_variables (key, value, is_preview, is_literal, is_multiline, is_shown_once, \
         is_runtime, is_buildtime, is_required, comment, resourceable_type, resourceable_id, version, uuid, is_shared, \
         created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$16) RETURNING id",
    )
    .bind(key)
    .bind(cipher)
    .bind(preview)
    .bind(truthy(attrs.get("is_literal")))
    .bind(truthy(attrs.get("is_multiline")))
    .bind(truthy(attrs.get("is_shown_once")))
    .bind(truthy(attrs.get("is_runtime")))
    .bind(truthy(attrs.get("is_buildtime")))
    .bind(truthy(attrs.get("is_required")))
    .bind(attrs.get("comment").and_then(Value::as_str))
    .bind(rtype)
    .bind(rid)
    .bind(attrs["version"].as_str())
    .bind(attrs["uuid"].as_str())
    .bind(truthy(attrs.get("is_shared")))
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(InsertError::Db)?;
    tx.commit().await.map_err(InsertError::Db)?;
    Ok(id)
}

/// `EnvironmentVariable::created`: a new regular application variable gets a preview copy, unless one exists or
/// it is a generated magic variable of a Docker Compose application.
async fn create_preview_copy(
    pool: &PgPool,
    owner: &Owner,
    env: &Map<String, Value>,
    app_key: &[u8; 32],
) -> Result<(), String> {
    let key = env["key"].as_str().unwrap_or_default();
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM environment_variables WHERE key = $1 AND resourceable_type = $2 \
         AND resourceable_id = $3 AND is_preview = true)",
    )
    .bind(key)
    .bind(&owner.rtype)
    .bind(owner.id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    if exists
        || (owner.build_pack.as_deref() == Some("dockercompose")
            && is_generated_magic_variable(key))
    {
        return Ok(());
    }
    let mut copy = Map::new();
    copy.insert("key".into(), env["key"].clone());
    copy.insert(
        "value".into(),
        env.get("value").cloned().unwrap_or(Value::Null),
    );
    for f in [
        "is_multiline",
        "is_literal",
        "is_runtime",
        "is_buildtime",
        "is_required",
        "is_shown_once",
    ] {
        copy.insert(f.into(), Value::Bool(truthy(env.get(f))));
    }
    copy.insert(
        "comment".into(),
        env.get("comment").cloned().unwrap_or(Value::Null),
    );
    copy.insert(
        "resourceable_type".into(),
        Value::String(owner.rtype.clone()),
    );
    copy.insert("resourceable_id".into(), Value::from(owner.id));
    copy.insert("is_preview".into(), Value::Bool(true));
    copy.insert("version".into(), Value::String(version().to_string()));
    copy.insert("uuid".into(), Value::String(new_public_id()));
    copy.insert(
        "is_shared".into(),
        Value::Bool(is_shared_reference(
            env.get("value").and_then(Value::as_str),
        )),
    );
    match insert_env(pool, &copy, app_key, utc_now()).await {
        Ok(_) | Err(InsertError::Duplicate) => Ok(()),
        Err(InsertError::Encrypt) => Err("encryption failed".into()),
        Err(InsertError::Db(e)) => Err(e.to_string()),
    }
}

async fn save_changes(
    pool: &PgPool,
    env_id: i64,
    attrs: &Map<String, Value>,
    changed: &[&str],
    app_key: &[u8; 32],
    now: chrono::NaiveDateTime,
) -> Result<(), Response> {
    let mut q =
        sqlx::QueryBuilder::<sqlx::Postgres>::new("UPDATE environment_variables SET updated_at = ");
    q.push_bind(now);
    for field in changed {
        q.push(format!(", \"{field}\" = "));
        match *field {
            "value" => {
                let cipher = match attrs["value"].as_str() {
                    Some(v) => Some(
                        crate::crypto::encrypt(&without_edge_line_breaks(v), app_key)
                            .ok_or_else(encrypt_failed)?,
                    ),
                    None => None,
                };
                q.push_bind(cipher);
            }
            "comment" => {
                q.push_bind(attrs["comment"].as_str().map(str::to_string));
            }
            f => {
                q.push_bind(truthy(attrs.get(f)));
            }
        }
    }
    q.push(" WHERE id = ");
    q.push_bind(env_id);
    q.build()
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| query_err("update_env", e))
}

/// Eloquent `originalIsEquivalent` for the non-value columns this endpoint can change.
fn equivalent(field: &str, original: Option<&Value>, new: Option<&Value>) -> bool {
    let (o, n) = (
        original.unwrap_or(&Value::Null),
        new.unwrap_or(&Value::Null),
    );
    if o == n {
        return true;
    }
    if n.is_null() {
        return false;
    }
    if BOOL_CASTS.contains(&field) {
        return truthy(Some(o)) == truthy(Some(n));
    }
    // Uncast columns: equal only when identical, or both numeric with the same string form.
    match (php_numeric_string(o), php_numeric_string(n)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Dirty check of the encrypted value: Laravel compares the decrypted (serialized) strings.
fn value_is_dirty(stored_plain: Option<&str>, new: Option<&Value>) -> bool {
    match (stored_plain, new.and_then(Value::as_str)) {
        (None, None) => false,
        (Some(old), Some(new)) => old != without_edge_line_breaks(new),
        _ => true,
    }
}

// ---------------------------------------------------------------------------------------------
// Request input (TrimStrings, ConvertEmptyStringsToNull) and validation
// ---------------------------------------------------------------------------------------------

async fn split_request(req: Request) -> Result<(axum::http::request::Parts, Bytes), Response> {
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, 16 * 1024 * 1024)
        .await
        .map_err(|_| message(StatusCode::PAYLOAD_TOO_LARGE, "Payload too large."))?;
    Ok((parts, bytes))
}

fn content_type(headers: &HeaderMap) -> &str {
    headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
}

fn is_json(headers: &HeaderMap) -> bool {
    let ct = content_type(headers);
    ct.contains("/json") || ct.contains("+json")
}

/// `$request->all()`: the JSON (or form) body plus the query string (the body wins), after the global middleware.
fn parse_input(headers: &HeaderMap, query: Option<&str>, body: &[u8]) -> Map<String, Value> {
    let mut map = if is_json(headers) {
        match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(m)) => m,
            Ok(Value::Array(items)) => items
                .into_iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v))
                .collect(),
            Ok(Value::Null) | Err(_) => Map::new(),
            Ok(scalar) => Map::from_iter([("0".to_string(), scalar)]),
        }
    } else if content_type(headers).starts_with("application/x-www-form-urlencoded") {
        form_pairs(std::str::from_utf8(body).unwrap_or_default())
    } else {
        Map::new()
    };
    for (k, v) in form_pairs(query.unwrap_or_default()) {
        map.entry(k).or_insert(v);
    }
    map.into_iter()
        .map(|(k, v)| {
            let cleaned = clean(v, k == "value");
            (k, cleaned)
        })
        .collect()
}

/// `TrimStrings` (except `value`) then `ConvertEmptyStringsToNull`, recursively.
fn clean(v: Value, skip_trim: bool) -> Value {
    match v {
        Value::String(s) => {
            let s = if skip_trim {
                s
            } else {
                laravel_trim(&s).to_string()
            };
            if s.is_empty() {
                Value::Null
            } else {
                Value::String(s)
            }
        }
        Value::Array(items) => Value::Array(items.into_iter().map(|i| clean(i, false)).collect()),
        Value::Object(m) => {
            Value::Object(m.into_iter().map(|(k, v)| (k, clean(v, false))).collect())
        }
        other => other,
    }
}

/// application/x-www-form-urlencoded pairs (`+` is a space, `%XX` decoded).
// ponytail: PHP bracket arrays (`a[b]=c`) are kept as plain keys; nothing in these routes takes arrays.
fn form_pairs(s: &str) -> Map<String, Value> {
    fn decode(part: &str) -> String {
        let bytes = part.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => out.push(b' '),
                b'%' if i + 2 < bytes.len() => {
                    match std::str::from_utf8(&bytes[i + 1..i + 3])
                        .ok()
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                    {
                        Some(b) => {
                            out.push(b);
                            i += 2;
                        }
                        None => out.push(b'%'),
                    }
                }
                b => out.push(b),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    s.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), Value::String(decode(v)))
        })
        .collect()
}

/// `$request->merge(['key' => normalizeEnvironmentVariableKey((string) $request->key)])` when `key` is present.
fn normalize_key(mut input: Map<String, Value>) -> Map<String, Value> {
    if let Some(v) = input.get("key") {
        let as_string = match v {
            Value::Null | Value::Bool(false) => String::new(),
            Value::Bool(true) => "1".into(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            Value::Array(_) | Value::Object(_) => "Array".into(),
        };
        input.insert(
            "key".into(),
            Value::String(php_trim(&as_string).to_string()),
        );
    }
    input
}

/// `validateIncomingRequest` (application PATCH only).
fn validate_incoming_request(headers: &HeaderMap, body: &[u8]) -> Option<Response> {
    let invalid = |error: &str| {
        Some(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"message": "Invalid request.", "error": error})),
            )
                .into_response(),
        )
    };
    if !is_json(headers) {
        return invalid("Content-Type must be application/json.");
    }
    let decoded: Option<Value> = serde_json::from_slice(body).ok();
    let truthy_json = match &decoded {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !(s.is_empty() || s == "0"),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Bool(true)) | Some(Value::Object(_)) => true,
    };
    if !truthy_json {
        return invalid("Invalid JSON.");
    }
    if matches!(decoded, Some(Value::Object(ref m)) if m.is_empty()) {
        return invalid("Empty JSON.");
    }
    None
}

fn validate(input: &Map<String, Value>, kind: Kind) -> Option<Map<String, Value>> {
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut fail = |field: &str, msg: String| errors.push((field.to_string(), msg));

    // key: required|string|max:255|regex
    match input.get("key") {
        None | Some(Value::Null) => fail("key", "This field is required.".into()),
        Some(Value::String(s)) if s.trim().is_empty() => {
            fail("key", "This field is required.".into())
        }
        Some(v) => {
            let s = v.as_str().unwrap_or_default();
            if !v.is_string() {
                fail("key", "The key field must be a string.".into());
            }
            if s.chars().count() > 255 {
                fail(
                    "key",
                    "The key field must not be greater than 255 characters.".into(),
                );
            }
            if !is_valid_key(s) {
                fail("key", "The key field format is invalid.".into());
            }
        }
    }
    // value: string|nullable
    if let Some(v) = input.get("value") {
        if !v.is_null() && !v.is_string() {
            fail("value", "The value field must be a string.".into());
        }
    }
    let booleans: &[&str] = if kind == Kind::Application {
        &[
            "is_preview",
            "is_literal",
            "is_multiline",
            "is_shown_once",
            "is_runtime",
            "is_buildtime",
        ]
    } else {
        &["is_literal", "is_multiline", "is_shown_once"]
    };
    for field in booleans {
        if let Some(v) = input.get(*field) {
            if !is_laravel_boolean(v) {
                fail(
                    field,
                    format!(
                        "The {} field must be true or false.",
                        field.replace('_', " ")
                    ),
                );
            }
        }
    }
    // comment: string|nullable|max:256
    if let Some(v) = input.get("comment") {
        if !v.is_null() {
            if !v.is_string() {
                fail("comment", "The comment field must be a string.".into());
            }
            if laravel_size(v) > 256 {
                fail(
                    "comment",
                    "The comment field must not be greater than 256 characters.".into(),
                );
            }
        }
    }
    if kind == Kind::Application {
        const ALLOWED: &[&str] = &[
            "key",
            "value",
            "is_preview",
            "is_literal",
            "is_multiline",
            "is_shown_once",
            "is_runtime",
            "is_buildtime",
            "comment",
        ];
        for k in input.keys().filter(|k| !ALLOWED.contains(&k.as_str())) {
            fail(k, "This field is not allowed.".into());
        }
    }

    if errors.is_empty() {
        return None;
    }
    let mut map = Map::new();
    for (field, msg) in errors {
        if let Value::Array(list) = map.entry(field).or_insert_with(|| Value::Array(Vec::new())) {
            list.push(Value::String(msg));
        }
    }
    Some(map)
}

/// `ValidationPatterns::ENVIRONMENT_VARIABLE_KEY_PATTERN` = `/\A[A-Za-z_][A-Za-z0-9_.]*\z/u`.
fn is_valid_key(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Laravel `boolean` rule: strictly one of true, false, 0, 1, "0", "1".
fn is_laravel_boolean(v: &Value) -> bool {
    match v {
        Value::Bool(_) => true,
        Value::Number(n) => !n.is_f64() && n.as_i64().is_some_and(|i| i == 0 || i == 1),
        Value::String(s) => s == "0" || s == "1",
        _ => false,
    }
}

/// `getSize` without a numeric rule: string length, array count, or the length of the string form.
fn laravel_size(v: &Value) -> usize {
    match v {
        Value::String(s) => s.chars().count(),
        Value::Array(a) => a.len(),
        Value::Object(m) => m.len(),
        Value::Bool(true) => 1,
        Value::Bool(false) | Value::Null => 0,
        Value::Number(n) => n.to_string().chars().count(),
    }
}

fn validation_failed(errors: Map<String, Value>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": "Validation failed.", "errors": errors})),
    )
        .into_response()
}

/// `rejectNulByteInEnvironmentVariableValue` throws a ValidationException from the model.
fn nul_byte_error() -> Response {
    let msg = "The value cannot contain a NUL character.";
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": {"value": [msg]}})),
    )
        .into_response()
}

// ---------------------------------------------------------------------------------------------
// Small PHP-compatible helpers
// ---------------------------------------------------------------------------------------------

/// PHP truthiness of a raw attribute (`?? false` / `if (...)`).
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !(s.is_empty() || s == "0"),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(m)) => !m.is_empty(),
    }
}

/// `$request->field ?? default`: the raw value, or the default when absent or null.
fn raw_or(v: Option<&Value>, default: Value) -> Value {
    match v {
        None | Some(Value::Null) => default,
        Some(v) => v.clone(),
    }
}

fn php_numeric_string(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if s.trim().parse::<f64>().is_ok() => Some(s.clone()),
        _ => None,
    }
}

/// PHP `trim()` default characters.
fn php_trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B'))
}

/// `TrimStrings`: whitespace plus BOM / zero-width space / Mongolian vowel separator.
fn laravel_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{FEFF}' | '\u{200B}' | '\u{180E}'))
}

/// `EnvironmentVariable::withoutEdgeLineBreaks`.
fn without_edge_line_breaks(value: &str) -> String {
    let trimmed = value.trim_matches(|c| c == '\r' || c == '\n');
    if trimmed.contains('\n') {
        value.to_string()
    } else {
        trimmed.to_string()
    }
}

/// `isGeneratedMagicVariable` with `parseCommandFromMagicEnvVariable`.
fn is_generated_magic_variable(key: &str) -> bool {
    if !key.starts_with("SERVICE_") {
        return false;
    }
    let after = key
        .split_once("SERVICE_")
        .map(|(_, a)| a)
        .unwrap_or_default();
    let command = match key.matches('_').count() {
        2 => after.rsplit_once('_').map(|(c, _)| c).unwrap_or_default(),
        3 if key.starts_with("SERVICE_FQDN") || key.starts_with("SERVICE_URL") => {
            after.split_once('_').map(|(c, _)| c).unwrap_or(after)
        }
        3 => after.rsplit_once('_').map(|(c, _)| c).unwrap_or_default(),
        _ => "",
    };
    GENERATED_MAGIC_VARIABLE_COMMANDS.contains(&command)
}

/// `new_public_id()`: 24 lowercase alphanumeric characters.
fn new_public_id() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

/// `config('constants.tinyhelm.version')`: TINYHELM_VERSION, else release.json `stable.tinyhelm`.
fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        std::env::var("TINYHELM_VERSION")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                serde_json::from_str::<Value>(include_str!("../../../release.json"))
                    .ok()
                    .and_then(|r| r["stable"]["tinyhelm"].as_str().map(str::to_string))
                    .unwrap_or_default()
            })
    })
}

/// Laravel writes timestamps in the app timezone (UTC), to the second. The database session timezone is not UTC,
/// so `now()` in SQL must not be used for these columns.
fn utc_now() -> chrono::NaiveDateTime {
    use chrono::Timelike;
    let now = chrono::Utc::now().naive_utc();
    now.with_nanosecond(0).unwrap_or(now)
}

fn message(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "message": msg }))).into_response()
}

fn already_exists() -> Response {
    message(
        StatusCode::CONFLICT,
        "Environment variable already exists. Use PATCH request to update it.",
    )
}

fn no_key() -> Response {
    message(
        StatusCode::SERVICE_UNAVAILABLE,
        "Encryption key not configured.",
    )
}

fn decrypt_failed() -> Response {
    message(StatusCode::INTERNAL_SERVER_ERROR, "The payload is invalid.")
}

fn encrypt_failed() -> Response {
    message(StatusCode::INTERNAL_SERVER_ERROR, "Encryption failed.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_match_the_laravel_pattern() {
        let refs = shared_references("a {{ team.DB_URL }} b {{project.X}} {{bad}} {{ 1.x}}");
        let found: Vec<(&str, &str)> = refs
            .iter()
            .map(|(_, t, k)| (t.as_str(), k.as_str()))
            .collect();
        assert_eq!(
            found,
            vec![("team", "DB_URL"), ("project", "X"), ("1", "x")]
        );
        assert_eq!(refs[0].0, "{{ team.DB_URL }}");
    }

    #[test]
    fn shared_reference_needs_the_whole_value() {
        assert!(is_shared_reference(Some(" {{team.A}} ")));
        assert!(is_shared_reference(Some("{{ server.KEY }}")));
        assert!(!is_shared_reference(Some("x {{team.A}}")));
        assert!(!is_shared_reference(Some("{{other.A}}")));
        assert!(!is_shared_reference(None));
    }

    #[test]
    fn validation_messages_match_laravel() {
        let mut input = Map::new();
        input.insert("key".into(), json!("1BAD"));
        input.insert("is_literal".into(), json!("yes"));
        input.insert("extra".into(), json!(1));
        let errors = validate(&input, Kind::Application).unwrap();
        assert_eq!(errors["key"], json!(["The key field format is invalid."]));
        assert_eq!(
            errors["is_literal"],
            json!(["The is literal field must be true or false."])
        );
        assert_eq!(errors["extra"], json!(["This field is not allowed."]));
        let ok = normalize_key(Map::from_iter([("key".into(), json!(" OK "))]));
        assert!(validate(&ok, Kind::Service).is_none());
        assert_eq!(
            validate(&Map::new(), Kind::Database).unwrap()["key"],
            json!(["This field is required."])
        );
    }

    #[test]
    fn input_cleaning_keeps_value_untrimmed() {
        let mut h = HeaderMap::new();
        h.insert("content-type", "application/json".parse().unwrap());
        let m = parse_input(
            &h,
            Some("q=1"),
            br#"{"key":" A ","value":" v ","comment":""}"#,
        );
        assert_eq!(m["key"], json!("A"));
        assert_eq!(m["value"], json!(" v "));
        assert_eq!(m["comment"], Value::Null);
        assert_eq!(m["q"], json!("1"));
    }

    #[test]
    fn magic_variables() {
        assert!(is_generated_magic_variable("SERVICE_PASSWORD_DB"));
        assert!(is_generated_magic_variable("SERVICE_BASE64_64_DB"));
        assert!(!is_generated_magic_variable("SERVICE_FQDN_APP"));
        assert!(!is_generated_magic_variable("DATABASE_URL"));
    }

    #[test]
    fn edge_line_breaks_and_dirty_value() {
        assert_eq!(without_edge_line_breaks("\nabc\r\n"), "abc");
        assert_eq!(without_edge_line_breaks("a\nb\n"), "a\nb\n");
        assert!(!value_is_dirty(Some("abc"), Some(&json!("abc\n"))));
        assert!(value_is_dirty(Some("abc"), Some(&Value::Null)));
        assert!(!equivalent(
            "is_shown_once",
            Some(&json!(false)),
            Some(&json!("0"))
        ));
        assert!(equivalent(
            "is_literal",
            Some(&json!(false)),
            Some(&json!("0"))
        ));
    }
}
