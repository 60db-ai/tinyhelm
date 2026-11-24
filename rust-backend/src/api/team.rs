//! Native `/api/v1` handlers: team.

#[allow(unused_imports)]
use super::prelude::*;

// ---- current team ----
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct TeamRow {
    id: i64,
    name: Option<String>,
}

/// GET /api/v1/team — the token's team.
pub async fn get_current_team(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_as::<_, TeamRow>("SELECT id, name FROM teams WHERE id = $1 LIMIT 1")
        .bind(team_id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(t)) => (StatusCode::OK, Json(t)).into_response(),
        Ok(None) => not_found("Team not found."),
        Err(e) => query_err("get_current_team", e),
    }
}

// ---- team shared variables (shared_environment_variables, type='team', keyed by integer id) ----
#[derive(sqlx::FromRow)]
pub(crate) struct SharedEnvRow {
    id: i64,
    key: String,
    value: String,
}

/// GET /api/v1/team/envs
pub async fn list_team_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = sqlx::query_as::<_, SharedEnvRow>(
        "SELECT id, key, value FROM shared_environment_variables WHERE type = 'team' AND team_id = $1 ORDER BY key",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await;
    match rows {
        Ok(rs) => {
            let out: Vec<_> = rs
                .into_iter()
                .map(|r| {
                    let plain = state
                        .app_key
                        .as_ref()
                        .and_then(|k| crate::crypto::decrypt(&r.value, k));
                    json!({ "id": r.id, "key": r.key, "value": plain })
                })
                .collect();
            ok_json(out)
        }
        Err(e) => query_err("list_team_envs", e),
    }
}

/// POST /api/v1/team/envs
pub async fn create_team_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Json(body): Json<EnvInput>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(app_key) = state.app_key.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"message": "Encryption key not configured."})),
        )
            .into_response();
    };
    let key = body.key.trim();
    if key.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "Key is required."})),
        )
            .into_response();
    }
    let Some(enc) = crate::crypto::encrypt(body.value.as_deref().unwrap_or(""), app_key) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": "Encryption failed."})),
        )
            .into_response();
    };
    match sqlx::query_scalar::<_, i64>(
        "INSERT INTO shared_environment_variables (key, value, type, team_id, created_at, updated_at) VALUES ($1,$2,'team',$3,now(),now()) RETURNING id",
    )
    .bind(key)
    .bind(&enc)
    .bind(team_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id, "key": key }))).into_response(),
        Err(e) => query_err("create_team_env", e),
    }
}

#[derive(Deserialize)]
pub struct SharedEnvUpdate {
    key: Option<String>,
    value: Option<String>,
}

/// PATCH /api/v1/team/envs/:id
pub async fn update_team_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<i64>,
    Json(body): Json<SharedEnvUpdate>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(app_key) = state.app_key.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"message": "Encryption key not configured."})),
        )
            .into_response();
    };
    // value is NOT NULL; only update it when provided.
    let enc = match body.value.as_deref() {
        Some(v) => match crate::crypto::encrypt(v, app_key) {
            Some(e) => Some(e),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"message": "Encryption failed."})),
                )
                    .into_response()
            }
        },
        None => None,
    };
    match sqlx::query(
        "UPDATE shared_environment_variables SET key = COALESCE($1, key), value = COALESCE($2, value), updated_at = now() \
         WHERE id = $3 AND team_id = $4 AND type = 'team'",
    )
    .bind(body.key.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&enc)
    .bind(id)
    .bind(team_id)
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Shared variable not found."),
        Ok(_) => (StatusCode::OK, Json(json!({ "id": id }))).into_response(),
        Err(e) => query_err("update_team_env", e),
    }
}

/// DELETE /api/v1/team/envs/:id
pub async fn delete_team_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<i64>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query(
        "DELETE FROM shared_environment_variables WHERE id = $1 AND team_id = $2 AND type = 'team'",
    )
    .bind(id)
    .bind(team_id)
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Shared variable not found."),
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"message": "Shared variable deleted."})),
        )
            .into_response(),
        Err(e) => query_err("delete_team_env", e),
    }
}
