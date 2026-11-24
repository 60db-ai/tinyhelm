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

/// `Tag` columns in table order (Eloquent attribute order for `changed_fields`).
const TAG_COLUMNS: &[&str] = &["id", "uuid", "name", "team_id", "created_at", "updated_at"];

/// PATCH /api/v1/tags/:uuid — 409 when another tag of the team has the name.
pub async fn update_tag(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
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
    let old = match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM tags t WHERE team_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(t)) => as_map(t),
        Ok(None) => return not_found("Tag not found."),
        Err(e) => return query_err("update_tag", e),
    };
    let tag_id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
    let old_name = old.get("name").and_then(Value::as_str).unwrap_or_default();
    if name != old_name {
        match sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM tags WHERE team_id = $1 AND name = $2 AND id <> $3)",
        )
        .bind(team_id)
        .bind(&name)
        .bind(tag_id)
        .fetch_one(pool)
        .await
        {
            Ok(true) => return conflict(),
            Ok(false) => {}
            Err(e) => return query_err("update_tag", e),
        }
    }

    // `HasSafeStringAttribute` strips tags again; the validated name is already clean.
    let mut new = old.clone();
    new.insert("name".into(), Value::String(strip_tags(&name)));
    let changed = changed_fields(&old, &new, &[], TAG_COLUMNS);
    if !changed.is_empty() {
        match sqlx::query(&format!(
            "UPDATE tags SET name = $1, updated_at = {NOW} WHERE id = $2"
        ))
        .bind(new.get("name").and_then(Value::as_str))
        .bind(tag_id)
        .execute(pool)
        .await
        {
            Ok(_) => {}
            Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
                return conflict()
            }
            Err(e) => return query_err("update_tag", e),
        }
        let changes = audit_changes(Some(&old), Some(&new), &changed, &[]);
        record_mutation(
            &state,
            pool,
            &ctx,
            team_id,
            "tag",
            "updated",
            Some(&uuid),
            new.get("name").and_then(Value::as_str),
            changed,
            changes,
        )
        .await;
    }
    audit::record(
        pool,
        &ctx,
        "api.tag.updated",
        json!({
            "team_id": team_id,
            "tag_uuid": uuid,
            "tag_name": new.get("name"),
            "changed_fields": ["name"],
        }),
        "info",
    )
    .await;

    match sqlx::query_scalar::<_, Value>(&format!("SELECT {TAG_JSON} FROM tags WHERE id = $1"))
        .bind(tag_id)
        .fetch_one(pool)
        .await
    {
        Ok(tag) => ok_json(tag),
        Err(e) => query_err("update_tag", e),
    }
}

// ---- resource tags (HandlesTagsApi on the Applications, Databases and Services controllers) ----

/// A taggable resource: its id and the morph class stored in `taggables.taggable_type`.
struct Taggable {
    id: i64,
    class: &'static str,
}

/// `STANDALONE_DATABASE_MODELS` order with each model's morph class.
const DATABASE_CLASSES: &[(&str, &str)] = &[
    ("standalone_postgresqls", r"App\Models\StandalonePostgresql"),
    ("standalone_redis", r"App\Models\StandaloneRedis"),
    ("standalone_mongodbs", r"App\Models\StandaloneMongodb"),
    ("standalone_mysqls", r"App\Models\StandaloneMysql"),
    ("standalone_mariadbs", r"App\Models\StandaloneMariadb"),
    ("standalone_keydbs", r"App\Models\StandaloneKeydb"),
    ("standalone_dragonflies", r"App\Models\StandaloneDragonfly"),
    ("standalone_clickhouses", r"App\Models\StandaloneClickhouse"),
    ("standalone_sqlites", r"App\Models\StandaloneSqlite"),
];

