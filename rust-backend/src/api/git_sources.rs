//! Native `/api/v1` handlers: Git sources (`GithubController`, `GitlabController`, `GiteaController`):
//! the app lists and GitLab app deletion.
//!
//! Left in Laravel: create/update (the URL rules resolve DNS to block internal hosts), repository and branch
//! listing (GitHub API), and GitHub app deletion (runner checks and the private-key cascade with its own audit).

#[allow(unused_imports)]
use super::prelude::*;

use axum::extract::Request;
use serde_json::Value;

/// `team_id === $teamId` in `removeSensitiveData`: secrets only for the app's own team.
fn own_team(row: &Value, team_id: i64) -> bool {
    row.get("team_id").and_then(Value::as_i64) == Some(team_id)
}

/// `->where('team_id', $teamId)->orWhere('is_system_wide', true)` (unordered in Laravel; id order here).
async fn source_rows(pool: &PgPool, table: &str, team_id: i64) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(&format!(
        "SELECT to_jsonb(a) FROM {table} a WHERE a.team_id = $1 OR a.is_system_wide = true ORDER BY a.id"
    ))
    .bind(team_id)
    .fetch_all(pool)
    .await
}

const GITHUB_SPEC: ModelSpec = ModelSpec {
    hidden: &["client_secret", "webhook_secret"],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// GET /api/v1/github-apps
pub async fn list_github_apps(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match source_rows(pool, "github_apps", team_id).await {
        Ok(r) => r,
        Err(e) => return query_err("list_github_apps", e),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let visible: &[&str] = if ctx.can_read_sensitive() && own_team(&row, team_id) {
            &["client_secret", "webhook_secret"]
        } else {
            &[]
        };
        let Some(mut map) = credential_json(
            row,
            &GITHUB_SPEC,
            visible,
            &["client_secret", "webhook_secret"],
            state.app_key.as_ref(),
        ) else {
            return crate::proxy(State(state.clone()), req).await;
        };
        map.insert("type".into(), json!("github"));
        out.push(Value::Object(map));
    }
    ok_json(out)
}

const GITLAB_SPEC: ModelSpec = ModelSpec {
    hidden: &[
        "webhook_token",
        "app_secret",
        "client_secret",
        "access_token",
        "refresh_token",
    ],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};
const GITLAB_TOGGLED: &[&str] = &[
    "client_secret",
    "webhook_token",
    "access_token",
    "refresh_token",
];
const GITLAB_ENCRYPTED: &[&str] = &[
    "access_token",
    "refresh_token",
    "client_secret",
    "app_secret",
];

/// GET /api/v1/gitlab-apps
pub async fn list_gitlab_apps(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match source_rows(pool, "gitlab_apps", team_id).await {
        Ok(r) => r,
        Err(e) => return query_err("list_gitlab_apps", e),
    };
    let mut out = Vec::with_capacity(rows.len());
    for mut row in rows {
        let visible: &[&str] = if ctx.can_read_sensitive() && own_team(&row, team_id) {
            GITLAB_TOGGLED
        } else {
            &[]
        };
        // `webhookToken()` accessor: encrypted, but legacy plaintext values are returned as they are.
        if let (Some(Value::String(raw)), Some(key)) =
            (row.get("webhook_token"), state.app_key.as_ref())
        {
            if !raw.is_empty() {
                let plain = crate::crypto::decrypt(raw, key).unwrap_or_else(|| raw.clone());
                row["webhook_token"] = Value::String(plain);
            }
        }
        match credential_json(
            row,
            &GITLAB_SPEC,
            visible,
            GITLAB_ENCRYPTED,
            state.app_key.as_ref(),
        ) {
            Some(map) => out.push(Value::Object(map)),
            None => return crate::proxy(State(state.clone()), req).await,
        }
    }
    ok_json(out)
}

const GITEA_SECRETS: &[&str] = &[
    "client_secret",
    "access_token",
    "refresh_token",
    "webhook_secret",
];
const GITEA_SPEC: ModelSpec = ModelSpec {
    hidden: GITEA_SECRETS,
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// PHP `empty()` of a decrypted token.
fn php_empty(v: Option<&str>) -> bool {
    matches!(v, None | Some("") | Some("0"))
}

/// GET /api/v1/gitea-apps — with `is_connected` (both OAuth tokens present).
pub async fn list_gitea_apps(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match source_rows(pool, "gitea_apps", team_id).await {
        Ok(r) => r,
        Err(e) => return query_err("list_gitea_apps", e),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // `isConnected()` reads the decrypted tokens; a value that does not decrypt is a Laravel 500.
        let mut tokens = Vec::with_capacity(2);
        for field in ["access_token", "refresh_token"] {
            let plain = match (
                row.get(field).and_then(Value::as_str),
                state.app_key.as_ref(),
            ) {
                (None, _) => None,
                (Some(c), Some(k)) => match crate::crypto::decrypt(c, k) {
                    Some(p) => Some(p),
                    None => return crate::proxy(State(state.clone()), req).await,
                },
                (Some(_), None) => return crate::proxy(State(state.clone()), req).await,
            };
            tokens.push(plain);
        }
        let connected = !php_empty(tokens[0].as_deref()) && !php_empty(tokens[1].as_deref());
        let visible: &[&str] = if ctx.can_read_sensitive() && own_team(&row, team_id) {
            GITEA_SECRETS
        } else {
            &[]
        };
        let Some(mut map) = credential_json(
            row,
            &GITEA_SPEC,
            visible,
            GITEA_SECRETS,
            state.app_key.as_ref(),
        ) else {
            return crate::proxy(State(state.clone()), req).await;
        };
        map.insert("is_connected".into(), json!(connected));
        out.push(Value::Object(map));
    }
    ok_json(out)
}

/// Columns of `gitlab_apps` in table order (the Auditable `deleted` change set).
const GITLAB_COLUMNS: &[&str] = &[
    "id",
    "uuid",
    "name",
    "organization",
    "api_url",
    "html_url",
    "custom_port",
    "custom_user",
    "is_system_wide",
    "is_public",
    "app_id",
    "app_secret",
    "oauth_id",
    "group_name",
    "public_key",
    "webhook_token",
    "deploy_key_id",
    "private_key_id",
    "team_id",
    "created_at",
    "updated_at",
    "client_id",
    "client_secret",
    "access_token",
    "refresh_token",
    "expires_at",
    "redirect_uri",
];

/// DELETE /api/v1/gitlab-apps/:id — 409 while applications use it.
pub async fn delete_gitlab_app(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    // A non-numeric id is a query error in Laravel (bigint comparison): let Laravel answer it.
    let Ok(app_id) = id.parse::<i64>() else {
        return crate::proxy(State(state.clone()), req).await;
    };
    let row = match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(a) FROM gitlab_apps a WHERE a.id = $1 AND a.team_id = $2",
    )
    .bind(app_id)
    .bind(team_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => as_map(r),
        Ok(None) => return not_found("GitLab app not found"),
        Err(e) => return query_err("delete_gitlab_app", e),
    };
    // `GitlabAppPolicy::delete`: system-wide apps need an admin of the root team.
    if row.get("is_system_wide").and_then(Value::as_bool) == Some(true) {
        let root_admin = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM team_user WHERE user_id = $1 AND team_id = 0 AND role IN ('admin','owner'))",
        )
        .bind(ctx.user_id)
        .fetch_one(pool)
        .await;
        match root_admin {
            Ok(true) => {}
            Ok(false) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"message": "This action is unauthorized."})),
                )
                    .into_response()
            }
            Err(e) => return query_err("delete_gitlab_app", e),
        }
    }
    let used = sqlx::query_scalar::<_, i64>(
        r"SELECT count(*) FROM applications WHERE source_type = 'App\Models\GitlabApp' AND source_id = $1",
    )
    .bind(app_id)
    .fetch_one(pool)
    .await;
    match used {
        Ok(0) => {}
        Ok(count) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({"message": format!(
                    "This GitLab app is being used by {count} application(s). Please delete all applications first."
                )})),
            )
                .into_response()
        }
        Err(e) => return query_err("delete_gitlab_app", e),
    }
    if let Err(e) = sqlx::query("DELETE FROM gitlab_apps WHERE id = $1")
        .bind(app_id)
        .execute(pool)
        .await
    {
        return query_err("delete_gitlab_app", e);
    }
    let uuid = row.get("uuid").and_then(Value::as_str).map(str::to_string);
    let name = row.get("name").and_then(Value::as_str).map(str::to_string);
    let fields: Vec<String> = GITLAB_COLUMNS.iter().map(|c| c.to_string()).collect();
    let changes = audit_changes(Some(&row), None, &fields, GITLAB_SPEC.hidden);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "gitlab_app",
        "deleted",
        uuid.as_deref(),
        name.as_deref(),
        vec![],
        changes,
    )
    .await;
    audit::record(
        pool,
        &ctx,
        "api.gitlab_app.deleted",
        json!({"team_id": team_id, "gitlab_app_uuid": uuid, "gitlab_app_name": name}),
        "info",
    )
    .await;
    ok_json(json!({"message": "GitLab app deleted successfully"}))
}
