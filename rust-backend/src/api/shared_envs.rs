//! Shared variables (Laravel `SharedEnvironmentVariablesController`) for every scope: team, project,
//! environment and server. The scoped handlers resolve the parent (404 first, as Laravel), then call the
//! generic list/create/update/delete below.

#[allow(unused_imports)]
use super::prelude::*;

use axum::{body::Bytes, http::HeaderMap};
use serde_json::{Map, Value};

/// `SharedEnvironmentVariable` columns in table order (Eloquent attribute order for `changed_fields`).
const SHARED_ENV_COLUMNS: &[&str] = &[
    "id",
    "key",
    "value",
    "is_shown_once",
    "type",
    "team_id",
    "project_id",
    "environment_id",
    "created_at",
    "updated_at",
    "is_multiline",
    "version",
    "is_literal",
    "comment",
    "server_id",
];
/// `SharedEnvironmentVariable::$hidden` (excluded from audit `changes`).
const SHARED_ENV_HIDDEN: &[&str] = &["value"];
const SHARED_ENV_ALLOWED: &[&str] = &[
    "key",
    "value",
    "is_literal",
    "is_multiline",
    "is_shown_once",
    "comment",
];
const SHARED_ENV_RESOURCE: &str = "shared_environment_variable";

/// The `type` of a shared variable and the parent it belongs to.
#[derive(Clone, Copy)]
pub(crate) struct EnvScope {
    kind: &'static str,
    project_id: Option<i64>,
    environment_id: Option<i64>,
    server_id: Option<i64>,
}

impl EnvScope {
    pub(crate) const TEAM: EnvScope = EnvScope {
        kind: "team",
        project_id: None,
        environment_id: None,
        server_id: None,
    };

    pub(crate) fn project(id: i64) -> Self {
        Self {
            kind: "project",
            project_id: Some(id),
            ..Self::TEAM
        }
    }

    pub(crate) fn environment(id: i64) -> Self {
        Self {
            kind: "environment",
            environment_id: Some(id),
            ..Self::TEAM
        }
    }

    pub(crate) fn server(id: i64) -> Self {
        Self {
            kind: "server",
            server_id: Some(id),
            ..Self::TEAM
        }
    }
}

/// `removeSensitiveData`: drop the scope columns and `version`; `value` (decrypted) only for tokens
/// that may read secrets, and never for show-once variables.
fn shared_env_json(row: Map<String, Value>, ctx: &Ctx, state: &crate::AppState) -> Value {
    let shown_once = row
        .get("is_shown_once")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reveal = ctx.can_read_sensitive() && !shown_once;
    let mut out: Map<String, Value> = row
        .into_iter()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "team_id" | "project_id" | "environment_id" | "server_id" | "version"
            )
        })
        .collect();
    let encrypted = out.remove("value");
    if reveal {
        let plain = encrypted.as_ref().and_then(Value::as_str).and_then(|v| {
            state
                .app_key
                .as_ref()
                .and_then(|k| crate::crypto::decrypt(v, k))
        });
        out.insert("value".into(), json!(plain));
    }
    to_laravel(Value::Object(out), &ModelSpec::default())
}

/// `findEnvInScope`: the scope's own columns are matched; the others are not constrained.
const SCOPE_FILTER: &str =
    "team_id = $1 AND type = $2 AND ($3::bigint IS NULL OR project_id = $3) \
    AND ($4::bigint IS NULL OR environment_id = $4) AND ($5::bigint IS NULL OR server_id = $5)";

async fn env_row(
    pool: &PgPool,
    team_id: i64,
    scope: EnvScope,
    id: &str,
) -> Result<Option<Map<String, Value>>, sqlx::Error> {
    // Laravel compares the route segment with the integer key; a non-numeric id matches nothing.
    let Ok(id) = id.parse::<i64>() else {
        return Ok(None);
    };
    sqlx::query_scalar::<_, Value>(&format!(
        "SELECT to_jsonb(s) FROM shared_environment_variables s WHERE {SCOPE_FILTER} AND id = $6"
    ))
    .bind(team_id)
    .bind(scope.kind)
    .bind(scope.project_id)
    .bind(scope.environment_id)
    .bind(scope.server_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map(|r| r.map(as_map))
}

/// `keyExistsInScope`: the scope's columns equal, the other scope columns NULL.
async fn key_exists(
    pool: &PgPool,
    team_id: i64,
    scope: EnvScope,
    key: &str,
    except_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shared_environment_variables WHERE team_id = $1 AND type = $2 \
         AND key = $3 AND project_id IS NOT DISTINCT FROM $4 AND environment_id IS NOT DISTINCT FROM $5 \
         AND server_id IS NOT DISTINCT FROM $6 AND ($7::bigint IS NULL OR id <> $7))",
    )
    .bind(team_id)
    .bind(scope.kind)
    .bind(key)
    .bind(scope.project_id)
    .bind(scope.environment_id)
    .bind(scope.server_id)
    .bind(except_id)
    .fetch_one(pool)
    .await
}

