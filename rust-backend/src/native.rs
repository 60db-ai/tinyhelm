//! Phase 3: native Rust endpoints — EXPERIMENTAL, feature-flagged via RUST_NATIVE_ROUTES.
//!
//! Replicates Laravel's Sanctum token auth + team scoping against the same Postgres. This is the
//! migration pattern (read endpoint ported to Rust behind the gateway), NOT production-trusted:
//! enable only after diffing responses against Laravel and a security review of the auth path.

use axum::{extract::{Path, State}, http::HeaderMap, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

/// Build a pool from DATABASE_URL (same database Laravel uses). None if unset/unreachable.
pub async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    match PgPoolOptions::new().max_connections(5).connect(&url).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::error!("native db connect failed: {e}");
            None
        }
    }
}

struct TokenAuth {
    team_id: String,
    abilities: String,
    user_id: i64,
}

/// Validate a Sanctum bearer token: "<id>|<plaintext>" (column stores sha256(plaintext) hex).
async fn authenticate(pool: &PgPool, headers: &HeaderMap) -> Option<TokenAuth> {
    let header = headers.get("authorization")?.to_str().ok()?;
    let raw = header.strip_prefix("Bearer ").or_else(|| header.strip_prefix("bearer "))?.trim();
    let plaintext = raw.split_once('|').map(|(_, t)| t).unwrap_or(raw);
    let hash = hex::encode(Sha256::digest(plaintext.as_bytes()));

    let row = sqlx::query(
        "SELECT team_id, abilities, tokenable_id, expires_at, paused_at FROM personal_access_tokens WHERE token = $1 LIMIT 1",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .ok()??;

    let expires: Option<chrono::NaiveDateTime> = row.try_get("expires_at").ok().flatten();
    if let Some(exp) = expires {
        if exp < chrono::Utc::now().naive_utc() {
            return None;
        }
    }
    let paused: Option<chrono::NaiveDateTime> = row.try_get("paused_at").ok().flatten();
    if paused.is_some() {
        return None;
    }

    Some(TokenAuth {
        team_id: row.try_get::<String, _>("team_id").ok()?,
        abilities: row.try_get::<Option<String>, _>("abilities").ok().flatten().unwrap_or_default(),
        user_id: row.try_get::<i64, _>("tokenable_id").ok()?,
    })
}

/// abilities is a JSON array, e.g. ["read","write"] or ["*"] / ["root"].
fn has_ability(abilities: &str, needed: &str) -> bool {
    serde_json::from_str::<Vec<String>>(abilities)
        .map(|a| a.iter().any(|x| x == "*" || x == "root" || x == needed))
        .unwrap_or(false)
}

#[derive(Serialize, sqlx::FromRow)]
struct ServerRow {
    uuid: String,
    name: String,
    description: Option<String>,
    ip: String,
    port: i32,
    #[sqlx(rename = "user")]
    user: String,
}

/// Validate token + read ability, returning the pool and the token's team id (or an error response).
async fn authorize_read<'a>(state: &'a crate::AppState, headers: &HeaderMap) -> Result<(&'a PgPool, i64), Response> {
    let Some(pool) = state.pool.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Native DB not configured."}))).into_response());
    };
    let Some(auth) = authenticate(pool, headers).await else {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"message": "Unauthenticated."}))).into_response());
    };
    if !has_ability(&auth.abilities, "read") {
        return Err((StatusCode::FORBIDDEN, Json(json!({"message": "Missing read ability."}))).into_response());
    }
    let Ok(team_id) = auth.team_id.parse::<i64>() else {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"message": "Invalid token team."}))).into_response());
    };

    // Safety gate: native endpoints scope by team only (no access-grant filtering yet), so restrict
    // them to full-team (admin/owner) tokens. Scoped members must use the Laravel path (flag off).
    // Fail-closed: unknown/missing role -> denied.
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM team_user WHERE user_id = $1 AND team_id = $2 LIMIT 1")
        .bind(auth.user_id)
        .bind(team_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    if !matches!(role.as_deref(), Some("admin") | Some("owner")) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"message": "Native routes require an admin/owner token (access-grant scoping not yet ported)."}))).into_response());
    }

    Ok((pool, team_id))
}

