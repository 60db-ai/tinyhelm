//! Native `/api/v1` handlers: the token's team (Laravel `TeamController::current_team`) and team
//! shared variables (`SharedEnvironmentVariablesController`, type `team`).

#[allow(unused_imports)]
use super::prelude::*;

use axum::{body::Bytes, http::HeaderMap};
use serde_json::{Map, Value};

/// `Team` model JSON minus `custom_server_limit` and `pivot` (`TeamController::removeSensitiveData`).
const TEAM_SPEC: ModelSpec = ModelSpec {
    hidden: &["custom_server_limit"],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// GET /api/v1/team — the token's team.
pub async fn get_current_team(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM teams t WHERE id = $1")
        .bind(team_id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(team)) => ok_json(to_laravel(team, &TEAM_SPEC)),
        Ok(None) => not_found("Team not found."),
        Err(e) => query_err("get_current_team", e),
    }
}

// ---- team shared variables ----

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

async fn shared_env_row(
    pool: &PgPool,
    team_id: i64,
    id: &str,
) -> Result<Option<Map<String, Value>>, sqlx::Error> {
    // Laravel compares the route segment with the integer key; a non-numeric id matches nothing.
    let Ok(id) = id.parse::<i64>() else {
        return Ok(None);
    };
    sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(s) FROM shared_environment_variables s WHERE team_id = $1 AND type = 'team' AND id = $2",
    )
    .bind(team_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map(|r| r.map(as_map))
}

/// `keyExistsInScope` for team variables (no project, environment or server).
async fn team_key_exists(
    pool: &PgPool,
    team_id: i64,
    key: &str,
    except_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shared_environment_variables WHERE team_id = $1 AND type = 'team' \
         AND key = $2 AND project_id IS NULL AND environment_id IS NULL AND server_id IS NULL \
         AND ($3::bigint IS NULL OR id <> $3))",
    )
    .bind(team_id)
    .bind(key)
    .bind(except_id)
    .fetch_one(pool)
    .await
}

/// `validateEnvPayload`: `customApiValidator` rules and extra fields, then the model's NUL check.
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
    id: i64,
    key: &str,
) {
    audit::record(
        pool,
        ctx,
        &format!("api.shared_env.{action}"),
        json!({"team_id": team_id, "env_id": id, "env_key": key, "type": "team"}),
        "info",
    )
    .await;
}

fn conflict(message: &str) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "message": message }))).into_response()
}

/// GET /api/v1/team/envs — ordered by id.
pub async fn list_team_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(s) FROM shared_environment_variables s WHERE team_id = $1 AND type = 'team' ORDER BY id",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(
            rows.into_iter()
                .map(|r| shared_env_json(as_map(r), &ctx, &state))
                .collect::<Vec<_>>(),
        ),
        Err(e) => query_err("list_team_envs", e),
    }
}

/// POST /api/v1/team/envs — 409 when the key exists; returns `{"id": ...}`.
pub async fn create_team_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = match json_input(&headers, &body) {
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
    match team_key_exists(pool, team_id, &key, None).await {
        Ok(true) => {
            return conflict("Environment variable already exists. Use PATCH request to update it.")
        }
        Ok(false) => {}
        Err(e) => return query_err("create_team_env", e),
    }
    let value = match encrypt_value(&state, input_str(&input, "value").as_deref()) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let is_literal = input_bool(&input, "is_literal");
    let is_multiline = input_bool(&input, "is_multiline");
    let is_shown_once = input_bool(&input, "is_shown_once");
    let comment = input_str(&input, "comment");

    let id: i64 = match sqlx::query_scalar(&format!(
        "INSERT INTO shared_environment_variables (key, value, is_literal, is_multiline, is_shown_once, comment, \
         type, team_id, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,'team',$7,{NOW},{NOW}) RETURNING id"
    ))
    .bind(&key)
    .bind(&value)
    .bind(is_literal)
    .bind(is_multiline)
    .bind(is_shown_once)
    .bind(&comment)
    .bind(team_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => return query_err("create_team_env", e),
    };

    let new = as_map(json!({
        "key": key, "value": value, "is_literal": is_literal, "is_multiline": is_multiline,
        "is_shown_once": is_shown_once, "comment": comment, "type": "team", "team_id": team_id,
        "project_id": null, "environment_id": null, "server_id": null,
    }));
    let changes = audit_changes(None, Some(&new), &fields_of(&new), SHARED_ENV_HIDDEN);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        SHARED_ENV_RESOURCE,
        "created",
        None,
        Some(&key),
        vec![],
        changes,
    )
    .await;
    audit_shared_env(pool, &ctx, "created", team_id, id, &key).await;

    (StatusCode::CREATED, Json(json!({ "id": id }))).into_response()
}