/// `validateEnvPayload`: `customApiValidator` rules and extra fields, the empty-PATCH check, then the
/// model's NUL check.
fn validate_shared_env(input: &Map<String, Value>, require_key: bool) -> Option<Response> {
    let presence = if require_key {
        Rule::Required
    } else {
        Rule::Nullable
    };
    let mut v = Validator::new(input, CUSTOM_API_MESSAGES);
    v.field(
        "key",
        &[
            presence,
            Rule::Str,
            Rule::Max(255),
            Rule::Regex(ENV_KEY_PATTERN),
        ],
    );
    v.field("value", &[Rule::Str, Rule::Nullable]);
    v.field("is_literal", &[Rule::Boolean]);
    v.field("is_multiline", &[Rule::Boolean]);
    v.field("is_shown_once", &[Rule::Boolean]);
    v.field("comment", &[Rule::Str, Rule::Nullable, Rule::Max(256)]);
    v.reject_extra(SHARED_ENV_ALLOWED);
    if v.fails() {
        return Some(v.response());
    }
    if !require_key && input.is_empty() {
        return Some(
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"message": "At least one field must be provided."})),
            )
                .into_response(),
        );
    }
    if input_str(input, "value").is_some_and(|s| s.contains('\0')) {
        let msg = "The value cannot contain a NUL character.";
        return Some(
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"message": msg, "errors": {"value": [msg]}})),
            )
                .into_response(),
        );
    }
    None
}

fn encrypt_value(state: &crate::AppState, value: Option<&str>) -> Result<Option<String>, Response> {
    let Some(value) = value else {
        return Ok(None);
    };
    let key = state.app_key.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"message": "APP_KEY not configured."})),
        )
            .into_response()
    })?;
    crate::crypto::encrypt_string(value, key)
        .map(Some)
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"message": "Encryption failed."})),
            )
                .into_response()
        })
}

/// The controller-level audit row (`api.shared_env.<action>`).
async fn audit_shared_env(
    pool: &PgPool,
    ctx: &Ctx,
    action: &str,
    team_id: i64,
    scope: EnvScope,
    id: i64,
    key: &str,
) {
    audit::record(
        pool,
        ctx,
        &format!("api.shared_env.{action}"),
        json!({"team_id": team_id, "env_id": id, "env_key": key, "type": scope.kind}),
        "info",
    )
    .await;
}

fn conflict(message: &str) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "message": message }))).into_response()
}

/// `listEnvs` — ordered by id.
pub(crate) async fn list_envs(
    state: &crate::AppState,
    ctx: &Ctx,
    pool: &PgPool,
    team_id: i64,
    scope: EnvScope,
) -> Response {
    match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT to_jsonb(s) FROM shared_environment_variables s WHERE {SCOPE_FILTER} ORDER BY id"
    ))
    .bind(team_id)
    .bind(scope.kind)
    .bind(scope.project_id)
    .bind(scope.environment_id)
    .bind(scope.server_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(
            rows.into_iter()
                .map(|r| shared_env_json(as_map(r), ctx, state))
                .collect::<Vec<_>>(),
        ),
        Err(e) => query_err("list_shared_envs", e),
    }
}