/// GET /api/v1/servers — team-scoped, mirrors the Laravel endpoint's core fields.
pub async fn list_servers(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ServerRow>(
        r#"SELECT uuid, name, description, ip, port, "user" FROM servers WHERE team_id = $1 ORDER BY id"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap_or(json!([])))).into_response(),
        Err(e) => {
            tracing::error!("list_servers query failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(json!({"message": "Query failed."}))).into_response()
        }
    }
}

/// GET /api/v1/servers/:uuid — single server, team-scoped.
pub async fn get_server(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ServerRow>(
        r#"SELECT uuid, name, description, ip, port, "user" FROM servers WHERE team_id = $1 AND uuid = $2 LIMIT 1"#,
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"message": "Server not found."}))).into_response(),
        Err(e) => {
            tracing::error!("get_server query failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(json!({"message": "Query failed."}))).into_response()
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
struct ProjectRow {
    uuid: String,
    name: String,
    description: Option<String>,
}

/// GET /api/v1/projects — team-scoped.
pub async fn list_projects(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ProjectRow>(
        "SELECT uuid, name, description FROM projects WHERE team_id = $1 ORDER BY id",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap_or(json!([])))).into_response(),
        Err(e) => {
            tracing::error!("list_projects query failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(json!({"message": "Query failed."}))).into_response()
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
struct ApplicationRow {
    uuid: String,
    name: String,
    description: Option<String>,
    fqdn: Option<String>,
    status: String,
    git_repository: Option<String>,
    git_branch: Option<String>,
    build_pack: Option<String>,
}

/// GET /api/v1/applications — team-scoped via environment -> project.
///
/// SECURITY: this does NOT yet replicate Laravel's access-grant filtering (ScopedByAccessGrants),
/// so a project-scoped member's token would see ALL of the team's applications. Must port the
/// access-grant scope before enabling this route for anyone but full-team (admin/owner) tokens.
pub async fn list_applications(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT a.uuid, a.name, a.description, a.fqdn, a.status,
                  a.git_repository, a.git_branch, a.build_pack
           FROM applications a
           JOIN environments e ON e.id = a.environment_id
           JOIN projects p ON p.id = e.project_id
           WHERE p.team_id = $1
           ORDER BY a.id"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap_or(json!([])))).into_response(),
        Err(e) => {
            tracing::error!("list_applications query failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(json!({"message": "Query failed."}))).into_response()
        }
    }
}

/// GET /api/v1/applications/:uuid — single application, team-scoped via environment -> project.
pub async fn get_application(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT a.uuid, a.name, a.description, a.fqdn, a.status,
                  a.git_repository, a.git_branch, a.build_pack
           FROM applications a
           JOIN environments e ON e.id = a.environment_id
           JOIN projects p ON p.id = e.project_id
           WHERE p.team_id = $1 AND a.uuid = $2 LIMIT 1"#,
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"message": "Application not found."}))).into_response(),
        Err(e) => {
            tracing::error!("get_application query failed: {e}");
            (StatusCode::BAD_GATEWAY, Json(json!({"message": "Query failed."}))).into_response()
        }
    }
}

// ---- shared response helpers ----
fn ok_json<T: Serialize>(rows: T) -> Response {
    (StatusCode::OK, Json(serde_json::to_value(rows).unwrap_or(json!([])))).into_response()
}
fn not_found(msg: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "message": msg }))).into_response()
}
fn query_err(what: &str, e: sqlx::Error) -> Response {
    tracing::error!("{what} query failed: {e}");
    (StatusCode::BAD_GATEWAY, Json(json!({"message": "Query failed."}))).into_response()
}

// ---- services ----
#[derive(Serialize, sqlx::FromRow)]
struct ServiceRow {
    uuid: String,
    name: String,
    description: Option<String>,
}

