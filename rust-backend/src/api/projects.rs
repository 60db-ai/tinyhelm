//! Native `/api/v1` handlers: projects.

#[allow(unused_imports)]
use super::prelude::*;

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct ProjectRow {
    uuid: String,
    name: String,
    description: Option<String>,
}

/// GET /api/v1/projects — team-scoped.
pub async fn list_projects(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
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
        Ok(r) => (
            StatusCode::OK,
            Json(serde_json::to_value(r).unwrap_or(json!([]))),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("list_projects query failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"message": "Query failed."})),
            )
                .into_response()
        }
    }
}

// ---- environments (by project) ----
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct EnvironmentRow {
    uuid: String,
    name: String,
}

/// GET /api/v1/projects/:uuid/environments
pub async fn list_environments(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(project_uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_as::<_, EnvironmentRow>(
        r#"SELECT e.uuid, e.name FROM environments e JOIN projects p ON p.id = e.project_id
           WHERE p.uuid = $1 AND p.team_id = $2 ORDER BY e.id"#,
    )
    .bind(&project_uuid)
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_environments", e),
    }
}

// ---- project writes (pure DB; no deploy engine needed) ----
#[derive(Deserialize)]
pub struct CreateProject {
    name: String,
    description: Option<String>,
}

pub(crate) async fn fetch_project(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<Option<ProjectRow>, sqlx::Error> {
    sqlx::query_as::<_, ProjectRow>(
        "SELECT uuid, name, description FROM projects WHERE team_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

/// POST /api/v1/projects — create a project and its default `production` environment (matches Laravel).
pub async fn create_project(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Json(body): Json<CreateProject>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let name = body.name.trim();
    if name.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "Name is required."})),
        )
            .into_response();
    }
    let project_uuid = cuid2::create_id();
    let env_uuid = cuid2::create_id();

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return query_err("create_project.begin", e),
    };
    let project_id: i64 = match sqlx::query_scalar(
        "INSERT INTO projects (uuid, name, description, team_id, created_at, updated_at) VALUES ($1,$2,$3,$4,now(),now()) RETURNING id",
    )
    .bind(&project_uuid)
    .bind(name)
    .bind(&body.description)
    .bind(team_id)
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            let _ = tx.rollback().await;
            return query_err("create_project", e);
        }
    };
    if let Err(e) = sqlx::query(
        "INSERT INTO environments (uuid, name, project_id, created_at, updated_at) VALUES ($1,'production',$2,now(),now())",
    )
    .bind(&env_uuid)
    .bind(project_id)
    .execute(&mut *tx)
    .await
    {
        let _ = tx.rollback().await;
        return query_err("create_project.environment", e);
    }
    if let Err(e) = tx.commit().await {
        return query_err("create_project.commit", e);
    }

    (
        StatusCode::CREATED,
        Json(json!({ "uuid": project_uuid, "name": name, "description": body.description })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct UpdateProject {
    name: Option<String>,
    description: Option<String>,
}

/// PATCH /api/v1/projects/:uuid — update name/description.
pub async fn update_project(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    Json(body): Json<UpdateProject>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let res = sqlx::query(
        "UPDATE projects SET name = COALESCE($1, name), description = COALESCE($2, description), updated_at = now() WHERE uuid = $3 AND team_id = $4",
    )
    .bind(body.name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&body.description)
    .bind(&uuid)
    .bind(team_id)
    .execute(pool)
    .await;
    match res {
        Ok(r) if r.rows_affected() == 0 => not_found("Project not found."),
        Ok(_) => match fetch_project(pool, team_id, &uuid).await {
            Ok(Some(p)) => (StatusCode::OK, Json(p)).into_response(),
            Ok(None) => not_found("Project not found."),
            Err(e) => query_err("update_project.fetch", e),
        },
        Err(e) => query_err("update_project", e),
    }
}

// Counts resources across a project's environments (apps + services + every standalone db engine).
pub(crate) const PROJECT_RESOURCE_COUNT: &str = "SELECT \
 (SELECT count(*) FROM applications WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM services WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_postgresqls WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_mysqls WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_mariadbs WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_mongodbs WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_redis WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_keydbs WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_dragonflies WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_clickhouses WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) \
+(SELECT count(*) FROM standalone_sqlites WHERE environment_id IN (SELECT id FROM environments WHERE project_id=$1)) AS total";

/// DELETE /api/v1/projects/:uuid — refuses if the project still has resources (data-loss guard).
pub async fn delete_project(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let pid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM projects WHERE uuid = $1 AND team_id = $2")
            .bind(&uuid)
            .bind(team_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let Some(pid) = pid else {
        return not_found("Project not found.");
    };

    let total: i64 = match sqlx::query_scalar(PROJECT_RESOURCE_COUNT)
        .bind(pid)
        .fetch_one(pool)
        .await
    {
        Ok(n) => n,
        Err(e) => return query_err("delete_project.count", e),
    };
    if total > 0 {
        return (
            StatusCode::CONFLICT,
            Json(json!({"message": "Project has resources; delete them first."})),
        )
            .into_response();
    }

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return query_err("delete_project.begin", e),
    };
    if let Err(e) = sqlx::query("DELETE FROM environments WHERE project_id = $1")
        .bind(pid)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        return query_err("delete_project.environments", e);
    }
    if let Err(e) = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(pid)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        return query_err("delete_project", e);
    }
    if let Err(e) = tx.commit().await {
        return query_err("delete_project.commit", e);
    }
    (StatusCode::OK, Json(json!({"message": "Project deleted."}))).into_response()
}