/// `createEnv` — 409 when the key exists in the scope; returns `{"id": ...}`.
pub(crate) async fn create_env(
    state: &crate::AppState,
    ctx: &Ctx,
    pool: &PgPool,
    team_id: i64,
    scope: EnvScope,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    let input = match json_input(headers, body) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if let Some(r) = validate_shared_env(&input, true) {
        return r;
    }
    let key = input_str(&input, "key")
        .unwrap_or_default()
        .trim()
        .to_string();
    match key_exists(pool, team_id, scope, &key, None).await {
        Ok(true) => {
            return conflict("Environment variable already exists. Use PATCH request to update it.")
        }
        Ok(false) => {}
        Err(e) => return query_err("create_shared_env", e),
    }
    let value = match encrypt_value(state, input_str(&input, "value").as_deref()) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let is_literal = input_bool(&input, "is_literal");
    let is_multiline = input_bool(&input, "is_multiline");
    let is_shown_once = input_bool(&input, "is_shown_once");
    let comment = input_str(&input, "comment");

    let id: i64 = match sqlx::query_scalar(&format!(
        "INSERT INTO shared_environment_variables (key, value, is_literal, is_multiline, is_shown_once, comment, \
         type, team_id, project_id, environment_id, server_id, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,{NOW},{NOW}) RETURNING id"
    ))
    .bind(&key)
    .bind(&value)
    .bind(is_literal)
    .bind(is_multiline)
    .bind(is_shown_once)
    .bind(&comment)
    .bind(scope.kind)
    .bind(team_id)
    .bind(scope.project_id)
    .bind(scope.environment_id)
    .bind(scope.server_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => return query_err("create_shared_env", e),
    };

    let new = as_map(json!({
        "key": key, "value": value, "is_literal": is_literal, "is_multiline": is_multiline,
        "is_shown_once": is_shown_once, "comment": comment, "type": scope.kind, "team_id": team_id,
        "project_id": scope.project_id, "environment_id": scope.environment_id, "server_id": scope.server_id,
    }));
    let changes = audit_changes(None, Some(&new), &fields_of(&new), SHARED_ENV_HIDDEN);
    record_mutation(
        state,
        pool,
        ctx,
        team_id,
        SHARED_ENV_RESOURCE,
        "created",
        None,
        Some(&key),
        vec![],
        changes,
    )
    .await;
    audit_shared_env(pool, ctx, "created", team_id, scope, id, &key).await;

    (StatusCode::CREATED, Json(json!({ "id": id }))).into_response()
}

/// `updateEnv` — returns the updated variable.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_env(
    state: &crate::AppState,
    ctx: &Ctx,
    pool: &PgPool,
    team_id: i64,
    scope: EnvScope,
    id: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    let old = match env_row(pool, team_id, scope, id).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found("Environment variable not found."),
        Err(e) => return query_err("update_shared_env", e),
    };
    let input = match json_input(headers, body) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if let Some(r) = validate_shared_env(&input, false) {
        return r;
    }
    let env_id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
    let old_key = old
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut new = old.clone();
    if let Some(key) = input_str(&input, "key").map(|k| k.trim().to_string()) {
        if key != old_key {
            match key_exists(pool, team_id, scope, &key, Some(env_id)).await {
                Ok(true) => return conflict("Environment variable already exists with this key."),
                Ok(false) => {}
                Err(e) => return query_err("update_shared_env", e),
            }
            new.insert("key".into(), Value::String(key));
        }
    }
    // `value` is an `encrypted` cast: dirty only when the plaintext differs.
    let old_plain = old.get("value").and_then(Value::as_str).and_then(|v| {
        state
            .app_key
            .as_ref()
            .and_then(|k| crate::crypto::decrypt(v, k))
    });
    if input.contains_key("value") {
        let plain = input_str(&input, "value");
        if plain != old_plain {
            match encrypt_value(state, plain.as_deref()) {
                Ok(enc) => {
                    new.insert("value".into(), json!(enc));
                }
                Err(r) => return r,
            }
        }
    }
    for flag in ["is_literal", "is_multiline", "is_shown_once"] {
        if input.contains_key(flag) {
            new.insert(flag.into(), Value::Bool(input_bool(&input, flag)));
        }
    }
    // A show-once lock is permanent (model `saving` hook).
    if old.get("is_shown_once").and_then(Value::as_bool) == Some(true) {
        new.insert("is_shown_once".into(), Value::Bool(true));
    }
    if input.contains_key("comment") {
        new.insert("comment".into(), json!(input_str(&input, "comment")));
    }

    let changed = changed_fields(&old, &new, &[], SHARED_ENV_COLUMNS);
    if !changed.is_empty() {
        let saved = sqlx::query(&format!(
            "UPDATE shared_environment_variables SET key = $1, value = $2, is_literal = $3, is_multiline = $4, \
             is_shown_once = $5, comment = $6, updated_at = {NOW} WHERE id = $7"
        ))
        .bind(new.get("key").and_then(Value::as_str))
        .bind(new.get("value").and_then(Value::as_str))
        .bind(new.get("is_literal").and_then(Value::as_bool))
        .bind(new.get("is_multiline").and_then(Value::as_bool))
        .bind(new.get("is_shown_once").and_then(Value::as_bool))
        .bind(new.get("comment").and_then(Value::as_str))
        .bind(env_id)
        .execute(pool)
        .await;
        if let Err(e) = saved {
            return query_err("update_shared_env", e);
        }
        let changes = audit_changes(Some(&old), Some(&new), &changed, SHARED_ENV_HIDDEN);
        let key = new.get("key").and_then(Value::as_str);
        record_mutation(
            state,
            pool,
            ctx,
            team_id,
            SHARED_ENV_RESOURCE,
            "updated",
            None,
            key,
            changed,
            changes,
        )
        .await;
    }
    let key = new
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    audit_shared_env(pool, ctx, "updated", team_id, scope, env_id, &key).await;

    match env_row(pool, team_id, scope, &env_id.to_string()).await {
        Ok(Some(row)) => ok_json(shared_env_json(row, ctx, state)),
        Ok(None) => not_found("Environment variable not found."),
        Err(e) => query_err("update_shared_env", e),
    }
}

