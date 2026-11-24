//! API authentication, mirroring Laravel's `api` route group in order:
//! `auth:sanctum` -> `api.token.team` -> `ApiAllowed` (+ `ApiIpAllowed`) -> `api.sensitive` -> `api.ability:<x>`.
//!
//! Native handlers only serve the case they replicate completely: a full-team token (no project scopes)
//! whose user is an admin or owner of the token's team, with no IP allow-list configured. Every other
//! request (members with access grants, scoped tokens, IP allow-lists) is forwarded to Laravel unchanged,
//! so behavior stays identical while the access-grant resolver is not ported yet.

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

use super::{audit, throttle};
use crate::AppState;

/// The authenticated caller of a native API request.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub user_id: i64,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub team_id: i64,
    pub token_id: i64,
    pub token_name: Option<String>,
    abilities: Vec<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub method: String,
    pub path: String,
}

impl Ctx {
    /// Sanctum `tokenCan`: the ability is listed, or the token has `*`.
    pub fn can(&self, ability: &str) -> bool {
        self.abilities.iter().any(|a| a == "*" || a == ability)
    }

    /// `api.sensitive`: root or read:sensitive, and admin of the token team (guaranteed by the guard).
    pub fn can_read_sensitive(&self) -> bool {
        self.can("root") || self.can("read:sensitive")
    }

    /// `api.ability:<ability>`: root passes; otherwise the token needs the ability.
    /// Returns the pool and team id, or the exact Laravel 403 (and records `api.auth.ability_denied`).
    pub async fn require<'a>(
        &self,
        state: &'a AppState,
        ability: &str,
    ) -> Result<(&'a PgPool, i64), Response> {
        let pool = state.pool.as_ref().ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"message": "Native DB not configured."})),
            )
                .into_response()
        })?;
        if self.can("root") || self.can(ability) {
            return Ok((pool, self.team_id));
        }
        audit::record(
            pool,
            self,
            "api.auth.ability_denied",
            json!({
                "required_abilities": [ability],
                "token_id": self.token_id,
                "reason": "Invalid ability provided.",
            }),
            "warning",
        )
        .await;
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "message": format!("Missing required permissions: {ability}") })),
        )
            .into_response())
    }
}

enum Resolution {
    Native(Box<Ctx>),
    /// Laravel must answer (member, scoped token, IP allow-list).
    Delegate,
    Unauthenticated,
    InvalidToken,
    ApiDisabled,
}

/// Middleware for every native API route.
pub async fn native_guard(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let Some(pool) = state.pool.clone() else {
        return crate::proxy(State(state), req).await;
    };
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string());

    match resolve(&pool, req.headers()).await {
        Resolution::Delegate => crate::proxy(State(state), req).await,
        Resolution::Unauthenticated => message(StatusCode::UNAUTHORIZED, "Unauthenticated."),
        Resolution::InvalidToken => message(StatusCode::UNAUTHORIZED, "Invalid token."),
        Resolution::ApiDisabled => (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "API is disabled."})),
        )
            .into_response(),
        Resolution::Native(ctx) => {
            let mut ctx = *ctx;
            ctx.ip = ip;
            ctx.user_agent = req
                .headers()
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.chars().take(200).collect());
            ctx.method = req.method().to_string();
            ctx.path = req.uri().path().trim_start_matches('/').to_string();

            let limit = throttle::check(&format!("user:{}", ctx.user_id));
            if !limit.allowed {
                return throttle::too_many(&limit);
            }
            req.extensions_mut().insert(ctx);
            let mut res = next.run(req).await;
            throttle::annotate(&mut res, &limit);
            res
        }
    }
}

fn message(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "message": msg }))).into_response()
}

