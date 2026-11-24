//! Native `/api/v1` read handlers: application storages, destinations (Laravel `DestinationsController`)
//! and application destinations. Writes stay in Laravel (they change containers and networks on servers).
//!
//! ponytail: `/databases/{uuid}/storages` and `/services/{uuid}/storages` stay in Laravel: their file
//! storages embed the lazy-loaded database or service model, which needs those serializers exposed
//! like `embedded_application`.

#[allow(unused_imports)]
use super::prelude::*;

use serde_json::{Map, Value};

const APPLICATION_MORPH: &str = r"App\Models\Application";
const STANDALONE_MORPH: &str = r"App\Models\StandaloneDocker";
const SWARM_MORPH: &str = r"App\Models\SwarmDocker";
const BINARY_PLACEHOLDER: &str = "[binary file]";
const TOO_LARGE_PLACEHOLDER: &str = "[file too large to display]";

async fn team_application_id(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT a.id FROM applications a JOIN environments e ON e.id = a.environment_id \
         JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND a.uuid = $2 AND a.deleted_at IS NULL LIMIT 1",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

/// GET /api/v1/applications/{uuid}/storages
pub async fn list_application_storages(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let app_id = match team_application_id(pool, team_id, &uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return not_found("Application not found"),
        Err(e) => return query_err("list_application_storages", e),
    };
    let rows_of = |table: &'static str| {
        format!(
            "SELECT to_jsonb(v) FROM {table} v WHERE v.resource_type = $1 AND v.resource_id = $2 ORDER BY v.id"
        )
    };
    let persistent: Vec<Value> = match sqlx::query_scalar(&rows_of("local_persistent_volumes"))
        .bind(APPLICATION_MORPH)
        .bind(app_id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_application_storages", e),
    };
    let files: Vec<Value> = match sqlx::query_scalar(&rows_of("local_file_volumes"))
        .bind(APPLICATION_MORPH)
        .bind(app_id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_application_storages", e),
    };

    let persistent: Vec<Value> = persistent
        .into_iter()
        .map(|row| to_laravel(row, &ModelSpec::default()))
        .collect();
    // `exposeFileStorageContentIfAllowed()` evaluates `$storage->resource`, so every file storage carries
    // the lazy-loaded application.
    let resource = if files.is_empty() {
        None
    } else {
        // The storages response skips serializeApiResponse(), so server secrets are never stripped.
        match embedded_application(pool, app_id, state.app_key.as_ref(), false).await {
            Ok(a) => a,
            Err(e) => return query_err("list_application_storages", e),
        }
    };
    let sensitive = ctx.can_read_sensitive();
    let files: Vec<Value> = files
        .into_iter()
        .map(|row| file_storage_json(row, sensitive, state.app_key.as_ref(), resource.clone()))
        .collect();
    ok_json(json!({"persistent_storages": persistent, "file_storages": files}))
}

/// `LocalFileVolume` JSON: `content` (encrypted, hidden unless the token can read secrets), the
/// `is_binary` / `is_too_large` appends, and the loaded `resource`.
fn file_storage_json(
    row: Value,
    sensitive: bool,
    key: Option<&[u8; 32]>,
    resource: Option<Value>,
) -> Value {
    let mut m = match to_laravel(row, &ModelSpec::default()) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    let content = match m.remove("content") {
        Some(Value::String(payload)) => key.and_then(|k| crate::crypto::decrypt(&payload, k)),
        _ => None,
    };
    if sensitive {
        m.insert(
            "content".into(),
            content.clone().map_or(Value::Null, Value::String),
        );
    }
    m.insert(
        "is_binary".into(),
        json!(content.as_deref() == Some(BINARY_PLACEHOLDER)),
    );
    m.insert(
        "is_too_large".into(),
        json!(content.as_deref() == Some(TOO_LARGE_PLACEHOLDER)),
    );
    m.insert("resource".into(), resource.unwrap_or(Value::Null));
    Value::Object(m)
}

/// `DestinationsController::transform()` rows of one docker table, for servers of the team.
fn destination_select(table: &str, kind: &str, extra_filter: &str) -> String {
    format!(
        "SELECT jsonb_build_object('uuid', d.uuid, 'name', d.name, 'network', d.network, 'type', '{kind}', \
           'server_uuid', s.uuid, 'created_at', d.created_at, 'updated_at', d.updated_at) \
         FROM {table} d JOIN servers s ON s.id = d.server_id AND s.deleted_at IS NULL \
         WHERE s.team_id = $1 {extra_filter} ORDER BY d.id"
    )
}

