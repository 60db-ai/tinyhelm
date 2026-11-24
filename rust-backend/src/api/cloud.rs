//! Native `/api/v1` handlers: cloud provider tokens (`CloudProviderTokensController`), cloud-init scripts
//! (`CloudInitScriptsController`) and S3/backup storages (`S3StoragesController`, read + delete).
//!
//! Left in Laravel: token create and `/validate` (call the provider API), storage create/update and
//! `/validate` (endpoint safety rules and live connection tests), cloud-init scripts that are YAML
//! (Symfony YAML parse errors are part of the 422), and storage deletes that must detach backups.

#[allow(unused_imports)]
use super::prelude::*;

use axum::extract::Request;
use serde_json::{Map, Value};

fn message(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "message": msg }))).into_response()
}

// =====================================================================================================
// Cloud provider tokens
// =====================================================================================================

const CLOUD_TOKEN_SPEC: ModelSpec = ModelSpec {
    hidden: &["token", "id"],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// `withCount('servers')`.
const CLOUD_TOKEN_ROW: &str = "to_jsonb(t) || jsonb_build_object('servers_count', \
    (SELECT count(*) FROM servers s WHERE s.cloud_provider_token_id = t.id))";

fn cloud_token_json(state: &crate::AppState, ctx: &Ctx, row: Value) -> Option<Value> {
    let visible: &[&str] = if ctx.can_read_sensitive() {
        &["token"]
    } else {
        &[]
    };
    credential_json(
        row,
        &CLOUD_TOKEN_SPEC,
        visible,
        &["token"],
        state.app_key.as_ref(),
    )
    .map(Value::Object)
}

/// GET /api/v1/cloud-tokens
pub async fn list_cloud_tokens(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {CLOUD_TOKEN_ROW} FROM cloud_provider_tokens t WHERE t.team_id = $1 ORDER BY t.id"
    ))
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_cloud_tokens", e),
    };
    let out: Option<Vec<Value>> = rows
        .into_iter()
        .map(|r| cloud_token_json(&state, &ctx, r))
        .collect();
    match out {
        Some(list) => ok_json(list),
        None => crate::proxy(State(state.clone()), req).await,
    }
}

async fn fetch_cloud_token(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {CLOUD_TOKEN_ROW} FROM cloud_provider_tokens t WHERE t.team_id = $1 AND t.uuid = $2"
    ))
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

/// GET /api/v1/cloud-tokens/:uuid
pub async fn get_cloud_token(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match fetch_cloud_token(pool, team_id, &uuid).await {
        Ok(Some(row)) => match cloud_token_json(&state, &ctx, row) {
            Some(v) => ok_json(v),
            None => crate::proxy(State(state.clone()), req).await,
        },
        Ok(None) => not_found("Cloud provider token not found."),
        Err(e) => query_err("get_cloud_token", e),
    }
}

/// PATCH /api/v1/cloud-tokens/:uuid — only the name changes.
pub async fn update_cloud_token(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (parts, bytes) = split_request(req).await;
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let mut v = Validator::new(&input, CUSTOM_API_MESSAGES);
    v.field("name", &[Rule::Required, Rule::Str, Rule::Max(255)]);
    v.reject_extra(&["name"]);
    if v.fails() {
        return v.response();
    }
    let row = match sqlx::query(
        "SELECT id, provider FROM cloud_provider_tokens WHERE team_id = $1 AND uuid = $2",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Cloud provider token not found."),
        Err(e) => return query_err("update_cloud_token", e),
    };
    let id: i64 = row.try_get("id").unwrap_or_default();
    let provider: Option<String> = row.try_get("provider").ok();
    let name = input_str(&input, "name").unwrap_or_default();
    // Eloquent touches updated_at only when the value changes.
    if let Err(e) = sqlx::query(&format!(
        "UPDATE cloud_provider_tokens SET name = $1, updated_at = {NOW} WHERE id = $2 AND name IS DISTINCT FROM $1"
    ))
    .bind(&name)
    .bind(id)
    .execute(pool)
    .await
    {
        return query_err("update_cloud_token", e);
    }
    audit::record(
        pool,
        &ctx,
        "api.cloud_token.updated",
        json!({"team_id": team_id, "cloud_token_uuid": uuid, "cloud_token_name": name, "provider": provider, "changed_fields": ["name"]}),
        "info",
    )
    .await;
    ok_json(json!({ "uuid": uuid }))
}

