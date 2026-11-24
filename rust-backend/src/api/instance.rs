//! Native instance endpoints (Laravel `OtherController`): health, version, realtime config, and the
//! instance-admin switches for the API and the MCP server.

#[allow(unused_imports)]
use super::prelude::*;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::extract::ConnectInfo;

use super::throttle;

/// Laravel `env()`: the process environment first, then the app's `.env` file
/// (`TINYHELM_LARAVEL_ENV`, default `../.env` next to `rust-backend/`). `null`/`(null)` mean unset.
pub fn laravel_env(key: &str) -> Option<String> {
    let raw = std::env::var(key).ok().or_else(|| {
        let path = std::env::var("TINYHELM_LARAVEL_ENV").unwrap_or_else(|_| "../.env".into());
        std::fs::read_to_string(path)
            .ok()?
            .lines()
            .find_map(|line| {
                let line = line.trim();
                let (k, v) = line
                    .strip_prefix("export ")
                    .unwrap_or(line)
                    .split_once('=')?;
                (k.trim() == key).then(|| {
                    let v = v.trim();
                    let unquoted = v
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));
                    unquoted
                        .unwrap_or_else(|| v.split(" #").next().unwrap_or(v))
                        .to_string()
                })
            })
    })?;
    match raw.to_lowercase().as_str() {
        "null" | "(null)" => None,
        "empty" | "(empty)" => Some(String::new()),
        _ => Some(raw),
    }
}

/// `config('constants.tinyhelm.version')`: `TINYHELM_VERSION` or `release.json` `stable.tinyhelm`.
fn version() -> String {
    if let Some(v) = laravel_env("TINYHELM_VERSION").filter(|v| !v.is_empty()) {
        return v;
    }
    let path = std::env::var("TINYHELM_RELEASE_FILE").unwrap_or_else(|_| "../release.json".into());
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|j| j["stable"]["tinyhelm"].as_str().map(str::to_string))
        .unwrap_or_default()
}

fn text(status: StatusCode, body: String) -> Response {
    (status, [("content-type", "text/html; charset=UTF-8")], body).into_response()
}

/// GET /api/v1/version
pub async fn get_version(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    if let Err(r) = ctx.require(&state, "read").await {
        return r;
    }
    text(StatusCode::OK, version())
}

/// PHP `(int)` on a string: the leading integer, 0 when there is none.
fn php_int_cast(s: &str) -> i64 {
    let t = s.trim_start();
    let end = t
        .char_indices()
        .find(|(i, c)| !(c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+'))))
        .map_or(t.len(), |(i, _)| i);
    t[..end].parse().unwrap_or(0)
}

/// GET /api/v1/realtime/config
pub async fn get_realtime_config(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    if let Err(r) = ctx.require(&state, "read").await {
        return r;
    }
    ok_json(json!({
        "key": laravel_env("PUSHER_APP_KEY").unwrap_or_else(|| "tinyhelm".into()),
        "host": laravel_env("PUSHER_HOST"),
        "port": laravel_env("PUSHER_PORT").map_or(6001, |p| php_int_cast(&p)),
        "scheme": laravel_env("PUSHER_SCHEME").unwrap_or_else(|| "https".into()),
    }))
}

/// `OtherController::hasPendingMigrations()`: migration files under `database/migrations` that have
/// not run. ponytail: package migration paths are not scanned; read errors count as none pending.
async fn has_pending_migrations(pool: &PgPool) -> bool {
    let enabled = laravel_env("MIGRATION_ENABLED").map_or(true, |v| {
        !matches!(v.to_lowercase().as_str(), "false" | "0" | "" | "off" | "no")
    });
    if !enabled {
        return false;
    }
    let exists: bool = match sqlx::query_scalar("SELECT to_regclass('migrations') IS NOT NULL")
        .fetch_one(pool)
        .await
    {
        Ok(e) => e,
        Err(_) => return false,
    };
    if !exists {
        return true;
    }
    let dir = PathBuf::from(
        std::env::var("TINYHELM_MIGRATIONS_DIR")
            .unwrap_or_else(|_| "../database/migrations".into()),
    );
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let files: Vec<String> = entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".php").map(str::to_string))
        .collect();
    let ran: Vec<String> = match sqlx::query_scalar("SELECT migration FROM migrations")
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(_) => return false,
    };
    files.iter().any(|f| !ran.contains(f))
}