/// PATCH /api/v1/team/envs/:id — returns the updated variable.
pub async fn update_team_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let old = match shared_env_row(pool, team_id, &id).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found("Environment variable not found."),
        Err(e) => return query_err("update_team_env", e),
    };
    let input = match json_input(&headers, &body) {
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
            match team_key_exists(pool, team_id, &key, Some(env_id)).await {
                Ok(true) => return conflict("Environment variable already exists with this key."),
                Ok(false) => {}
                Err(e) => return query_err("update_team_env", e),
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
            match encrypt_value(&state, plain.as_deref()) {
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
            return query_err("update_team_env", e);
        }
        let changes = audit_changes(Some(&old), Some(&new), &changed, SHARED_ENV_HIDDEN);
        let key = new.get("key").and_then(Value::as_str);
        record_mutation(
            &state,
            pool,
            &ctx,
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
    audit_shared_env(pool, &ctx, "updated", team_id, env_id, &key).await;

    match shared_env_row(pool, team_id, &env_id.to_string()).await {
        Ok(Some(row)) => ok_json(shared_env_json(row, &ctx, &state)),
        Ok(None) => not_found("Environment variable not found."),
        Err(e) => query_err("update_team_env", e),
    }
}

/// DELETE /api/v1/team/envs/:id
pub async fn delete_team_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let old = match shared_env_row(pool, team_id, &id).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found("Environment variable not found."),
        Err(e) => return query_err("delete_team_env", e),
    };
    let env_id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
    if let Err(e) = sqlx::query("DELETE FROM shared_environment_variables WHERE id = $1")
        .bind(env_id)
        .execute(pool)
        .await
    {
        return query_err("delete_team_env", e);
    }
    let key = old
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let changes = audit_changes(Some(&old), None, &fields_of(&old), SHARED_ENV_HIDDEN);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        SHARED_ENV_RESOURCE,
        "deleted",
        None,
        Some(&key),
        vec![],
        changes,
    )
    .await;
    audit_shared_env(pool, &ctx, "deleted", team_id, env_id, &key).await;

    (
        StatusCode::OK,
        Json(json!({"message": "Environment variable deleted."})),
    )
        .into_response()
}

// ---- teams and members (TeamController) ----