/// DELETE /api/v1/cloud-tokens/:uuid — 400 while servers use it.
pub async fn delete_cloud_token(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let row = match sqlx::query(
        "SELECT t.id, t.name, t.provider, \
                EXISTS (SELECT 1 FROM servers s WHERE s.cloud_provider_token_id = t.id) AS used \
         FROM cloud_provider_tokens t WHERE t.team_id = $1 AND t.uuid = $2",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Cloud provider token not found."),
        Err(e) => return query_err("delete_cloud_token", e),
    };
    if row.try_get::<bool, _>("used").unwrap_or(true) {
        return message(
            StatusCode::BAD_REQUEST,
            "Cannot delete token that is used by servers.",
        );
    }
    let id: i64 = row.try_get("id").unwrap_or_default();
    let name: Option<String> = row.try_get("name").ok();
    let provider: Option<String> = row.try_get("provider").ok();
    if let Err(e) = sqlx::query("DELETE FROM cloud_provider_tokens WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        return query_err("delete_cloud_token", e);
    }
    audit::record(
        pool,
        &ctx,
        "api.cloud_token.deleted",
        json!({"team_id": team_id, "cloud_token_uuid": uuid, "cloud_token_name": name, "provider": provider}),
        "info",
    )
    .await;
    ok_json(json!({"message": "Cloud provider token deleted."}))
}

// =====================================================================================================
// Cloud-init scripts
// =====================================================================================================

const CLOUD_INIT_SPEC: ModelSpec = ModelSpec {
    hidden: &["script", "id", "team_id"],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

fn cloud_init_json(state: &crate::AppState, ctx: &Ctx, row: Value) -> Option<Value> {
    let visible: &[&str] = if ctx.can_read_sensitive() {
        &["script"]
    } else {
        &[]
    };
    credential_json(
        row,
        &CLOUD_INIT_SPEC,
        visible,
        &["script"],
        state.app_key.as_ref(),
    )
    .map(Value::Object)
}

/// `ValidCloudInitYaml` runs Symfony's YAML parser on everything that is not a `#!` script; its error text
/// is part of the 422, so those requests are answered by Laravel.
// ponytail: YAML cloud-config writes stay in Laravel until a Symfony-compatible YAML error port exists.
fn needs_yaml_validation(input: &Map<String, Value>) -> bool {
    match input.get("script") {
        Some(Value::String(s)) => !s.is_empty() && !s.trim().starts_with("#!"),
        _ => false,
    }
}

/// GET /api/v1/cloud-init-scripts — newest first.
pub async fn list_cloud_init_scripts(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(c) FROM cloud_init_scripts c WHERE c.team_id = $1 ORDER BY c.created_at DESC, c.id",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_cloud_init_scripts", e),
    };
    let out: Option<Vec<Value>> = rows
        .into_iter()
        .map(|r| cloud_init_json(&state, &ctx, r))
        .collect();
    match out {
        Some(list) => ok_json(list),
        None => crate::proxy(State(state.clone()), req).await,
    }
}

