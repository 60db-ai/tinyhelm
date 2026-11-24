//! Native `/api/v1` handler: every resource of the team (Laravel `ResourcesController::resources`).

#[allow(unused_imports)]
use super::prelude::*;

/// Resource tables behind `Project` relations (applications, services, `DATABASE_TYPES`).
const RESOURCE_TABLES: &[&str] = &[
    "applications",
    "services",
    "standalone_postgresqls",
    "standalone_redis",
    "standalone_mongodbs",
    "standalone_mysqls",
    "standalone_mariadbs",
    "standalone_keydbs",
    "standalone_dragonflies",
    "standalone_clickhouses",
    "standalone_sqlites",
];

/// GET /api/v1/resources
pub async fn list_resources(
    State(state): State<crate::AppState>,
    req: axum::extract::Request,
) -> Response {
    let Some(ctx) = req.extensions().get::<Ctx>().cloned() else {
        return crate::proxy(State(state), req).await;
    };
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let count_sql = RESOURCE_TABLES
        .iter()
        .map(|t| {
            format!(
                "(SELECT count(*) FROM {t} r JOIN environments e ON e.id = r.environment_id \
                 JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1 AND r.deleted_at IS NULL)"
            )
        })
        .collect::<Vec<_>>()
        .join(" + ");
    match sqlx::query_scalar::<_, i64>(&format!("SELECT {count_sql}"))
        .bind(team_id)
        .fetch_one(pool)
        .await
    {
        Ok(0) => ok_json(json!([])),
        // ponytail: each resource goes through serializeApiResource (exposeSensitiveFields, locked-value
        // redaction, accessor-loaded relations) plus status/type; Laravel answers until that serializer is
        // ported for applications, services and every database type.
        Ok(_) => crate::proxy(State(state), req).await,
        Err(e) => query_err("list_resources", e),
    }
}
