//! Native `/api/v1` handlers: tags (Laravel `TagsController`).

#[allow(unused_imports)]
use super::prelude::*;

use axum::{body::Bytes, http::HeaderMap};
use serde_json::{Map, Value};

/// `TagsController::serializeTag`, built in SQL: uuid, name and Carbon-formatted timestamps.
const TAG_JSON: &str = "jsonb_build_object('uuid', uuid, 'name', name, \
    'created_at', to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'), \
    'updated_at', to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'))";

/// GET /api/v1/tags — ordered by name.
pub async fn list_tags(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {TAG_JSON} FROM tags WHERE team_id = $1 ORDER BY name"
    ))
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(rows),
        Err(e) => query_err("list_tags", e),
    }
}

/// `validateTagWriteRequest`: name required|string|min:2|max:255, and `strtolower(trim(strip_tags()))`
/// must still be 2 characters long.
fn validate_tag(input: &Map<String, Value>) -> Result<String, Response> {
    let mut v = Validator::new(input, &[]);
    v.field(
        "name",
        &[Rule::Required, Rule::Str, Rule::Min(2), Rule::Max(255)],
    );
    v.reject_extra(&["name"]);
    if v.fails() {
        return Err(v.response());
    }
    let name = strip_tags(&input_str(input, "name").unwrap_or_default())
        .trim()
        .to_lowercase();
    if name.chars().count() < 2 {
        return Err(validation_failed(json!({
            "name": ["The tag name must be at least 2 characters after sanitization."]
        })));
    }
    Ok(name)
}

fn conflict() -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({"message": "Tag with this name already exists."})),
    )
        .into_response()
}

/// POST /api/v1/tags — 409 when the team already has the name.
pub async fn create_tag(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = match json_input(&headers, &body) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let name = match validate_tag(&input) {
        Ok(n) => n,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM tags WHERE team_id = $1 AND name = $2)",
    )
    .bind(team_id)
    .bind(&name)
    .fetch_one(pool)
    .await
    {
        Ok(true) => return conflict(),
        Ok(false) => {}
        Err(e) => return query_err("create_tag", e),
    }

    let uuid = new_public_id();
    let tag = match sqlx::query_scalar::<_, Value>(&format!(
        "INSERT INTO tags (uuid, name, team_id, created_at, updated_at) VALUES ($1,$2,$3,{NOW},{NOW}) \
         RETURNING {TAG_JSON}"
    ))
    .bind(&uuid)
    .bind(&name)
    .bind(team_id)
    .fetch_one(pool)
    .await
    {
        Ok(t) => t,
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => return conflict(),
        Err(e) => return query_err("create_tag", e),
    };

    let new = as_map(json!({"name": name, "team_id": team_id, "uuid": uuid}));
    let changes = audit_changes(None, Some(&new), &fields_of(&new), &[]);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "tag",
        "created",
        Some(&uuid),
        Some(&name),
        vec![],
        changes,
    )
    .await;
    audit::record(
        pool,
        &ctx,
        "api.tag.created",
        json!({"team_id": team_id, "tag_uuid": uuid, "tag_name": name}),
        "info",
    )
    .await;

    (StatusCode::CREATED, Json(tag)).into_response()
}

/// DELETE /api/v1/tags/:uuid — taggables rows cascade through the FK.
pub async fn delete_tag(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let tag = match sqlx::query_scalar::<_, Value>(
        "DELETE FROM tags t WHERE team_id = $1 AND uuid = $2 RETURNING to_jsonb(t)",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(t)) => as_map(t),
        Ok(None) => return not_found("Tag not found."),
        Err(e) => return query_err("delete_tag", e),
    };
    let name = tag
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let changes = audit_changes(Some(&tag), None, &fields_of(&tag), &[]);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "tag",
        "deleted",
        Some(&uuid),
        Some(&name),
        vec![],
        changes,
    )
    .await;
    audit::record(
        pool,
        &ctx,
        "api.tag.deleted",
        json!({"team_id": team_id, "tag_uuid": uuid, "tag_name": name}),
        "info",
    )
    .await;

    (StatusCode::OK, Json(json!({"message": "Tag deleted."}))).into_response()
}
