//! Native `/api/v1` handlers: databases.

#[allow(unused_imports)]
use super::prelude::*;

// ---- databases (union across the 9 standalone engines) ----
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct DatabaseRow {
    uuid: String,
    name: String,
    description: Option<String>,
    status: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    db_type: String,
}

pub(crate) const DATABASE_UNION: &str = r#"
    SELECT uuid,name,description,status,environment_id,'postgresql' AS type FROM standalone_postgresqls
    UNION ALL SELECT uuid,name,description,status,environment_id,'mysql' FROM standalone_mysqls
    UNION ALL SELECT uuid,name,description,status,environment_id,'mariadb' FROM standalone_mariadbs
    UNION ALL SELECT uuid,name,description,status,environment_id,'mongodb' FROM standalone_mongodbs
    UNION ALL SELECT uuid,name,description,status,environment_id,'redis' FROM standalone_redis
    UNION ALL SELECT uuid,name,description,status,environment_id,'keydb' FROM standalone_keydbs
    UNION ALL SELECT uuid,name,description,status,environment_id,'dragonfly' FROM standalone_dragonflies
    UNION ALL SELECT uuid,name,description,status,environment_id,'clickhouse' FROM standalone_clickhouses
    UNION ALL SELECT uuid,name,description,status,environment_id,'sqlite' FROM standalone_sqlites"#;

/// GET /api/v1/databases
pub async fn list_databases(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let sql = format!(
        "SELECT d.uuid, d.name, d.description, d.status, d.type FROM ({DATABASE_UNION}) d \
         JOIN environments e ON e.id = d.environment_id JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 ORDER BY d.name"
    );
    match sqlx::query_as::<_, DatabaseRow>(&sql)
        .bind(team_id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_databases", e),
    }
}

/// GET /api/v1/databases/:uuid
pub async fn get_database(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let sql = format!(
        "SELECT d.uuid, d.name, d.description, d.status, d.type FROM ({DATABASE_UNION}) d \
         JOIN environments e ON e.id = d.environment_id JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND d.uuid = $2 LIMIT 1"
    );
    match sqlx::query_as::<_, DatabaseRow>(&sql)
        .bind(team_id)
        .bind(&uuid)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => not_found("Database not found."),
        Err(e) => query_err("get_database", e),
    }
}
