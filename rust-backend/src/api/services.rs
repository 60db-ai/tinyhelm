//! Native `/api/v1` handlers: services.

#[allow(unused_imports)]
use super::prelude::*;

// ---- services ----
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct ServiceRow {
    uuid: String,
    name: String,
    description: Option<String>,
}

pub(crate) const SERVICE_SELECT: &str = r#"SELECT s.uuid, s.name, s.description
    FROM services s
    JOIN environments e ON e.id = s.environment_id
    JOIN projects p ON p.id = e.project_id"#;

/// GET /api/v1/services
pub async fn list_services(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let sql = format!("{SERVICE_SELECT} WHERE p.team_id = $1 ORDER BY s.id");
    match sqlx::query_as::<_, ServiceRow>(&sql)
        .bind(team_id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_services", e),
    }
}

/// GET /api/v1/services/:uuid
pub async fn get_service(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let sql = format!("{SERVICE_SELECT} WHERE p.team_id = $1 AND s.uuid = $2 LIMIT 1");
    match sqlx::query_as::<_, ServiceRow>(&sql)
        .bind(team_id)
        .bind(&uuid)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => not_found("Service not found."),
        Err(e) => query_err("get_service", e),
    }
}