/// Standalone destinations first, then swarm ones (`concat`), optionally filtered by `$2`.
async fn destinations(
    pool: &PgPool,
    team_id: i64,
    filter: &str,
    arg: &str,
) -> Result<Vec<Value>, sqlx::Error> {
    let mut out = Vec::new();
    for (table, kind) in [
        ("standalone_dockers", "standalone"),
        ("swarm_dockers", "swarm"),
    ] {
        let sql = destination_select(table, kind, filter);
        let mut q = sqlx::query_scalar::<_, Value>(&sql).bind(team_id);
        if !filter.is_empty() {
            q = q.bind(arg);
        }
        out.extend(q.fetch_all(pool).await?);
    }
    Ok(out
        .into_iter()
        .map(|row| to_laravel(row, &ModelSpec::default()))
        .collect())
}

/// The production body of Laravel's `ModelNotFoundException` 404 (debug mode adds a trace).
fn model_not_found(model: &str) -> Response {
    not_found(&format!(
        "No query results for model [App\\Models\\{model}]."
    ))
}

/// GET /api/v1/destinations
pub async fn list_destinations(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match destinations(pool, team_id, "", "").await {
        Ok(v) => ok_json(v),
        Err(e) => query_err("list_destinations", e),
    }
}

/// GET /api/v1/destinations/{uuid}
pub async fn get_destination(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match destinations(pool, team_id, "AND d.uuid = $2", &uuid).await {
        Ok(v) => match v.into_iter().next() {
            Some(d) => ok_json(d),
            // firstOrFail() on SwarmDocker after the StandaloneDocker lookup found nothing.
            None => model_not_found("SwarmDocker"),
        },
        Err(e) => query_err("get_destination", e),
    }
}

/// GET /api/v1/servers/{server_uuid}/destinations
pub async fn list_server_destinations(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(server_uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let exists: bool = match sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM servers WHERE team_id = $1 AND uuid = $2 AND deleted_at IS NULL)",
    )
    .bind(team_id)
    .bind(&server_uuid)
    .fetch_one(pool)
    .await
    {
        Ok(b) => b,
        Err(e) => return query_err("list_server_destinations", e),
    };
    if !exists {
        return model_not_found("Server");
    }
    match destinations(pool, team_id, "AND s.uuid = $2", &server_uuid).await {
        Ok(v) => ok_json(v),
        Err(e) => query_err("list_server_destinations", e),
    }
}

/// GET /api/v1/applications/{uuid}/destinations: the primary destination, then the additional networks.
pub async fn list_application_destinations(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let app = match sqlx::query(
        "SELECT a.id, a.destination_type, a.destination_id FROM applications a \
         JOIN environments e ON e.id = a.environment_id JOIN projects p ON p.id = e.project_id \
         WHERE p.team_id = $1 AND a.uuid = $2 AND a.deleted_at IS NULL LIMIT 1",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Application not found."),
        Err(e) => return query_err("list_application_destinations", e),
    };
    let app_id: i64 = app.try_get("id").unwrap_or_default();
    let destination_type: Option<String> = app.try_get("destination_type").ok().flatten();
    let destination_id: Option<i64> = app.try_get("destination_id").ok().flatten();

    let mut out: Vec<Value> = Vec::new();
    // The `destination` morphTo; the docker's `server` relation excludes trashed servers.
    let table = match destination_type.as_deref() {
        Some(STANDALONE_MORPH) => Some("standalone_dockers"),
        Some(SWARM_MORPH) => Some("swarm_dockers"),
        _ => None,
    };
    if let (Some(table), Some(id)) = (table, destination_id) {
        let primary: Option<Value> = match sqlx::query_scalar(&format!(
            "SELECT jsonb_build_object('uuid', d.uuid, 'name', d.name, 'network', d.network, \
               'server_uuid', s.uuid, 'server_id', d.server_id, 'is_primary', true) \
             FROM {table} d LEFT JOIN servers s ON s.id = d.server_id AND s.deleted_at IS NULL WHERE d.id = $1"
        ))
        .bind(id)
        .fetch_optional(pool)
        .await
        {
            Ok(v) => v,
            Err(e) => return query_err("list_application_destinations", e),
        };
        out.extend(primary);
    }
    let additional: Vec<Value> = match sqlx::query_scalar(
        "SELECT jsonb_build_object('uuid', d.uuid, 'name', d.name, 'network', d.network, \
           'server_uuid', s.uuid, 'server_id', COALESCE(ad.server_id, d.server_id), 'is_primary', false) \
         FROM additional_destinations ad JOIN standalone_dockers d ON d.id = ad.standalone_docker_id \
         LEFT JOIN servers s ON s.id = d.server_id AND s.deleted_at IS NULL \
         WHERE ad.application_id = $1 ORDER BY ad.id",
    )
    .bind(app_id)
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => return query_err("list_application_destinations", e),
    };
    out.extend(additional);
    ok_json(out)
}
