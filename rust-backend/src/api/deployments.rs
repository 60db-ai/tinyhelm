//! Native `/api/v1` handlers: deployment reads (Laravel `DeployController`). Starting and cancelling
//! deployments stays in Laravel until the deploy engine is ported.

#[allow(unused_imports)]
use super::prelude::*;

use axum::extract::RawQuery;
use serde_json::{Map, Value};

/// `ApplicationDeploymentQueue::$hidden` (besides `logs`) + casts.
const DEPLOYMENT_SPEC: ModelSpec = ModelSpec {
    hidden: &["configuration_snapshot", "configuration_diff"],
    datetimes: &["finished_at", "deploy_started_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// IDs of the team's applications, deleted ones included, as strings (`application_id` is varchar).
const TEAM_APP_IDS: &str =
    "SELECT a.id::text FROM applications a JOIN environments e ON e.id = a.environment_id \
     JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1";

/// Model JSON of one deployment row: `logs` stays hidden unless the token can read resource secrets.
fn deployment_json(row: Value, sensitive: bool) -> Map<String, Value> {
    let mut m = match to_laravel(row, &DEPLOYMENT_SPEC) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if !sensitive {
        m.remove("logs");
    }
    m
}

fn application_id_of(m: &Map<String, Value>) -> Option<i64> {
    m.get("application_id")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<i64>().ok())
}

/// `removeSensitiveData()` evaluates `$deployment->application`, so the lazy-loaded relation is
/// serialized too (null for a missing or deleted application).
async fn with_application(
    state: &crate::AppState,
    pool: &PgPool,
    mut m: Map<String, Value>,
    sensitive: bool,
) -> Result<Value, sqlx::Error> {
    let app = match application_id_of(&m) {
        Some(id) => embedded_application(pool, id, state.app_key.as_ref(), !sensitive).await?,
        None => None,
    };
    m.insert("application".into(), app.unwrap_or(Value::Null));
    Ok(Value::Object(m))
}

/// GET /api/v1/deployments: the team's queued and running deployments.
pub async fn list_deployments(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows: Vec<Value> = match sqlx::query_scalar(&format!(
        "SELECT to_jsonb(d) FROM application_deployment_queues d \
         WHERE d.application_id IN ({TEAM_APP_IDS}) AND d.status IN ('in_progress', 'queued') ORDER BY d.id"
    ))
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_deployments", e),
    };
    // ponytail: Laravel sorts with sortBy('id') and keeps the keys, so a heap order that is not by id
    // would make it serialize an object; Postgres returns these small sets in id order, which gives an array.
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let sensitive = ctx.can_read_sensitive();
        match with_application(&state, pool, deployment_json(row, sensitive), sensitive).await {
            Ok(v) => out.push(v),
            Err(e) => return query_err("list_deployments", e),
        }
    }
    ok_json(out)
}

/// GET /api/v1/deployments/{uuid}
pub async fn get_deployment(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let row: Option<Value> = match sqlx::query_scalar(
        "SELECT to_jsonb(d) FROM application_deployment_queues d WHERE d.deployment_uuid = $1 \
         ORDER BY d.id LIMIT 1",
    )
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("get_deployment", e),
    };
    let Some(row) = row else {
        return not_found("Deployment not found.");
    };
    let m = deployment_json(row, ctx.can_read_sensitive());
    // `$deployment->application` (not trashed) must belong to the token's team.
    let owned: bool = match application_id_of(&m) {
        Some(id) => match sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM applications a JOIN environments e ON e.id = a.environment_id \
             JOIN projects p ON p.id = e.project_id WHERE a.id = $1 AND a.deleted_at IS NULL AND p.team_id = $2)",
        )
        .bind(id)
        .bind(team_id)
        .fetch_one(pool)
        .await
        {
            Ok(b) => b,
            Err(e) => return query_err("get_deployment", e),
        },
        // ponytail: a non-numeric application_id makes Laravel's belongsTo query fail (500); Rust says 404.
        None => false,
    };
    if !owned {
        return not_found("Deployment not found.");
    }
    let mut out = match with_application(&state, pool, m, ctx.can_read_sensitive()).await {
        Ok(v) => v,
        Err(e) => return query_err("get_deployment", e),
    };
    // `$application->team()` is `data_get($this, 'environment.project.team')`: it lazy-loads the chain,
    // which the application JSON then carries.
    if let Some(app) = out.get_mut("application").and_then(Value::as_object_mut) {
        let app_id = app.get("id").and_then(Value::as_i64).unwrap_or_default();
        match environment_chain(pool, app_id).await {
            Ok(env) => {
                app.insert("environment".into(), env);
            }
            Err(e) => return query_err("get_deployment", e),
        }
    }
    ok_json(out)
}

