//! Native `/api/v1` handlers: dashboard.

#[allow(unused_imports)]
use super::prelude::*;

// ---- dashboard (one call: projects w/ counts, servers w/ status, recent deployments) ----
pub(crate) const RESOURCE_TABLES: [&str; 11] = [
    "applications",
    "services",
    "standalone_postgresqls",
    "standalone_mysqls",
    "standalone_mariadbs",
    "standalone_mongodbs",
    "standalone_redis",
    "standalone_keydbs",
    "standalone_dragonflies",
    "standalone_clickhouses",
    "standalone_sqlites",
];

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct DashProject {
    uuid: String,
    name: String,
    description: Option<String>,
    environments: i64,
    resources: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct DashDeployment {
    deployment_uuid: String,
    status: String,
    application_name: Option<String>,
    application_uuid: String,
    project: String,
    environment: String,
    created_at: Option<chrono::NaiveDateTime>,
}

/// GET /api/v1/dashboard
pub async fn dashboard(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };

    let resources_expr = RESOURCE_TABLES
        .iter()
        .map(|t| format!("(SELECT count(*) FROM {t} WHERE environment_id IN (SELECT id FROM environments WHERE project_id = p.id))"))
        .collect::<Vec<_>>()
        .join(" + ");
    let projects_sql = format!(
        "SELECT p.uuid, p.name, p.description, \
         (SELECT count(*) FROM environments e WHERE e.project_id = p.id) AS environments, \
         ({resources_expr}) AS resources \
         FROM projects p WHERE p.team_id = $1 ORDER BY p.name"
    );
    let servers_sql = format!("{SERVER_SELECT} WHERE s.team_id = $1 ORDER BY s.name");
    let deployments_sql =
        "SELECT d.deployment_uuid, d.status, d.application_name, a.uuid AS application_uuid, \
         p.name AS project, e.name AS environment, d.created_at \
         FROM application_deployment_queues d \
         JOIN applications a ON (a.id::text = d.application_id OR a.uuid = d.application_id) \
         JOIN environments e ON e.id = a.environment_id \
         JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 ORDER BY d.id DESC LIMIT 10";

    let projects = sqlx::query_as::<_, DashProject>(&projects_sql)
        .bind(team_id)
        .fetch_all(pool)
        .await;
    let servers = sqlx::query_as::<_, ServerRow>(&servers_sql)
        .bind(team_id)
        .fetch_all(pool)
        .await;
    let deployments = sqlx::query_as::<_, DashDeployment>(deployments_sql)
        .bind(team_id)
        .fetch_all(pool)
        .await;

    match (projects, servers, deployments) {
        (Ok(p), Ok(s), Ok(d)) => ok_json(json!({ "projects": p, "servers": s, "deployments": d })),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => query_err("dashboard", e),
    }
}