async fn team_resource_id(
    pool: &PgPool,
    table: &str,
    team_id: i64,
    uuid: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(&format!(
        "SELECT r.id FROM {table} r JOIN environments e ON e.id = r.environment_id \
         JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND r.uuid = $2 AND r.deleted_at IS NULL ORDER BY r.id LIMIT 1"
    ))
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

/// `findTaggableResource` of each controller; the Laravel 404 when it is missing.
async fn find_taggable(
    pool: &PgPool,
    kind: &str,
    team_id: i64,
    uuid: &str,
) -> Result<Taggable, Response> {
    let (found, missing) = match kind {
        "applications" => (
            team_resource_id(pool, "applications", team_id, uuid)
                .await
                .map(|id| {
                    id.map(|id| Taggable {
                        id,
                        class: r"App\Models\Application",
                    })
                }),
            "Application not found.",
        ),
        "services" => (
            team_resource_id(pool, "services", team_id, uuid)
                .await
                .map(|id| {
                    id.map(|id| Taggable {
                        id,
                        class: r"App\Models\Service",
                    })
                }),
            "Service not found.",
        ),
        _ => {
            // `queryDatabaseByUuidWithinTeam`: first engine with the uuid in the team; the instance
            // database (postgres id 0) is never returned.
            let mut found = Ok(None);
            for (table, class) in DATABASE_CLASSES {
                match team_resource_id(pool, table, team_id, uuid).await {
                    Ok(Some(0)) if *table == "standalone_postgresqls" => break,
                    Ok(Some(id)) => {
                        found = Ok(Some(Taggable { id, class }));
                        break;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        found = Err(e);
                        break;
                    }
                }
            }
            (found, "Database not found.")
        }
    };
    match found {
        Ok(Some(t)) => Ok(t),
        Ok(None) => Err(not_found(missing)),
        Err(e) => Err(query_err("find_taggable", e)),
    }
}

/// `$resource->tags->map(serializeTag(...))` — the morphToMany query, unordered as in Laravel.
async fn resource_tags_json(pool: &PgPool, resource: &Taggable) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object('uuid', tags.uuid, 'name', tags.name, \
         'created_at', to_char(tags.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'), \
         'updated_at', to_char(tags.updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')) \
         FROM tags INNER JOIN taggables ON tags.id = taggables.tag_id \
         WHERE taggables.taggable_id = $1 AND taggables.taggable_type = $2",
    )
    .bind(resource.id)
    .bind(resource.class)
    .fetch_all(pool)
    .await
}

/// GET /api/v1/{applications,services,databases}/:uuid/tags
async fn list_resource_tags(
    state: crate::AppState,
    ctx: Ctx,
    kind: &str,
    uuid: String,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let resource = match find_taggable(pool, kind, team_id, &uuid).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    match resource_tags_json(pool, &resource).await {
        Ok(tags) => ok_json(tags),
        Err(e) => query_err("list_resource_tags", e),
    }
}

/// `normalizeTagNames`: `strtolower(trim(strip_tags()))`, unique, in order.
fn normalize_tag_names(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let clean = strip_tags(name).trim().to_lowercase();
        if !out.contains(&clean) {
            out.push(clean);
        }
    }
    out
}

/// `createTag` validation (Laravel's default messages: `Validator::make`, not the custom API validator).
fn validate_attach(input: &Map<String, Value>) -> Result<Vec<String>, Response> {
    if input.contains_key("tag_name") && input.contains_key("tag_names") {
        return Err(validation_failed(json!({
            "tag_name": ["Provide either tag_name or tag_names, not both."]
        })));
    }
    let filled = |k: &str| match input.get(k) {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    };
    let neither = !filled("tag_name") && !filled("tag_names");
    let mut errors: Map<String, Value> = Map::new();
    let mut add = |field: &str, message: String| {
        if let Some(list) = errors
            .entry(field.to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut()
        {
            list.push(Value::String(message));
        }
    };
    if neither {
        add(
            "tag_name",
            "The tag name field is required when tag names is not present.".into(),
        );
    }
    if input.contains_key("tag_name") && !matches!(input.get("tag_name"), Some(Value::String(_))) {
        add("tag_name", "The tag name field must be a string.".into());
    }
    if neither {
        add(
            "tag_names",
            "The tag names field is required when tag name is not present.".into(),
        );
    }
    match input.get("tag_names") {
        None => {}
        Some(Value::Array(items)) => {
            if items.is_empty() {
                add(
                    "tag_names",
                    "The tag names field must have at least 1 items.".into(),
                );
            }
            for (i, item) in items.iter().enumerate() {
                if !item.is_string() {
                    add(
                        &format!("tag_names.{i}"),
                        format!("The tag_names.{i} field must be a string."),
                    );
                }
            }
        }
        Some(_) => add("tag_names", "The tag names field must be an array.".into()),
    }
    for key in input.keys() {
        if key != "tag_name" && key != "tag_names" {
            add(key, "This field is not allowed.".into());
        }
    }
    if !errors.is_empty() {
        return Err(validation_failed(Value::Object(errors)));
    }

    let raw: Vec<String> = match input.get("tag_names") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .collect(),
        _ => vec![input_str(input, "tag_name").unwrap_or_default()],
    };
    let names = normalize_tag_names(&raw);
    if names.iter().any(|n| n.chars().count() < 2) {
        return Err(validation_failed(json!({
            "tag_name": ["Each tag name must be at least 2 characters after sanitization."]
        })));
    }
    Ok(names)
}

