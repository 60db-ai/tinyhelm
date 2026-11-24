//! Native `/api/v1` handlers: envs.

#[allow(unused_imports)]
use super::prelude::*;

// ---- application environment variables (Laravel-encrypted values) ----
pub(crate) const APP_RESOURCEABLE: &str = r"App\Models\Application";

/// Resolve an application's numeric id, team-scoped via environment -> project.
pub(crate) async fn app_id_for(pool: &PgPool, team_id: i64, uuid: &str) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT a.id FROM applications a JOIN environments e ON e.id = a.environment_id \
         JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1 AND a.uuid = $2",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

#[derive(sqlx::FromRow)]
pub(crate) struct EnvDbRow {
    uuid: String,
    key: String,
    value: Option<String>,
    is_buildtime: bool,
}

/// GET /api/v1/applications/:uuid/envs — values decrypted to plaintext.
pub async fn list_app_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else {
        return not_found("Application not found.");
    };
    let rows = sqlx::query_as::<_, EnvDbRow>(
        "SELECT uuid, key, value, is_buildtime FROM environment_variables \
         WHERE resourceable_type = $1 AND resourceable_id = $2 ORDER BY key",
    )
    .bind(APP_RESOURCEABLE)
    .bind(aid)
    .fetch_all(pool)
    .await;
    match rows {
        Ok(rs) => {
            let out: Vec<_> = rs
                .into_iter()
                .map(|r| {
                    let plain = match (r.value.as_deref(), state.app_key.as_ref()) {
                        (Some(v), Some(k)) => crate::crypto::decrypt(v, k),
                        _ => None,
                    };
                    json!({ "uuid": r.uuid, "key": r.key, "value": plain, "is_build_time": r.is_buildtime })
                })
                .collect();
            ok_json(out)
        }
        Err(e) => query_err("list_app_envs", e),
    }
}

#[derive(Deserialize)]
pub struct EnvInput {
    pub(crate) key: String,
    pub(crate) value: Option<String>,
    #[serde(default)]
    pub(crate) is_build_time: bool,
}

pub(crate) fn encrypt_opt(value: Option<&str>, key: &[u8; 32]) -> Result<Option<String>, Response> {
    match value {
        Some(v) => match crate::crypto::encrypt(v, key) {
            Some(e) => Ok(Some(e)),
            None => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"message": "Encryption failed."})),
            )
                .into_response()),
        },
        None => Ok(None),
    }
}

/// POST /api/v1/applications/:uuid/envs
pub async fn create_app_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else {
        return not_found("Application not found.");
    };
    let key = body.key.trim();
    if key.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "Key is required."})),
        )
            .into_response();
    }
    let enc = match encrypt_opt(body.value.as_deref(), app_key) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let env_uuid = cuid2::create_id();
    match sqlx::query(
        "INSERT INTO environment_variables (key, value, uuid, resourceable_type, resourceable_id, is_buildtime, is_runtime, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,now(),now())",
    )
    .bind(key)
    .bind(&enc)
    .bind(&env_uuid)
    .bind(APP_RESOURCEABLE)
    .bind(aid)
    .bind(body.is_build_time)
    .bind(!body.is_build_time)
    .execute(pool)
    .await
    {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "uuid": env_uuid, "key": key, "is_build_time": body.is_build_time }))).into_response(),
        Err(e) => query_err("create_app_env", e),
    }
}

/// PATCH /api/v1/applications/:uuid/envs — update a variable's value by key.
pub async fn update_app_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else {
        return not_found("Application not found.");
    };
    let enc = match encrypt_opt(body.value.as_deref(), app_key) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query(
        "UPDATE environment_variables SET value = $1, updated_at = now() \
         WHERE resourceable_type = $2 AND resourceable_id = $3 AND key = $4",
    )
    .bind(&enc)
    .bind(APP_RESOURCEABLE)
    .bind(aid)
    .bind(body.key.trim())
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Variable not found."),
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "key": body.key.trim(), "is_build_time": body.is_build_time })),
        )
            .into_response(),
        Err(e) => query_err("update_app_env", e),
    }
}