async fn resolve(pool: &PgPool, headers: &HeaderMap) -> Resolution {
    let Some(raw) = bearer(headers) else {
        return Resolution::Unauthenticated;
    };
    let Some(token) = find_token(pool, raw).await else {
        return Resolution::Unauthenticated;
    };

    // api.token.team
    let Some(team_id) = token
        .team_id
        .as_deref()
        .and_then(|t| t.trim().parse::<i64>().ok())
    else {
        return Resolution::InvalidToken;
    };
    let role: Option<Option<String>> = sqlx::query_scalar(
        "SELECT role FROM team_user WHERE user_id = $1 AND team_id = $2 LIMIT 1",
    )
    .bind(token.user_id)
    .bind(team_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let Some(role) = role else {
        return Resolution::InvalidToken;
    };
    // Access-grant resolution (members, scoped tokens) is not ported: Laravel answers those.
    if !matches!(role.as_deref(), Some("admin") | Some("owner")) || token.has_scopes {
        return Resolution::Delegate;
    }

    // ApiAllowed + ApiIpAllowed (self-hosted; TinyHelm has no cloud mode).
    let settings =
        sqlx::query("SELECT is_api_enabled, allowed_ips FROM instance_settings WHERE id = 0")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if let Some(row) = settings {
        if row
            .try_get::<Option<bool>, _>("is_api_enabled")
            .ok()
            .flatten()
            == Some(false)
        {
            return Resolution::ApiDisabled;
        }
        let allowed: String = row
            .try_get::<Option<String>, _>("allowed_ips")
            .ok()
            .flatten()
            .unwrap_or_default();
        let allowed = allowed.trim();
        if !allowed.is_empty() && allowed != "0.0.0.0" {
            // ponytail: CIDR allow-list matching stays in Laravel until client_ip() semantics are ported.
            return Resolution::Delegate;
        }
    }

    // Sanctum marks the token as used on every authenticated request.
    let _ = sqlx::query("UPDATE personal_access_tokens SET last_used_at = now() WHERE id = $1")
        .bind(token.id)
        .execute(pool)
        .await;

    let user = sqlx::query("SELECT name, email FROM users WHERE id = $1")
        .bind(token.user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    Resolution::Native(Box::new(Ctx {
        user_id: token.user_id,
        user_name: user.as_ref().and_then(|u| u.try_get("name").ok()),
        user_email: user.as_ref().and_then(|u| u.try_get("email").ok()),
        team_id,
        token_id: token.id,
        token_name: token.name,
        abilities: token.abilities,
        ip: None,
        user_agent: None,
        method: String::new(),
        path: String::new(),
    }))
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let header = headers.get("authorization")?.to_str().ok()?;
    let (scheme, value) = header.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| value.trim())
        .filter(|v| !v.is_empty())
}

struct TokenRow {
    id: i64,
    user_id: i64,
    name: Option<String>,
    team_id: Option<String>,
    abilities: Vec<String>,
    has_scopes: bool,
}

/// Sanctum `PersonalAccessToken::findToken`: "<id>|<plaintext>" looks up by id and compares hashes;
/// a bare plaintext token looks up by hash.
async fn find_token(pool: &PgPool, raw: &str) -> Option<TokenRow> {
    const COLUMNS: &str = "id, tokenable_type, tokenable_id, name, token, team_id, abilities, expires_at, paused_at, \
                           scopes IS NOT NULL AS has_scopes";
    let (row, plaintext) = match raw.split_once('|') {
        Some((id, plaintext)) => {
            let id: i64 = id.parse().ok()?;
            let row = sqlx::query(&format!(
                "SELECT {COLUMNS} FROM personal_access_tokens WHERE id = $1"
            ))
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()??;
            (row, plaintext)
        }
        None => {
            let hash = hex::encode(Sha256::digest(raw.as_bytes()));
            let row = sqlx::query(&format!(
                "SELECT {COLUMNS} FROM personal_access_tokens WHERE token = $1"
            ))
            .bind(hash)
            .fetch_optional(pool)
            .await
            .ok()??;
            (row, raw)
        }
    };

    let stored: String = row.try_get("token").ok()?;
    let given = hex::encode(Sha256::digest(plaintext.as_bytes()));
    if !crate::crypto::constant_eq(stored.as_bytes(), given.as_bytes()) {
        return None;
    }
    if row.try_get::<String, _>("tokenable_type").ok()? != r"App\Models\User" {
        return None;
    }
    let now = chrono::Utc::now().naive_utc();
    let expired = row
        .try_get::<Option<chrono::NaiveDateTime>, _>("expires_at")
        .ok()
        .flatten()
        .is_some_and(|exp| exp <= now);
    let paused = row
        .try_get::<Option<chrono::NaiveDateTime>, _>("paused_at")
        .ok()
        .flatten()
        .is_some();
    if expired || paused {
        return None;
    }

    let abilities = row
        .try_get::<Option<String>, _>("abilities")
        .ok()
        .flatten()
        .and_then(|a| serde_json::from_str::<Vec<String>>(&a).ok())
        .unwrap_or_default();

    Some(TokenRow {
        id: row.try_get("id").ok()?,
        user_id: row.try_get("tokenable_id").ok()?,
        name: row.try_get("name").ok(),
        team_id: row.try_get("team_id").ok().flatten(),
        abilities,
        // Fail closed: if the column cannot be read, treat the token as scoped (Laravel answers).
        has_scopes: row.try_get("has_scopes").unwrap_or(true),
    })
}

#[cfg(test)]
pub mod tests_support {
    use super::Ctx;

    pub fn ctx_with(abilities: &[&str]) -> Ctx {
        Ctx {
            user_id: 1,
            user_name: Some("Tester".into()),
            user_email: Some("tester@example.test".into()),
            team_id: 1,
            token_id: 7,
            token_name: Some("t".into()),
            abilities: abilities.iter().map(|s| s.to_string()).collect(),
            ip: None,
            user_agent: None,
            method: "GET".into(),
            path: "api/v1/x".into(),
        }
    }

    pub fn ctx() -> Ctx {
        ctx_with(&["*"])
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::ctx_with;
    use super::*;

    #[test]
    fn star_token_can_everything() {
        assert!(ctx_with(&["*"]).can("write"));
        assert!(ctx_with(&["*"]).can_read_sensitive());
    }

    #[test]
    fn read_token_cannot_write_or_read_sensitive() {
        let c = ctx_with(&["read"]);
        assert!(c.can("read"));
        assert!(!c.can("write"));
        assert!(!c.can_read_sensitive());
    }

    #[test]
    fn root_reads_sensitive_and_can_is_literal() {
        let c = ctx_with(&["root"]);
        assert!(c.can_read_sensitive());
        assert!(!c.can("write")); // the root bypass lives in require(), like ApiAbility
    }

    #[test]
    fn bearer_parsing() {
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer 2|abc".parse().unwrap());
        assert_eq!(bearer(&h), Some("2|abc"));
        h.insert("authorization", "Basic xyz".parse().unwrap());
        assert_eq!(bearer(&h), None);
    }
}