/// POST /api/v1/{applications,services,databases}/:uuid/tags — attaches (creating tags as needed);
/// returns the resource's tags with 201.
async fn attach_resource_tags(
    state: crate::AppState,
    ctx: Ctx,
    kind: &str,
    uuid: String,
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
    let resource = match find_taggable(pool, kind, team_id, &uuid).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let names = match validate_attach(&input) {
        Ok(n) => n,
        Err(r) => return r,
    };
    for name in names {
        // `Tag::createOrFirst`: a new tag is a model `created` event (audited); an existing one is reused.
        let new_uuid = new_public_id();
        let inserted = sqlx::query_scalar::<_, i64>(&format!(
            "INSERT INTO tags (uuid, name, team_id, created_at, updated_at) VALUES ($1,$2,$3,{NOW},{NOW}) \
             ON CONFLICT (team_id, name) DO NOTHING RETURNING id"
        ))
        .bind(&new_uuid)
        .bind(strip_tags(&name))
        .bind(team_id)
        .fetch_optional(pool)
        .await;
        let tag_id = match inserted {
            Ok(Some(id)) => {
                let new = as_map(json!({"team_id": team_id, "name": name, "uuid": new_uuid}));
                let changes = audit_changes(None, Some(&new), &fields_of(&new), &[]);
                record_mutation(
                    &state,
                    pool,
                    &ctx,
                    team_id,
                    "tag",
                    "created",
                    Some(&new_uuid),
                    Some(&name),
                    vec![],
                    changes,
                )
                .await;
                id
            }
            Ok(None) => match sqlx::query_scalar::<_, i64>(
                "SELECT id FROM tags WHERE team_id = $1 AND name = $2",
            )
            .bind(team_id)
            .bind(&name)
            .fetch_one(pool)
            .await
            {
                Ok(id) => id,
                Err(e) => return query_err("attach_resource_tags", e),
            },
            Err(e) => return query_err("attach_resource_tags", e),
        };
        // `syncWithoutDetaching`
        if let Err(e) = sqlx::query(
            "INSERT INTO taggables (tag_id, taggable_id, taggable_type) VALUES ($1,$2,$3) \
             ON CONFLICT (tag_id, taggable_id, taggable_type) DO NOTHING",
        )
        .bind(tag_id)
        .bind(resource.id)
        .bind(resource.class)
        .execute(pool)
        .await
        {
            return query_err("attach_resource_tags", e);
        }
    }
    match resource_tags_json(pool, &resource).await {
        Ok(tags) => (StatusCode::CREATED, Json(Value::Array(tags))).into_response(),
        Err(e) => query_err("attach_resource_tags", e),
    }
}

/// DELETE /api/v1/{applications,services,databases}/:uuid/tags/:tag_uuid — detaches only.
async fn detach_resource_tag(
    state: crate::AppState,
    ctx: Ctx,
    kind: &str,
    uuid: String,
    tag_uuid: String,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let resource = match find_taggable(pool, kind, team_id, &uuid).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let tag_id = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM tags WHERE team_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(&tag_uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(id)) => id,
        Ok(None) => return not_found("Tag not found."),
        Err(e) => return query_err("detach_resource_tag", e),
    };
    match sqlx::query(
        "DELETE FROM taggables WHERE tag_id = $1 AND taggable_id = $2 AND taggable_type = $3",
    )
    .bind(tag_id)
    .bind(resource.id)
    .bind(resource.class)
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() == 0 => not_found("Tag not found on resource."),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Tag removed."}))).into_response(),
        Err(e) => query_err("detach_resource_tag", e),
    }
}

macro_rules! resource_tag_handlers {
    ($kind:literal, $list:ident, $attach:ident, $detach:ident) => {
        pub async fn $list(
            State(state): State<crate::AppState>,
            Extension(ctx): Extension<Ctx>,
            Path(uuid): Path<String>,
        ) -> Response {
            list_resource_tags(state, ctx, $kind, uuid).await
        }

        pub async fn $attach(
            State(state): State<crate::AppState>,
            Extension(ctx): Extension<Ctx>,
            Path(uuid): Path<String>,
            headers: HeaderMap,
            body: Bytes,
        ) -> Response {
            attach_resource_tags(state, ctx, $kind, uuid, headers, body).await
        }

        pub async fn $detach(
            State(state): State<crate::AppState>,
            Extension(ctx): Extension<Ctx>,
            Path((uuid, tag_uuid)): Path<(String, String)>,
        ) -> Response {
            detach_resource_tag(state, ctx, $kind, uuid, tag_uuid).await
        }
    };
}

resource_tag_handlers!(
    "applications",
    list_application_tags,
    attach_application_tags,
    detach_application_tag
);
resource_tag_handlers!(
    "services",
    list_service_tags,
    attach_service_tags,
    detach_service_tag
);
resource_tag_handlers!(
    "databases",
    list_database_tags,
    attach_database_tags,
    detach_database_tag
);