/// `deleteEnv`.
pub(crate) async fn delete_env(
    state: &crate::AppState,
    ctx: &Ctx,
    pool: &PgPool,
    team_id: i64,
    scope: EnvScope,
    id: &str,
) -> Response {
    let old = match env_row(pool, team_id, scope, id).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found("Environment variable not found."),
        Err(e) => return query_err("delete_shared_env", e),
    };
    let env_id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
    if let Err(e) = sqlx::query("DELETE FROM shared_environment_variables WHERE id = $1")
        .bind(env_id)
        .execute(pool)
        .await
    {
        return query_err("delete_shared_env", e);
    }
    let key = old
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let changes = audit_changes(Some(&old), None, &fields_of(&old), SHARED_ENV_HIDDEN);
    record_mutation(
        state,
        pool,
        ctx,
        team_id,
        SHARED_ENV_RESOURCE,
        "deleted",
        None,
        Some(&key),
        vec![],
        changes,
    )
    .await;
    audit_shared_env(pool, ctx, "deleted", team_id, scope, env_id, &key).await;

    (
        StatusCode::OK,
        Json(json!({"message": "Environment variable deleted."})),
    )
        .into_response()
}

// ---- scope resolution (Laravel resolveProject / resolveEnvironment / resolveServer) ----

/// `resolveProject`: the project id, or the Laravel 404.
pub(crate) async fn resolve_project_id(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<i64, Response> {
    match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM projects WHERE team_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(id)) => Ok(id),
        Ok(None) => Err(not_found("Project not found.")),
        Err(e) => Err(query_err("resolve_project", e)),
    }
}

/// `resolveEnvironment`: by name first, then by uuid, inside the project.
pub(crate) async fn resolve_environment(
    pool: &PgPool,
    project_id: i64,
    name_or_uuid: &str,
) -> Result<Map<String, Value>, Response> {
    for column in ["name", "uuid"] {
        let found = sqlx::query_scalar::<_, Value>(&format!(
            "SELECT to_jsonb(e) FROM environments e WHERE project_id = $1 AND {column} = $2 ORDER BY id LIMIT 1"
        ))
        .bind(project_id)
        .bind(name_or_uuid)
        .fetch_optional(pool)
        .await;
        match found {
            Ok(Some(row)) => return Ok(as_map(row)),
            Ok(None) => {}
            Err(e) => return Err(query_err("resolve_environment", e)),
        }
    }
    Err(not_found("Environment not found."))
}

async fn resolve_server_id(pool: &PgPool, team_id: i64, uuid: &str) -> Result<i64, Response> {
    match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM servers WHERE team_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(id)) => Ok(id),
        Ok(None) => Err(not_found("Server not found.")),
        Err(e) => Err(query_err("resolve_server", e)),
    }
}

async fn project_scope(pool: &PgPool, team_id: i64, uuid: &str) -> Result<EnvScope, Response> {
    resolve_project_id(pool, team_id, uuid)
        .await
        .map(EnvScope::project)
}

