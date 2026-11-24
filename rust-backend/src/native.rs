//! Phase 3: native Rust endpoints — EXPERIMENTAL, feature-flagged via RUST_NATIVE_ROUTES.
//!
//! Replicates Laravel's Sanctum token auth + team scoping against the same Postgres. This is the
//! migration pattern (read endpoint ported to Rust behind the gateway), NOT production-trusted:
//! enable only after diffing responses against Laravel and a security review of the auth path.

use axum::{extract::{Path, State}, http::HeaderMap, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::{Deserialize, Serialize};
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

/// Validate token + read ability; return pool + team id (or an error response).
async fn authorize_read<'a>(state: &'a crate::AppState, headers: &HeaderMap) -> Result<(&'a PgPool, i64), Response> {
    authorize_with(state, headers, "read").await
}

/// Validate token + the required ability + admin/owner role; return pool + team id (or an error response).
async fn authorize_with<'a>(state: &'a crate::AppState, headers: &HeaderMap, ability: &str) -> Result<(&'a PgPool, i64), Response> {
    let Some(pool) = state.pool.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Native DB not configured."}))).into_response());
    };
    let Some(auth) = authenticate(pool, headers).await else {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"message": "Unauthenticated."}))).into_response());
    };
    if !has_ability(&auth.abilities, ability) {
        return Err((StatusCode::FORBIDDEN, Json(json!({ "message": format!("Missing {ability} ability.") }))).into_response());
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

// ---- project writes (pure DB; no deploy engine needed) ----
#[derive(Deserialize)]
pub struct CreateProject {
    name: String,
    description: Option<String>,
}

async fn fetch_project(pool: &PgPool, team_id: i64, uuid: &str) -> Result<Option<ProjectRow>, sqlx::Error> {
    sqlx::query_as::<_, ProjectRow>("SELECT uuid, name, description FROM projects WHERE team_id = $1 AND uuid = $2 LIMIT 1")
        .bind(team_id)
        .bind(uuid)
        .fetch_optional(pool)
        .await
}

/// POST /api/v1/projects — create a project and its default `production` environment (matches Laravel).
pub async fn create_project(State(state): State<crate::AppState>, headers: HeaderMap, Json(body): Json<CreateProject>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let name = body.name.trim();
    if name.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"message": "Name is required."}))).into_response();
    }
    let project_uuid = cuid2::create_id();
    let env_uuid = cuid2::create_id();

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return query_err("create_project.begin", e),
    };
    let project_id: i64 = match sqlx::query_scalar(
        "INSERT INTO projects (uuid, name, description, team_id, created_at, updated_at) VALUES ($1,$2,$3,$4,now(),now()) RETURNING id",
    )
    .bind(&project_uuid)
    .bind(name)
    .bind(&body.description)
    .bind(team_id)
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            let _ = tx.rollback().await;
            return query_err("create_project", e);
        }
    };
    if let Err(e) = sqlx::query(
        "INSERT INTO environments (uuid, name, project_id, created_at, updated_at) VALUES ($1,'production',$2,now(),now())",
    )
    .bind(&env_uuid)
    .bind(project_id)
    .execute(&mut *tx)
    .await
    {
        let _ = tx.rollback().await;
        return query_err("create_project.environment", e);
    }
    if let Err(e) = tx.commit().await {
        return query_err("create_project.commit", e);
    }

    (
        StatusCode::CREATED,
        Json(json!({ "uuid": project_uuid, "name": name, "description": body.description })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct UpdateProject {
    name: Option<String>,
    description: Option<String>,
}

/// PATCH /api/v1/projects/:uuid — update name/description.
pub async fn update_project(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<UpdateProject>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let res = sqlx::query(
        "UPDATE projects SET name = COALESCE($1, name), description = COALESCE($2, description), updated_at = now() WHERE uuid = $3 AND team_id = $4",
    )
    .bind(body.name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&body.description)
    .bind(&uuid)
    .bind(team_id)
    .execute(pool)
    .await;
    match res {
        Ok(r) if r.rows_affected() == 0 => not_found("Project not found."),
        Ok(_) => match fetch_project(pool, team_id, &uuid).await {
            Ok(Some(p)) => (StatusCode::OK, Json(p)).into_response(),
            Ok(None) => not_found("Project not found."),
            Err(e) => query_err("update_project.fetch", e),
        },
        Err(e) => query_err("update_project", e),
    }
}

// Counts resources across a project's environments (apps + services + every standalone db engine).
const PROJECT_RESOURCE_COUNT: &str = "SELECT \
 (SELECT count(*) FROM applications WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM services WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_postgresqls WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_mysqls WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_mariadbs WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_mongodbs WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_redis WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_keydbs WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_dragonflies WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_clickhouses WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_sqlites WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) AS total";

/// DELETE /api/v1/projects/:uuid — refuses if the project still has resources (data-loss guard).
pub async fn delete_project(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "delete").await { Ok(v) => v, Err(r) => return r };
    let pid: Option<i64> = sqlx::query_scalar("SELECT id FROM projects WHERE uuid = $1 AND team_id = $2")
        .bind(&uuid)
        .bind(team_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    let Some(pid) = pid else { return not_found("Project not found.") };

    let total: i64 = match sqlx::query_scalar(PROJECT_RESOURCE_COUNT).bind(pid).fetch_one(pool).await {
        Ok(n) => n,
        Err(e) => return query_err("delete_project.count", e),
    };
    if total > 0 {
        return (StatusCode::CONFLICT, Json(json!({"message": "Project has resources; delete them first."}))).into_response();
    }

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return query_err("delete_project.begin", e),
    };
    if let Err(e) = sqlx::query("DELETE FROM environments WHERE project_id = $1").bind(pid).execute(&mut *tx).await {
        let _ = tx.rollback().await;
        return query_err("delete_project.environments", e);
    }
    if let Err(e) = sqlx::query("DELETE FROM projects WHERE id = $1").bind(pid).execute(&mut *tx).await {
        let _ = tx.rollback().await;
        return query_err("delete_project", e);
    }
    if let Err(e) = tx.commit().await {
        return query_err("delete_project.commit", e);
    }
    (StatusCode::OK, Json(json!({"message": "Project deleted."}))).into_response()
}

// ---- tags (pure DB) ----
#[derive(Serialize, sqlx::FromRow)]
struct TagRow {
    uuid: String,
    name: String,
}

/// GET /api/v1/tags
pub async fn list_tags(State(state): State<crate::AppState>, headers: HeaderMap) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    match sqlx::query_as::<_, TagRow>("SELECT uuid, name FROM tags WHERE team_id = $1 ORDER BY name")
        .bind(team_id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_tags", e),
    }
}

