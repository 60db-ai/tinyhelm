//! Native `/api/v1` handlers: applications.

#[allow(unused_imports)]
use super::prelude::*;

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct ApplicationRow {
    uuid: String,
    name: String,
    description: Option<String>,
    fqdn: Option<String>,
    status: String,
    git_repository: Option<String>,
    git_branch: Option<String>,
    build_pack: Option<String>,
    docker_registry_image_name: Option<String>,
    docker_registry_image_tag: Option<String>,
    ports_mappings: Option<String>,
    ports_exposes: Option<String>,
    base_directory: Option<String>,
    health_check_path: Option<String>,
    created_at: Option<chrono::NaiveDateTime>,
    updated_at: Option<chrono::NaiveDateTime>,
}

/// GET /api/v1/applications — team-scoped via environment -> project.
///
/// SECURITY: this does NOT yet replicate Laravel's access-grant filtering (ScopedByAccessGrants),
/// so a project-scoped member's token would see ALL of the team's applications. Must port the
/// access-grant scope before enabling this route for anyone but full-team (admin/owner) tokens.
pub async fn list_applications(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT a.uuid, a.name, a.description, a.fqdn, a.status,
                  a.git_repository, a.git_branch, a.build_pack,
                  a.docker_registry_image_name, a.docker_registry_image_tag, a.ports_mappings,
                  a.ports_exposes, a.base_directory, a.health_check_path, a.created_at, a.updated_at
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
        Ok(r) => (
            StatusCode::OK,
            Json(serde_json::to_value(r).unwrap_or(json!([]))),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("list_applications query failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"message": "Query failed."})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/applications/:uuid — single application, team-scoped via environment -> project.
pub async fn get_application(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT a.uuid, a.name, a.description, a.fqdn, a.status,
                  a.git_repository, a.git_branch, a.build_pack,
                  a.docker_registry_image_name, a.docker_registry_image_tag, a.ports_mappings,
                  a.ports_exposes, a.base_directory, a.health_check_path, a.created_at, a.updated_at
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
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"message": "Application not found."})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("get_application query failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"message": "Query failed."})),
            )
                .into_response()
        }
    }
}