async fn fetch_cloud_init(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(c) FROM cloud_init_scripts c WHERE c.team_id = $1 AND c.uuid = $2",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

/// GET /api/v1/cloud-init-scripts/:uuid
pub async fn get_cloud_init_script(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match fetch_cloud_init(pool, team_id, &uuid).await {
        Ok(Some(row)) => match cloud_init_json(&state, &ctx, row) {
            Some(v) => ok_json(v),
            None => crate::proxy(State(state.clone()), req).await,
        },
        Ok(None) => not_found("Cloud-init script not found."),
        Err(e) => query_err("get_cloud_init_script", e),
    }
}

/// POST /api/v1/cloud-init-scripts
pub async fn create_cloud_init_script(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (parts, bytes) = split_request(req).await;
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if needs_yaml_validation(&input) {
        return forward(state, parts, bytes).await;
    }
    let mut v = Validator::new(&input, CUSTOM_API_MESSAGES);
    v.field("name", &[Rule::Required, Rule::Str, Rule::Max(255)]);
    v.field("script", &[Rule::Required, Rule::Str]);
    v.reject_extra(&["name", "script"]);
    if v.fails() {
        return v.response();
    }
    let Some(app_key) = state.app_key else {
        return forward(state, parts, bytes).await;
    };
    let name = input_str(&input, "name").unwrap_or_default();
    let script = input_str(&input, "script").unwrap_or_default();
    let Some(cipher) = crate::crypto::encrypt_string(&script, &app_key) else {
        return forward(state, parts, bytes).await;
    };
    let row = match sqlx::query_scalar::<_, Value>(&format!(
        "INSERT INTO cloud_init_scripts (uuid, team_id, name, script, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,{NOW},{NOW}) RETURNING to_jsonb(cloud_init_scripts.*)"
    ))
    .bind(new_public_id())
    .bind(team_id)
    .bind(&name)
    .bind(&cipher)
    .fetch_one(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("create_cloud_init_script", e),
    };
    let uuid = row.get("uuid").cloned().unwrap_or(Value::Null);
    audit::record(
        pool,
        &ctx,
        "api.cloud_init_script.created",
        json!({"team_id": team_id, "cloud_init_script_uuid": uuid, "cloud_init_script_name": name}),
        "info",
    )
    .await;
    match cloud_init_json(&state, &ctx, row) {
        Some(v) => (StatusCode::CREATED, Json(v)).into_response(),
        None => query_err("create_cloud_init_script", sqlx::Error::RowNotFound),
    }
}

/// PATCH /api/v1/cloud-init-scripts/:uuid
pub async fn update_cloud_init_script(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (parts, bytes) = split_request(req).await;
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if input.is_empty() {
        return message(
            StatusCode::UNPROCESSABLE_ENTITY,
            "At least one field must be provided.",
        );
    }
    let row = match fetch_cloud_init(pool, team_id, &uuid).await {
        Ok(Some(r)) => as_map(r),
        Ok(None) => return not_found("Cloud-init script not found."),
        Err(e) => return query_err("update_cloud_init_script", e),
    };
    if needs_yaml_validation(&input) {
        return forward(state, parts, bytes).await;
    }
    let mut v = Validator::new(&input, CUSTOM_API_MESSAGES);
    v.field("name", &[Rule::Str, Rule::Max(255)]);
    v.field("script", &[Rule::Str]);
    v.reject_extra(&["name", "script"]);
    if v.fails() {
        return v.response();
    }
    let Some(app_key) = state.app_key else {
        return forward(state, parts, bytes).await;
    };
    let id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
    let stored_script = row
        .get("script")
        .and_then(Value::as_str)
        .and_then(|c| crate::crypto::decrypt(c, &app_key));
    let mut name = row.get("name").and_then(Value::as_str).map(str::to_string);
    let mut cipher = row
        .get("script")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut dirty = false;
    if input.contains_key("name") {
        let new_name = input_str(&input, "name");
        dirty |= new_name != name;
        name = new_name;
    }
    if input.contains_key("script") {
        let new_script = input_str(&input, "script");
        if new_script != stored_script {
            dirty = true;
            cipher = match new_script
                .as_deref()
                .map(|s| crate::crypto::encrypt_string(s, &app_key))
            {
                Some(Some(c)) => Some(c),
                Some(None) => return forward(state, parts, bytes).await,
                None => None,
            };
        }
    }
    if dirty {
        if let Err(e) = sqlx::query(&format!(
            "UPDATE cloud_init_scripts SET name = $1, script = $2, updated_at = {NOW} WHERE id = $3"
        ))
        .bind(&name)
        .bind(&cipher)
        .bind(id)
        .execute(pool)
        .await
        {
            return query_err("update_cloud_init_script", e);
        }
    }
    let changed: Vec<&str> = ["name", "script"]
        .into_iter()
        .filter(|f| input.contains_key(*f))
        .collect();
    audit::record(
        pool,
        &ctx,
        "api.cloud_init_script.updated",
        json!({"team_id": team_id, "cloud_init_script_uuid": uuid, "cloud_init_script_name": name, "changed_fields": changed}),
        "info",
    )
    .await;
    match fetch_cloud_init(pool, team_id, &uuid).await {
        Ok(Some(fresh)) => match cloud_init_json(&state, &ctx, fresh) {
            Some(v) => ok_json(v),
            None => query_err("update_cloud_init_script", sqlx::Error::RowNotFound),
        },
        Ok(None) => not_found("Cloud-init script not found."),
        Err(e) => query_err("update_cloud_init_script", e),
    }
}

/// DELETE /api/v1/cloud-init-scripts/:uuid
pub async fn delete_cloud_init_script(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let deleted = sqlx::query_scalar::<_, Option<String>>(
        "DELETE FROM cloud_init_scripts WHERE team_id = $1 AND uuid = $2 RETURNING name",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await;
    let name = match deleted {
        Ok(Some(name)) => name,
        Ok(None) => return not_found("Cloud-init script not found."),
        Err(e) => return query_err("delete_cloud_init_script", e),
    };
    audit::record(
        pool,
        &ctx,
        "api.cloud_init_script.deleted",
        json!({"team_id": team_id, "cloud_init_script_uuid": uuid, "cloud_init_script_name": name}),
        "info",
    )
    .await;
    ok_json(json!({"message": "Cloud-init script deleted."}))
}

// =====================================================================================================
// S3 / backup storages (read + delete)
// =====================================================================================================

const S3_SPEC: ModelSpec = ModelSpec {
    hidden: &["key", "secret", "id", "private_key_id"],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// Row plus the SFTP key UUID (`private_key_uuid` is set only for SFTP storages).
const S3_ROW: &str =
    "to_jsonb(s) || CASE WHEN s.type = 'sftp' THEN jsonb_build_object('private_key_uuid', \
    (SELECT k.uuid FROM private_keys k WHERE k.id = s.private_key_id)) ELSE '{}'::jsonb END";

fn s3_json(state: &crate::AppState, ctx: &Ctx, row: Value) -> Option<Value> {
    let visible: &[&str] = if ctx.can_read_sensitive() {
        &["key", "secret"]
    } else {
        &[]
    };
    credential_json(
        row,
        &S3_SPEC,
        visible,
        &["key", "secret"],
        state.app_key.as_ref(),
    )
    .map(Value::Object)
}

/// GET /api/v1/s3-storages — ordered by name.
pub async fn list_s3_storages(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {S3_ROW} FROM s3_storages s WHERE s.team_id = $1 ORDER BY s.name, s.id"
    ))
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_s3_storages", e),
    };
    let out: Option<Vec<Value>> = rows.into_iter().map(|r| s3_json(&state, &ctx, r)).collect();
    match out {
        Some(list) => ok_json(list),
        None => crate::proxy(State(state.clone()), req).await,
    }
}