/// DELETE /api/v1/applications/:uuid/envs/:env_uuid
pub async fn delete_app_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_uuid)): Path<(String, String)>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else {
        return not_found("Application not found.");
    };
    match sqlx::query("DELETE FROM environment_variables WHERE uuid = $1 AND resourceable_type = $2 AND resourceable_id = $3")
        .bind(&env_uuid)
        .bind(APP_RESOURCEABLE)
        .bind(aid)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Variable not found."),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Variable deleted."}))).into_response(),
        Err(e) => query_err("delete_app_env", e),
    }
}

// ---- service environment variables (same shape; different resourceable) ----
pub(crate) const SERVICE_RESOURCEABLE: &str = r"App\Models\Service";

pub(crate) async fn service_id_for(pool: &PgPool, team_id: i64, uuid: &str) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT s.id FROM services s JOIN environments e ON e.id = s.environment_id \
         JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1 AND s.uuid = $2",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// GET /api/v1/services/:uuid/envs
pub async fn list_service_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else {
        return not_found("Service not found.");
    };
    let rows = sqlx::query_as::<_, EnvDbRow>(
        "SELECT uuid, key, value, is_buildtime FROM environment_variables \
         WHERE resourceable_type = $1 AND resourceable_id = $2 ORDER BY key",
    )
    .bind(SERVICE_RESOURCEABLE)
    .bind(sid)
    .fetch_all(pool)
    .await;
    match rows {
        Ok(rs) => {
            let out: Vec<_> = rs
                .into_iter()
                .map(|r| {
                    let plain = match (r.value.as_deref(), state.app_key.as_ref()) {
                        (Some(v), Some(k)) => crate::crypto::decrypt(v, k),
                        _ => None,
                    };
                    json!({ "uuid": r.uuid, "key": r.key, "value": plain, "is_build_time": r.is_buildtime })
                })
                .collect();
            ok_json(out)
        }
        Err(e) => query_err("list_service_envs", e),
    }
}

/// POST /api/v1/services/:uuid/envs
pub async fn create_service_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else {
        return not_found("Service not found.");
    };
    let key = body.key.trim();
    if key.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "Key is required."})),
        )
            .into_response();
    }
    let enc = match encrypt_opt(body.value.as_deref(), app_key) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let env_uuid = cuid2::create_id();
    match sqlx::query(
        "INSERT INTO environment_variables (key, value, uuid, resourceable_type, resourceable_id, is_buildtime, is_runtime, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,now(),now())",
    )
    .bind(key)
    .bind(&enc)
    .bind(&env_uuid)
    .bind(SERVICE_RESOURCEABLE)
    .bind(sid)
    .bind(body.is_build_time)
    .bind(!body.is_build_time)
    .execute(pool)
    .await
    {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "uuid": env_uuid, "key": key, "is_build_time": body.is_build_time }))).into_response(),
        Err(e) => query_err("create_service_env", e),
    }
}

/// PATCH /api/v1/services/:uuid/envs
pub async fn update_service_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else {
        return not_found("Service not found.");
    };
    let enc = match encrypt_opt(body.value.as_deref(), app_key) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query(
        "UPDATE environment_variables SET value = $1, updated_at = now() \
         WHERE resourceable_type = $2 AND resourceable_id = $3 AND key = $4",
    )
    .bind(&enc)
    .bind(SERVICE_RESOURCEABLE)
    .bind(sid)
    .bind(body.key.trim())
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Variable not found."),
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "key": body.key.trim(), "is_build_time": body.is_build_time })),
        )
            .into_response(),
        Err(e) => query_err("update_service_env", e),
    }
}

/// DELETE /api/v1/services/:uuid/envs/:env_uuid
pub async fn delete_service_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_uuid)): Path<(String, String)>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else {
        return not_found("Service not found.");
    };
    match sqlx::query("DELETE FROM environment_variables WHERE uuid = $1 AND resourceable_type = $2 AND resourceable_id = $3")
        .bind(&env_uuid)
        .bind(SERVICE_RESOURCEABLE)
        .bind(sid)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Variable not found."),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Variable deleted."}))).into_response(),
        Err(e) => query_err("delete_service_env", e),
    }
}