#[derive(Deserialize)]
pub struct CreateTag {
    name: String,
}

/// POST /api/v1/tags
pub async fn create_tag(State(state): State<crate::AppState>, headers: HeaderMap, Json(body): Json<CreateTag>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let name = body.name.trim();
    if name.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"message": "Name is required."}))).into_response();
    }
    let uuid = cuid2::create_id();
    match sqlx::query("INSERT INTO tags (uuid, name, team_id, created_at, updated_at) VALUES ($1,$2,$3,now(),now())")
        .bind(&uuid)
        .bind(name)
        .bind(team_id)
        .execute(pool)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "uuid": uuid, "name": name }))).into_response(),
        Err(e) => query_err("create_tag", e),
    }
}

/// DELETE /api/v1/tags/:uuid
pub async fn delete_tag(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "delete").await { Ok(v) => v, Err(r) => return r };
    match sqlx::query("DELETE FROM tags WHERE uuid = $1 AND team_id = $2").bind(&uuid).bind(team_id).execute(pool).await {
        Ok(r) if r.rows_affected() == 0 => not_found("Tag not found."),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Tag deleted."}))).into_response(),
        Err(e) => query_err("delete_tag", e),
    }
}

// ---- application environment variables (Laravel-encrypted values) ----
const APP_RESOURCEABLE: &str = r"App\Models\Application";

/// Resolve an application's numeric id, team-scoped via environment -> project.
async fn app_id_for(pool: &PgPool, team_id: i64, uuid: &str) -> Option<i64> {
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
struct EnvDbRow {
    uuid: String,
    key: String,
    value: Option<String>,
    is_buildtime: bool,
}

/// GET /api/v1/applications/:uuid/envs — values decrypted to plaintext.
pub async fn list_app_envs(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else { return not_found("Application not found.") };
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
    key: String,
    value: Option<String>,
    #[serde(default)]
    is_build_time: bool,
}

fn encrypt_opt(value: Option<&str>, key: &[u8; 32]) -> Result<Option<String>, Response> {
    match value {
        Some(v) => match crate::crypto::encrypt(v, key) {
            Some(e) => Ok(Some(e)),
            None => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"message": "Encryption failed."}))).into_response()),
        },
        None => Ok(None),
    }
}

