//! Native reads of a service's parts: `ServiceApplicationsController::index/show` and
//! `ServiceDatabasesController::index/show`. Updates, logs and actions stay with Laravel.

#[allow(unused_imports)]
use super::prelude::*;

use axum::extract::Request;
use serde_json::Value;

/// `makeHidden` of each controller's `removeSensitiveData`.
const APPLICATION_HIDDEN: &[&str] = &[
    "id",
    "resourceable",
    "resourceable_id",
    "resourceable_type",
    "service",
];
const DATABASE_HIDDEN: &[&str] = &[
    "id",
    "service",
    "service_id",
    "resourceable",
    "resourceable_id",
    "resourceable_type",
];

/// Service id of a team's (not deleted) service.
async fn service_id(pool: &PgPool, team_id: i64, uuid: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT s.id FROM services s JOIN environments e ON e.id = s.environment_id \
         JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1 AND s.uuid = $2 AND s.deleted_at IS NULL LIMIT 1",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

fn application_json(row: Value) -> Result<Value, Fail> {
    let url = service_application_url(row.as_object().ok_or(Fail::Delegate)?)?;
    let Value::Object(mut m) = to_laravel(row, &SERVICE_APPLICATION_SPEC) else {
        return Err(Fail::Delegate);
    };
    for k in APPLICATION_HIDDEN {
        m.remove(*k);
    }
    m.insert("url".into(), url);
    Ok(Value::Object(m))
}

fn database_json(row: Value) -> Value {
    let mut v = to_laravel(row, &SERVICE_DATABASE_SPEC);
    if let Value::Object(m) = &mut v {
        for k in DATABASE_HIDDEN {
            m.remove(*k);
        }
    }
    v
}

/// Rows of `service_applications` or `service_databases` (soft-deleted excluded).
async fn part_rows(
    pool: &PgPool,
    table: &str,
    service_id: i64,
    uuid: Option<&str>,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT to_jsonb(t) FROM {table} t WHERE t.service_id = $1 AND t.deleted_at IS NULL \
         AND ($2::text IS NULL OR t.uuid = $2) ORDER BY t.id"
    ))
    .bind(service_id)
    .bind(uuid)
    .fetch_all(pool)
    .await
}

async fn respond(
    state: &crate::AppState,
    ctx: &Ctx,
    req: Request,
    service_uuid: &str,
    part: Option<&str>,
    applications: bool,
) -> Response {
    let (pool, team_id) = match ctx.require(state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let what = if applications {
        "service_applications"
    } else {
        "service_databases"
    };
    let sid = match service_id(pool, team_id, service_uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return not_found("Service not found."),
        Err(e) => return query_err(what, e),
    };
    let rows = match part_rows(pool, what, sid, part).await {
        Ok(rows) => rows,
        Err(e) => return query_err(what, e),
    };
    let serialized: Result<Vec<Value>, Fail> = rows
        .into_iter()
        .map(|r| {
            if applications {
                application_json(r)
            } else {
                Ok(database_json(r))
            }
        })
        .collect();
    let items = match serialized {
        Ok(items) => items,
        Err(fail) => return fail_response(state.clone(), req, fail, what).await,
    };
    match part {
        None => ok_json(items),
        Some(_) => match items.into_iter().next() {
            Some(item) => (StatusCode::OK, Json(item)).into_response(),
            None if applications => not_found("Service application not found."),
            None => not_found("Service database not found."),
        },
    }
}

/// GET /api/v1/services/:uuid/applications
pub async fn list_service_applications(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    respond(&state, &ctx, req, &uuid, None, true).await
}

/// GET /api/v1/services/:uuid/applications/:app_uuid
pub async fn get_service_application(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, app_uuid)): Path<(String, String)>,
    req: Request,
) -> Response {
    respond(&state, &ctx, req, &uuid, Some(&app_uuid), true).await
}

/// GET /api/v1/services/:uuid/databases
pub async fn list_service_databases(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    respond(&state, &ctx, req, &uuid, None, false).await
}

/// GET /api/v1/services/:uuid/databases/:database_uuid
pub async fn get_service_database(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, database_uuid)): Path<(String, String)>,
    req: Request,
) -> Response {
    respond(&state, &ctx, req, &uuid, Some(&database_uuid), false).await
}
