//! Native `/api/v1` handlers: the token's team (Laravel `TeamController::current_team`) and team
//! shared variables (`SharedEnvironmentVariablesController`, type `team`; logic in `shared_envs.rs`).

#[allow(unused_imports)]
use super::prelude::*;

use axum::{body::Bytes, http::HeaderMap};
use serde_json::Value;

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