// ---- database environment variables (resourceable_type is the specific engine model) ----
// Resolve a database uuid to (resourceable_type, id), team-scoped via environment -> project.
pub(crate) const DB_RESOURCEABLE_SQL: &str = r#"SELECT d.id, d.rtype FROM (
    SELECT id, environment_id, 'App\Models\StandalonePostgresql' AS rtype FROM standalone_postgresqls WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneMysql' FROM standalone_mysqls WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneMariadb' FROM standalone_mariadbs WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneMongodb' FROM standalone_mongodbs WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneRedis' FROM standalone_redis WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneKeydb' FROM standalone_keydbs WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneDragonfly' FROM standalone_dragonflies WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneClickhouse' FROM standalone_clickhouses WHERE uuid=$1
    UNION ALL SELECT id, environment_id, 'App\Models\StandaloneSqlite' FROM standalone_sqlites WHERE uuid=$1
) d JOIN environments e ON e.id = d.environment_id JOIN projects p ON p.id = e.project_id
WHERE p.team_id = $2 LIMIT 1"#;

pub(crate) async fn database_resourceable(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Option<(String, i64)> {
    let row = sqlx::query(DB_RESOURCEABLE_SQL)
        .bind(uuid)
        .bind(team_id)
        .fetch_optional(pool)
        .await
        .ok()??;
    Some((
        row.try_get::<String, _>("rtype").ok()?,
        row.try_get::<i64, _>("id").ok()?,
    ))
}

/// GET /api/v1/databases/:uuid/envs
pub async fn list_database_envs(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else {
        return not_found("Database not found.");
    };
    let rows = sqlx::query_as::<_, EnvDbRow>(
        "SELECT uuid, key, value, is_buildtime FROM environment_variables WHERE resourceable_type = $1 AND resourceable_id = $2 ORDER BY key",
    )
    .bind(&rtype)
    .bind(rid)
    .fetch_all(pool)
    .await;
    match rows {
        Ok(rs) => {
            let out: Vec<_> = rs
                .into_iter()
                .map(|r| {
                    let plain = match (r.value.as_deref(), state.app_key.as_ref()) {
                        (Some(v), Some(k)) => crate::crypto::decrypt(v, k),
                        _ => None,
                    };
                    json!({ "uuid": r.uuid, "key": r.key, "value": plain, "is_build_time": r.is_buildtime })
                })
                .collect();
            ok_json(out)
        }
        Err(e) => query_err("list_database_envs", e),
    }
}

/// POST /api/v1/databases/:uuid/envs
pub async fn create_database_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else {
        return not_found("Database not found.");
    };
    let key = body.key.trim();
    if key.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "Key is required."})),
        )
            .into_response();
    }
    let enc = match encrypt_opt(body.value.as_deref(), app_key) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let env_uuid = cuid2::create_id();
    match sqlx::query(
        "INSERT INTO environment_variables (key, value, uuid, resourceable_type, resourceable_id, is_buildtime, is_runtime, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,now(),now())",
    )
    .bind(key)
    .bind(&enc)
    .bind(&env_uuid)
    .bind(&rtype)
    .bind(rid)
    .bind(body.is_build_time)
    .bind(!body.is_build_time)
    .execute(pool)
    .await
    {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "uuid": env_uuid, "key": key, "is_build_time": body.is_build_time }))).into_response(),
        Err(e) => query_err("create_database_env", e),
    }
}

/// PATCH /api/v1/databases/:uuid/envs
pub async fn update_database_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else {
        return not_found("Database not found.");
    };
    let enc = match encrypt_opt(body.value.as_deref(), app_key) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query(
        "UPDATE environment_variables SET value = $1, updated_at = now() WHERE resourceable_type = $2 AND resourceable_id = $3 AND key = $4",
    )
    .bind(&enc)
    .bind(&rtype)
    .bind(rid)
    .bind(body.key.trim())
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Variable not found."),
        Ok(_) => (StatusCode::OK, Json(json!({ "key": body.key.trim(), "is_build_time": body.is_build_time }))).into_response(),
        Err(e) => query_err("update_database_env", e),
    }
}

/// DELETE /api/v1/databases/:uuid/envs/:env_uuid
pub async fn delete_database_env(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, env_uuid)): Path<(String, String)>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else {
        return not_found("Database not found.");
    };
    match sqlx::query("DELETE FROM environment_variables WHERE uuid = $1 AND resourceable_type = $2 AND resourceable_id = $3")
        .bind(&env_uuid)
        .bind(&rtype)
        .bind(rid)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Variable not found."),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Variable deleted."}))).into_response(),
        Err(e) => query_err("delete_database_env", e),
    }
}
