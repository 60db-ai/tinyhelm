//! Native `/api/v1` handlers: tags.

#[allow(unused_imports)]
use super::prelude::*;

// ---- tags (pure DB) ----
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct TagRow {
    uuid: String,
    name: String,
}

/// GET /api/v1/tags
pub async fn list_tags(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_as::<_, TagRow>(
        "SELECT uuid, name FROM tags WHERE team_id = $1 ORDER BY name",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => ok_json(r),
        Err(e) => query_err("list_tags", e),
    }
}

#[derive(Deserialize)]
pub struct CreateTag {
    name: String,
}

/// POST /api/v1/tags
pub async fn create_tag(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Json(body): Json<CreateTag>,
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
    let uuid = cuid2::create_id();
    match sqlx::query("INSERT INTO tags (uuid, name, team_id, created_at, updated_at) VALUES ($1,$2,$3,now(),now())")
        .bind(&uuid)
        .bind(name)
        .bind(team_id)
        .execute(pool)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "uuid": uuid, "name": name }))).into_response(),
        Err(e) => query_err("create_tag", e),
    }
}

/// DELETE /api/v1/tags/:uuid
pub async fn delete_tag(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query("DELETE FROM tags WHERE uuid = $1 AND team_id = $2")
        .bind(&uuid)
        .bind(team_id)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Tag not found."),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Tag deleted."}))).into_response(),
        Err(e) => query_err("delete_tag", e),
    }
}