/// `User` JSON for team member lists: `$hidden` plus `pivot`, `email_change_code(_expires_at)`.
const MEMBER_SPEC: ModelSpec = ModelSpec {
    hidden: &[
        "password",
        "remember_token",
        "two_factor_recovery_codes",
        "two_factor_secret",
        "created_before_oauth_identities",
        "created_by_invitation",
        "email_change_code",
        "email_change_code_expires_at",
    ],
    datetimes: &["email_verified_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// `teams` columns in table order (Eloquent attribute order for `changed_fields`).
const TEAM_COLUMNS: &[&str] = &[
    "id",
    "name",
    "description",
    "personal_team",
    "created_at",
    "updated_at",
    "show_boarding",
    "custom_server_limit",
    "is_mcp_server_enabled",
    "is_build_server_fallback_enabled",
    "avatar_path",
    "avatar_storage_type",
    "avatar_s3_storage_id",
    "is_two_factor_required",
    "scan_images_on_new_servers",
];
const TEAM_UPDATABLE: &[&str] = &[
    "is_build_server_fallback_enabled",
    "scan_images_on_new_servers",
];

async fn team_row(pool: &PgPool, team_id: i64) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>("SELECT to_jsonb(t) FROM teams t WHERE id = $1")
        .bind(team_id)
        .fetch_optional(pool)
        .await
}

/// PHP `(int)` of a route segment: the leading integer, 0 when there is none.
fn route_int(s: &str) -> i64 {
    let end = s
        .char_indices()
        .find(|(i, c)| !(c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+'))))
        .map_or(s.len(), |(i, _)| i);
    s[..end].parse().unwrap_or(0)
}

/// GET /api/v1/teams — the token's team only.
pub async fn list_teams(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match team_row(pool, team_id).await {
        Ok(team) => ok_json(
            team.into_iter()
                .map(|t| to_laravel(t, &TEAM_SPEC))
                .collect::<Vec<_>>(),
        ),
        Err(e) => query_err("list_teams", e),
    }
}

/// GET /api/v1/teams/:id — 404 unless it is the token's team.
pub async fn get_team_by_id(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if route_int(&id) != team_id {
        return not_found("Team not found.");
    }
    match team_row(pool, team_id).await {
        Ok(Some(team)) => ok_json(to_laravel(team, &TEAM_SPEC)),
        Ok(None) => not_found("Team not found."),
        Err(e) => query_err("get_team_by_id", e),
    }
}

async fn members_json(pool: &PgPool, team_id: i64) -> Response {
    // The `members` relation query (no ORDER BY), so rows come back in the same order as Laravel's.
    match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(users) FROM users INNER JOIN team_user ON users.id = team_user.user_id \
         WHERE team_user.team_id = $1",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(
            rows.into_iter()
                .map(|r| to_laravel(r, &MEMBER_SPEC))
                .collect::<Vec<_>>(),
        ),
        Err(e) => query_err("team_members", e),
    }
}

/// GET /api/v1/team/members and /api/v1/teams/current/members
pub async fn list_current_team_members(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    members_json(pool, team_id).await
}

/// GET /api/v1/teams/:id/members
pub async fn list_team_members_by_id(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if route_int(&id) != team_id {
        return not_found("Team not found.");
    }
    members_json(pool, team_id).await
}

/// `customApiValidator`: `required_without` between the two switches, then `boolean`, then extras.
fn validate_team_update(input: &Map<String, Value>) -> Option<Response> {
    let filled = |f: &str| match input.get(f) {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    };
    let mut v = Validator::new(input, CUSTOM_API_MESSAGES);
    for (field, other) in [
        (TEAM_UPDATABLE[0], TEAM_UPDATABLE[1]),
        (TEAM_UPDATABLE[1], TEAM_UPDATABLE[0]),
    ] {
        if !filled(field) && !filled(other) {
            v.add(
                field,
                &format!(
                    "The {} field is required when {} is not present.",
                    field.replace('_', " "),
                    other.replace('_', " ")
                ),
            );
        } else {
            v.field(field, &[Rule::Boolean]);
        }
    }
    v.reject_extra(TEAM_UPDATABLE);
    v.fails().then(|| v.response())
}

/// PATCH /api/v1/team — only the two team switches; `api.team.updated` when one changes.
pub async fn update_current_team(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let old = match team_row(pool, team_id).await {
        Ok(Some(t)) => as_map(t),
        Ok(None) => return not_found("Team not found."),
        Err(e) => return query_err("update_current_team", e),
    };
    // `$request->all()` (this route skips validateIncomingRequest): a non-object body is empty input.
    let input = json_input(&headers, &body).unwrap_or_default();
    if let Some(r) = validate_team_update(&input) {
        return r;
    }

    let mut new = old.clone();
    for field in TEAM_UPDATABLE {
        if input.contains_key(*field) {
            new.insert(field.to_string(), Value::Bool(input_bool(&input, field)));
        }
    }
    let changed = changed_fields(&old, &new, &[], TEAM_COLUMNS);
    if changed.is_empty() {
        return ok_json(to_laravel(Value::Object(old), &TEAM_SPEC));
    }
    let saved = sqlx::query(&format!(
        "UPDATE teams SET is_build_server_fallback_enabled = $1, scan_images_on_new_servers = $2, \
         updated_at = {NOW} WHERE id = $3"
    ))
    .bind(new.get(TEAM_UPDATABLE[0]).and_then(Value::as_bool))
    .bind(new.get(TEAM_UPDATABLE[1]).and_then(Value::as_bool))
    .bind(team_id)
    .execute(pool)
    .await;
    if let Err(e) = saved {
        return query_err("update_current_team", e);
    }
    let changes = audit_changes(Some(&old), Some(&new), &changed, &["custom_server_limit"]);
    let name = old.get("name").and_then(Value::as_str).map(str::to_string);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "team",
        "updated",
        None,
        name.as_deref(),
        changed,
        changes,
    )
    .await;
    match team_row(pool, team_id).await {
        Ok(Some(team)) => ok_json(to_laravel(team, &TEAM_SPEC)),
        Ok(None) => not_found("Team not found."),
        Err(e) => query_err("update_current_team", e),
    }
}