/// GET /api/v1/s3-storages/:uuid
pub async fn get_s3_storage(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {S3_ROW} FROM s3_storages s WHERE s.team_id = $1 AND s.uuid = $2"
    ))
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => match s3_json(&state, &ctx, row) {
            Some(v) => ok_json(v),
            None => crate::proxy(State(state.clone()), req).await,
        },
        Ok(None) => not_found("S3 storage not found."),
        Err(e) => query_err("get_s3_storage", e),
    }
}

/// Backup schedules, destinations and copies that `S3Storage::detachFromBackups()` rewrites.
const S3_BACKUP_REFERENCES: &str = "SELECT \
    EXISTS (SELECT 1 FROM scheduled_database_backups WHERE s3_storage_id = $1) \
 OR EXISTS (SELECT 1 FROM scheduled_volume_backups WHERE s3_storage_id = $1) \
 OR EXISTS (SELECT 1 FROM scheduled_database_backup_s3_storage WHERE s3_storage_id = $1) \
 OR EXISTS (SELECT 1 FROM scheduled_volume_backup_s3_storage WHERE s3_storage_id = $1) \
 OR EXISTS (SELECT 1 FROM database_backup_s3_replicas WHERE s3_storage_id = $1) \
 OR EXISTS (SELECT 1 FROM volume_backup_s3_replicas WHERE s3_storage_id = $1)";

/// Columns of `s3_storages` in table order (the Auditable `deleted` change set).
const S3_COLUMNS: &[&str] = &[
    "id",
    "uuid",
    "name",
    "description",
    "region",
    "key",
    "secret",
    "bucket",
    "endpoint",
    "team_id",
    "created_at",
    "updated_at",
    "is_usable",
    "unusable_email_sent",
    "use_path_style_endpoint",
    "type",
    "private_key_id",
    "sftp_host_key",
    "skip_tls_verification",
];

/// DELETE /api/v1/s3-storages/:uuid — Laravel answers when backups still use the storage.
pub async fn delete_s3_storage(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let row = match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(s) FROM s3_storages s WHERE s.team_id = $1 AND s.uuid = $2",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => as_map(r),
        Ok(None) => return not_found("S3 storage not found."),
        Err(e) => return query_err("delete_s3_storage", e),
    };
    let id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
    match sqlx::query_scalar::<_, bool>(S3_BACKUP_REFERENCES)
        .bind(id)
        .fetch_one(pool)
        .await
    {
        Ok(true) => return crate::proxy(State(state.clone()), req).await,
        Ok(false) => {}
        Err(e) => return query_err("delete_s3_storage", e),
    }
    if let Err(e) = sqlx::query("DELETE FROM s3_storages WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        return query_err("delete_s3_storage", e);
    }
    let name = row.get("name").and_then(Value::as_str).map(str::to_string);
    let fields: Vec<String> = S3_COLUMNS.iter().map(|c| c.to_string()).collect();
    let changes = audit_changes(Some(&row), None, &fields, &["key", "secret"]);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "s3_storage",
        "deleted",
        Some(&uuid),
        name.as_deref(),
        vec![],
        changes,
    )
    .await;
    audit::record(
        pool,
        &ctx,
        "api.s3_storage.deleted",
        json!({"team_id": team_id, "s3_storage_uuid": uuid, "s3_storage_name": name}),
        "info",
    )
    .await;
    ok_json(json!({"message": "S3 storage deleted."}))
}