/// The application's `environment` with `project` and `project.team` (no hidden attributes on these models).
async fn environment_chain(pool: &PgPool, app_id: i64) -> Result<Value, sqlx::Error> {
    let row: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) || jsonb_build_object('project', to_jsonb(p) || jsonb_build_object('team', to_jsonb(t))) \
         FROM applications a JOIN environments e ON e.id = a.environment_id \
         JOIN projects p ON p.id = e.project_id JOIN teams t ON t.id = p.team_id WHERE a.id = $1",
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map_or(Value::Null, |env| {
        let mut env = to_laravel(env, &ModelSpec::default());
        if let Some(project) = env.get_mut("project") {
            let mut p = to_laravel(project.take(), &ModelSpec::default());
            if let Some(team) = p.get_mut("team") {
                *team = to_laravel(team.take(), &ModelSpec::default());
            }
            *project = p;
        }
        env
    }))
}

/// GET /api/v1/deployments/applications/{uuid}?skip=&take=
pub async fn list_application_deployments(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    RawQuery(raw): RawQuery,
) -> Response {
    // The route's `api.ability:read` runs before the controller's `$request->validate()`.
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = query_input(raw.as_deref());
    let mut errors = Map::new();
    for (field, min) in [("skip", 0), ("take", 1)] {
        // `nullable`: a missing or empty value passes.
        let value = input.get(field).filter(|v| !v.is_null());
        let messages = integer_rule_errors(field, value, min, i64::MAX);
        if !messages.is_empty() {
            errors.insert(field.into(), json!(messages));
        }
    }
    if !errors.is_empty() {
        return validation_exception(errors);
    }

    let app_id: Option<i64> = match sqlx::query_scalar(
        "SELECT a.id FROM applications a JOIN environments e ON e.id = a.environment_id \
         JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND a.uuid = $2 AND a.deleted_at IS NULL LIMIT 1",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => return query_err("list_application_deployments", e),
    };
    let Some(app_id) = app_id else {
        return not_found("Application not found");
    };
    let number = |key: &str, default: i64| match input.get(key) {
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(default),
        _ => default,
    };
    let (skip, take) = (number("skip", 0), number("take", 10));
    let id = app_id.to_string();
    let count: i64 = match sqlx::query_scalar(
        "SELECT count(*) FROM application_deployment_queues WHERE application_id = $1",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    {
        Ok(n) => n,
        Err(e) => return query_err("list_application_deployments", e),
    };
    let rows: Vec<Value> = match sqlx::query_scalar(
        "SELECT to_jsonb(d) FROM application_deployment_queues d WHERE d.application_id = $1 \
         ORDER BY d.created_at DESC, d.id DESC OFFSET $2 LIMIT $3",
    )
    .bind(&id)
    .bind(skip)
    .bind(take)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_application_deployments", e),
    };
    let sensitive = ctx.can_read_sensitive();
    let deployments: Vec<Value> = rows
        .into_iter()
        .map(|row| Value::Object(deployment_json(row, sensitive)))
        .collect();
    ok_json(json!({"count": count, "deployments": deployments}))
}

/// Laravel's `ValidationException` JSON: the first message (plus "(and N more errors)") and all errors.
fn validation_exception(errors: Map<String, Value>) -> Response {
    let all: Vec<&str> = errors
        .values()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let first = all
        .first()
        .copied()
        .unwrap_or("The given data was invalid.");
    let message = match all.len() {
        0 | 1 => first.to_string(),
        2 => format!("{first} (and 1 more error)"),
        n => format!("{first} (and {} more errors)", n - 1),
    };
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": message, "errors": errors})),
    )
        .into_response()
}