/// GET /api/health and /api/v1/health: outside the auth group; `throttle:api` allows 1000/min per IP.
pub async fn health(
    State(state): State<crate::AppState>,
    connect: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let ip = connect
        .map(|ConnectInfo(a)| a.ip().to_string())
        .unwrap_or_default();
    let limit = throttle::check_max(&format!("health-ip:{ip}"), 1000);
    if !limit.allowed {
        return throttle::too_many(&limit);
    }
    let pending = match state.pool.as_ref() {
        Some(pool) => has_pending_migrations(pool).await,
        None => false,
    };
    let mut res = if pending {
        text(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database migrations are running.".into(),
        )
    } else {
        text(StatusCode::OK, "OK".into())
    };
    throttle::annotate(&mut res, &limit);
    res
}

/// GET /api/v1/disable — the route moved to POST.
pub async fn post_required(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    if let Err(r) = ctx.require(&state, "write").await {
        return r;
    }
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [("allow", "POST")],
        Json(json!({"message": "This endpoint has changed to a POST request."})),
    )
        .into_response()
}

/// POST /disable, /mcp/enable, /mcp/disable: only an admin or owner of the root team (token team `0`,
/// whose role the guard checked) may change the instance setting. `event` is the verb's audit stem.
async fn instance_switch(
    state: crate::AppState,
    ctx: Ctx,
    column: &str,
    value: bool,
    event: &str,
    denied: &str,
    done: &str,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if team_id != 0 {
        audit::record(
            pool,
            &ctx,
            &format!("{event}_denied"),
            json!({"team_id": team_id}),
            "warning",
        )
        .await;
        return (StatusCode::FORBIDDEN, Json(json!({ "message": denied }))).into_response();
    }
    // Eloquent `update()` writes (and touches updated_at) only when the value changes.
    let sql = format!(
        "UPDATE instance_settings SET {column} = $1, updated_at = {NOW} WHERE id = 0 AND {column} IS DISTINCT FROM $1"
    );
    if let Err(e) = sqlx::query(&sql).bind(value).execute(pool).await {
        return query_err("instance_switch", e);
    }
    audit::record(
        pool,
        &ctx,
        &format!("{event}d"),
        json!({"team_id": team_id}),
        "info",
    )
    .await;
    (StatusCode::OK, Json(json!({ "message": done }))).into_response()
}

/// POST /api/v1/disable
pub async fn disable_api(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    instance_switch(
        state,
        ctx,
        "is_api_enabled",
        false,
        "api.instance.disable",
        "You are not allowed to disable the API.",
        "API disabled.",
    )
    .await
}

/// POST /api/v1/mcp/enable
pub async fn enable_mcp(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    instance_switch(
        state,
        ctx,
        "is_mcp_server_enabled",
        true,
        "api.mcp.enable",
        "You are not allowed to enable the MCP server.",
        "MCP server enabled.",
    )
    .await
}

/// POST /api/v1/mcp/disable
pub async fn disable_mcp(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    instance_switch(
        state,
        ctx,
        "is_mcp_server_enabled",
        false,
        "api.mcp.disable",
        "You are not allowed to disable the MCP server.",
        "MCP server disabled.",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_int_cast_matches_php() {
        assert_eq!(php_int_cast("6001"), 6001);
        assert_eq!(php_int_cast(" 443abc"), 443);
        assert_eq!(php_int_cast("abc"), 0);
    }

    #[test]
    fn version_falls_back_to_release_json() {
        if laravel_env("TINYHELM_VERSION").is_none() {
            assert!(
                !version().is_empty(),
                "../release.json should provide stable.tinyhelm"
            );
        }
    }
}