const SERVICE_SELECT: &str = r#"SELECT s.uuid, s.name, s.description
    FROM services s
    JOIN environments e ON e.id = s.environment_id
    JOIN projects p ON p.id = e.project_id"#;

/// GET /api/v1/services
pub async fn list_services(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let sql = format!("{SERVICE_SELECT} WHERE p.team_id = $1 ORDER BY s.id");
    match sqlx::query_as::<_, ServiceRow>(&sql).bind(team_id).fetch_all(pool).await {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_services", e),
    }
}

/// GET /api/v1/services/:uuid
pub async fn get_service(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let sql = format!("{SERVICE_SELECT} WHERE p.team_id = $1 AND s.uuid = $2 LIMIT 1");
    match sqlx::query_as::<_, ServiceRow>(&sql).bind(team_id).bind(&uuid).fetch_optional(pool).await {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => not_found("Service not found."),
        Err(e) => query_err("get_service", e),
    }
}

// ---- databases (union across the 9 standalone engines) ----
#[derive(Serialize, sqlx::FromRow)]
struct DatabaseRow {
    uuid: String,
    name: String,
    description: Option<String>,
    status: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    db_type: String,
}

const DATABASE_UNION: &str = r#"
    SELECT uuid,name,description,status,environment_id,'postgresql' AS type FROM standalone_postgresqls
    UNION ALL SELECT uuid,name,description,status,environment_id,'mysql' FROM standalone_mysqls
    UNION ALL SELECT uuid,name,description,status,environment_id,'mariadb' FROM standalone_mariadbs
    UNION ALL SELECT uuid,name,description,status,environment_id,'mongodb' FROM standalone_mongodbs
    UNION ALL SELECT uuid,name,description,status,environment_id,'redis' FROM standalone_redis
    UNION ALL SELECT uuid,name,description,status,environment_id,'keydb' FROM standalone_keydbs
    UNION ALL SELECT uuid,name,description,status,environment_id,'dragonfly' FROM standalone_dragonflies
    UNION ALL SELECT uuid,name,description,status,environment_id,'clickhouse' FROM standalone_clickhouses
    UNION ALL SELECT uuid,name,description,status,environment_id,'sqlite' FROM standalone_sqlites"#;

/// GET /api/v1/databases
pub async fn list_databases(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let sql = format!(
        "SELECT d.uuid, d.name, d.description, d.status, d.type FROM ({DATABASE_UNION}) d \
         JOIN environments e ON e.id = d.environment_id JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 ORDER BY d.name"
    );
    match sqlx::query_as::<_, DatabaseRow>(&sql).bind(team_id).fetch_all(pool).await {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_databases", e),
    }
}

/// GET /api/v1/databases/:uuid
pub async fn get_database(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let sql = format!(
        "SELECT d.uuid, d.name, d.description, d.status, d.type FROM ({DATABASE_UNION}) d \
         JOIN environments e ON e.id = d.environment_id JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND d.uuid = $2 LIMIT 1"
    );
    match sqlx::query_as::<_, DatabaseRow>(&sql).bind(team_id).bind(&uuid).fetch_optional(pool).await {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => not_found("Database not found."),
        Err(e) => query_err("get_database", e),
    }
}

// ---- environments (by project) ----
#[derive(Serialize, sqlx::FromRow)]
struct EnvironmentRow {
    uuid: String,
    name: String,
}

/// GET /api/v1/projects/:uuid/environments
pub async fn list_environments(State(state): State<crate::AppState>, headers: HeaderMap, Path(project_uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    match sqlx::query_as::<_, EnvironmentRow>(
        r#"SELECT e.uuid, e.name FROM environments e JOIN projects p ON p.id = e.project_id
           WHERE p.uuid = $1 AND p.team_id = $2 ORDER BY e.id"#,
    )
    .bind(&project_uuid)
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_environments", e),
    }
}

// ---- current team ----
#[derive(Serialize, sqlx::FromRow)]
struct TeamRow {
    id: i64,
    name: Option<String>,
}

/// GET /api/v1/team — the token's team.
pub async fn get_current_team(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
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
