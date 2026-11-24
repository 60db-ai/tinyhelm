//! Phase 3: native Rust endpoints — EXPERIMENTAL, feature-flagged via RUST_NATIVE_ROUTES.
//!
//! Replicates Laravel's Sanctum token auth + team scoping against the same Postgres. This is the
//! migration pattern (read endpoint ported to Rust behind the gateway), NOT production-trusted:
//! enable only after diffing responses against Laravel and a security review of the auth path.

use axum::{extract::State, http::HeaderMap, http::StatusCode, response::{IntoResponse, Response}, Json};
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
}

/// Validate a Sanctum bearer token: "<id>|<plaintext>" (column stores sha256(plaintext) hex).
async fn authenticate(pool: &PgPool, headers: &HeaderMap) -> Option<TokenAuth> {
    let header = headers.get("authorization")?.to_str().ok()?;
    let raw = header.strip_prefix("Bearer ").or_else(|| header.strip_prefix("bearer "))?.trim();
    let plaintext = raw.split_once('|').map(|(_, t)| t).unwrap_or(raw);
    let hash = hex::encode(Sha256::digest(plaintext.as_bytes()));

    let row = sqlx::query(
        "SELECT team_id, abilities, expires_at, paused_at FROM personal_access_tokens WHERE token = $1 LIMIT 1",
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