async fn environment_scope(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
    environment: &str,
) -> Result<EnvScope, Response> {
    let project_id = resolve_project_id(pool, team_id, uuid).await?;
    let env = resolve_environment(pool, project_id, environment).await?;
    Ok(EnvScope::environment(
        env.get("id").and_then(Value::as_i64).unwrap_or_default(),
    ))
}

async fn server_scope(pool: &PgPool, team_id: i64, uuid: &str) -> Result<EnvScope, Response> {
    resolve_server_id(pool, team_id, uuid)
        .await
        .map(EnvScope::server)
}

/// Ability check, then the scope (both may answer with Laravel's error response).
macro_rules! scoped {
    ($state:expr, $ctx:expr, $ability:literal, $resolve:expr) => {{
        let (pool, team_id) = match $ctx.require(&$state, $ability).await {
            Ok(v) => v,
            Err(r) => return r,
        };
        let scope = match ($resolve)(pool, team_id).await {
            Ok(s) => s,
            Err(r) => return r,
        };
        (pool, team_id, scope)
    }};
}

// ---- project shared variables ----

/// GET /api/v1/projects/:uuid/envs
pub async fn list_project_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "read", |p, t| project_scope(p, t, &uuid));
    list_envs(&state, &ctx, pool, team_id, scope).await
}

/// POST /api/v1/projects/:uuid/envs
pub async fn create_project_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "write", |p, t| project_scope(p, t, &uuid));
    create_env(&state, &ctx, pool, team_id, scope, &headers, &body).await
}

/// PATCH /api/v1/projects/:uuid/envs/:env_id
pub async fn update_project_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "write", |p, t| project_scope(p, t, &uuid));
    update_env(&state, &ctx, pool, team_id, scope, &env_id, &headers, &body).await
}

/// DELETE /api/v1/projects/:uuid/envs/:env_id
pub async fn delete_project_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_id)): Path<(String, String)>,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "delete", |p, t| project_scope(p, t, &uuid));
    delete_env(&state, &ctx, pool, team_id, scope, &env_id).await
}

// ---- environment shared variables ----

/// GET /api/v1/projects/:uuid/environments/:environment/envs
pub async fn list_environment_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, environment)): Path<(String, String)>,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "read", |p, t| environment_scope(
        p,
        t,
        &uuid,
        &environment
    ));
    list_envs(&state, &ctx, pool, team_id, scope).await
}

/// POST /api/v1/projects/:uuid/environments/:environment/envs
pub async fn create_environment_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, environment)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "write", |p, t| environment_scope(
        p,
        t,
        &uuid,
        &environment
    ));
    create_env(&state, &ctx, pool, team_id, scope, &headers, &body).await
}

/// PATCH /api/v1/projects/:uuid/environments/:environment/envs/:env_id
pub async fn update_environment_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, environment, env_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "write", |p, t| environment_scope(
        p,
        t,
        &uuid,
        &environment
    ));
    update_env(&state, &ctx, pool, team_id, scope, &env_id, &headers, &body).await
}

/// DELETE /api/v1/projects/:uuid/environments/:environment/envs/:env_id
pub async fn delete_environment_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, environment, env_id)): Path<(String, String, String)>,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "delete", |p, t| environment_scope(
        p,
        t,
        &uuid,
        &environment
    ));
    delete_env(&state, &ctx, pool, team_id, scope, &env_id).await
}

// ---- server shared variables ----

/// GET /api/v1/servers/:uuid/envs
pub async fn list_server_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "read", |p, t| server_scope(p, t, &uuid));
    list_envs(&state, &ctx, pool, team_id, scope).await
}

/// POST /api/v1/servers/:uuid/envs
pub async fn create_server_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "write", |p, t| server_scope(p, t, &uuid));
    create_env(&state, &ctx, pool, team_id, scope, &headers, &body).await
}

/// PATCH /api/v1/servers/:uuid/envs/:env_id
pub async fn update_server_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "write", |p, t| server_scope(p, t, &uuid));
    update_env(&state, &ctx, pool, team_id, scope, &env_id, &headers, &body).await
}

/// DELETE /api/v1/servers/:uuid/envs/:env_id
pub async fn delete_server_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_id)): Path<(String, String)>,
) -> Response {
    let (pool, team_id, scope) = scoped!(state, ctx, "delete", |p, t| server_scope(p, t, &uuid));
    delete_env(&state, &ctx, pool, team_id, scope, &env_id).await
}