/// POST /api/v1/applications/:uuid/envs
pub async fn create_app_env(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<EnvInput>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let Some(app_key) = state.app_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Encryption key not configured."}))).into_response();
    };
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else { return not_found("Application not found.") };
    let key = body.key.trim();
    if key.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"message": "Key is required."}))).into_response();
    }
    let enc = match encrypt_opt(body.value.as_deref(), app_key) { Ok(v) => v, Err(r) => return r };
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
pub async fn update_app_env(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<EnvInput>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let Some(app_key) = state.app_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Encryption key not configured."}))).into_response();
    };
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else { return not_found("Application not found.") };
    let enc = match encrypt_opt(body.value.as_deref(), app_key) { Ok(v) => v, Err(r) => return r };
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
        Ok(_) => (StatusCode::OK, Json(json!({ "key": body.key.trim(), "is_build_time": body.is_build_time }))).into_response(),
        Err(e) => query_err("update_app_env", e),
    }
}

/// DELETE /api/v1/applications/:uuid/envs/:env_uuid
pub async fn delete_app_env(State(state): State<crate::AppState>, headers: HeaderMap, Path((uuid, env_uuid)): Path<(String, String)>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "delete").await { Ok(v) => v, Err(r) => return r };
    let Some(aid) = app_id_for(pool, team_id, &uuid).await else { return not_found("Application not found.") };
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
const SERVICE_RESOURCEABLE: &str = r"App\Models\Service";

async fn service_id_for(pool: &PgPool, team_id: i64, uuid: &str) -> Option<i64> {
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
pub async fn list_service_envs(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else { return not_found("Service not found.") };
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
pub async fn create_service_env(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<EnvInput>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let Some(app_key) = state.app_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Encryption key not configured."}))).into_response();
    };
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else { return not_found("Service not found.") };
    let key = body.key.trim();
    if key.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"message": "Key is required."}))).into_response();
    }
    let enc = match encrypt_opt(body.value.as_deref(), app_key) { Ok(v) => v, Err(r) => return r };
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
pub async fn update_service_env(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<EnvInput>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let Some(app_key) = state.app_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Encryption key not configured."}))).into_response();
    };
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else { return not_found("Service not found.") };
    let enc = match encrypt_opt(body.value.as_deref(), app_key) { Ok(v) => v, Err(r) => return r };
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
        Ok(_) => (StatusCode::OK, Json(json!({ "key": body.key.trim(), "is_build_time": body.is_build_time }))).into_response(),
        Err(e) => query_err("update_service_env", e),
    }
}

/// DELETE /api/v1/services/:uuid/envs/:env_uuid
pub async fn delete_service_env(State(state): State<crate::AppState>, headers: HeaderMap, Path((uuid, env_uuid)): Path<(String, String)>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "delete").await { Ok(v) => v, Err(r) => return r };
    let Some(sid) = service_id_for(pool, team_id, &uuid).await else { return not_found("Service not found.") };
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
const DB_RESOURCEABLE_SQL: &str = r#"SELECT d.id, d.rtype FROM (
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

async fn database_resourceable(pool: &PgPool, team_id: i64, uuid: &str) -> Option<(String, i64)> {
    let row = sqlx::query(DB_RESOURCEABLE_SQL).bind(uuid).bind(team_id).fetch_optional(pool).await.ok()??;
    Some((row.try_get::<String, _>("rtype").ok()?, row.try_get::<i64, _>("id").ok()?))
}

/// GET /api/v1/databases/:uuid/envs
pub async fn list_database_envs(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>) -> Response {
    let (pool, team_id) = match authorize_read(&state, &headers).await { Ok(v) => v, Err(r) => return r };
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else { return not_found("Database not found.") };
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
pub async fn create_database_env(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<EnvInput>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let Some(app_key) = state.app_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Encryption key not configured."}))).into_response();
    };
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else { return not_found("Database not found.") };
    let key = body.key.trim();
    if key.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"message": "Key is required."}))).into_response();
    }
    let enc = match encrypt_opt(body.value.as_deref(), app_key) { Ok(v) => v, Err(r) => return r };
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
pub async fn update_database_env(State(state): State<crate::AppState>, headers: HeaderMap, Path(uuid): Path<String>, Json(body): Json<EnvInput>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "write").await { Ok(v) => v, Err(r) => return r };
    let Some(app_key) = state.app_key.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"message": "Encryption key not configured."}))).into_response();
    };
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else { return not_found("Database not found.") };
    let enc = match encrypt_opt(body.value.as_deref(), app_key) { Ok(v) => v, Err(r) => return r };
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
pub async fn delete_database_env(State(state): State<crate::AppState>, headers: HeaderMap, Path((uuid, env_uuid)): Path<(String, String)>) -> Response {
    let (pool, team_id) = match authorize_with(&state, &headers, "delete").await { Ok(v) => v, Err(r) => return r };
    let Some((rtype, rid)) = database_resourceable(pool, team_id, &uuid).await else { return not_found("Database not found.") };
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
