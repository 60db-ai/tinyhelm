//! Native `/api/v1` handlers: the token's team (Laravel `TeamController::current_team`) and team
//! shared variables (`SharedEnvironmentVariablesController`, type `team`; logic in `shared_envs.rs`).

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

// ---- team shared variables (generic code in shared_envs.rs) ----

/// GET /api/v1/team/envs — ordered by id.
pub async fn list_team_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    list_envs(&state, &ctx, pool, team_id, EnvScope::TEAM).await
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
    create_env(&state, &ctx, pool, team_id, EnvScope::TEAM, &headers, &body).await
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
    update_env(
        &state,
        &ctx,
        pool,
        team_id,
        EnvScope::TEAM,
        &id,
        &headers,
        &body,
    )
    .await
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
    delete_env(&state, &ctx, pool, team_id, EnvScope::TEAM, &id).await
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
