//! Native `PATCH /applications/{uuid}` and `PATCH /databases/{uuid}` for the database-only subset.
//!
//! Only requests whose body changes `name` and/or `description` are answered here. Every other field
//! can dispatch jobs, reload the proxy, touch the server, run the compose parser, or depends on
//! validation not ported yet, so those requests are forwarded to Laravel unchanged.
//!
//! ponytail: renames do not clear Laravel's global-search cache (`ClearsGlobalSearchCache`, 300 s TTL in
//! Redis); search results can show the old name for up to five minutes until the cache layer is ported.

#[allow(unused_imports)]
use super::prelude::*;

use axum::body::{to_bytes, Body, Bytes};
use axum::extract::Request;
use axum::http::request::Parts;
use serde_json::{Map, Value};

/// Body keys the native path handles; any other key sends the request to Laravel.
const NATIVE_FIELDS: &[&str] = &["name", "description"];

/// Split the request so the body can be inspected and still forwarded unchanged.
async fn read(req: Request) -> Result<(Parts, Bytes), Response> {
    let (parts, body) = req.into_parts();
    match to_bytes(body, usize::MAX).await {
        Ok(bytes) => Ok((parts, bytes)),
        Err(_) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"message": "unreadable body"})),
        )
            .into_response()),
    }
}

async fn delegate(state: crate::AppState, parts: Parts, bytes: Bytes) -> Response {
    crate::proxy(State(state), Request::from_parts(parts, Body::from(bytes))).await
}

fn only_native_fields(input: &Map<String, Value>) -> bool {
    input.keys().all(|k| NATIVE_FIELDS.contains(&k.as_str()))
}

/// `nameRules(required: false, nullable: false)` + `descriptionRules()`; `None` when valid.
fn validate_name_description(
    input: &Map<String, Value>,
    messages: &[(&str, &str)],
) -> Option<Response> {
    let mut v = Validator::new(input, messages);
    v.field(
        "name",
        &[
            Rule::Str,
            Rule::Min(3),
            Rule::Max(255),
            Rule::Regex(NAME_PATTERN),
        ],
    );
    v.field(
        "description",
        &[
            Rule::Nullable,
            Rule::Str,
            Rule::Max(255),
            Rule::Regex(DESCRIPTION_PATTERN),
        ],
    );
    v.fails().then(|| v.response())
}

/// `$model->fill($request->only(['name', 'description']))` with `HasSafeStringAttribute`.
fn apply_safe_strings(old: &Map<String, Value>, input: &Map<String, Value>) -> Map<String, Value> {
    let mut new = old.clone();
    if input.contains_key("name") {
        // PHP 8 strip_tags(null) is "" (validation already rejects null names).
        let name = input_str(input, "name").unwrap_or_default();
        new.insert("name".into(), Value::String(strip_tags(&name)));
    }
    if input.contains_key("description") {
        new.insert(
            "description".into(),
            input_str(input, "description").map_or(Value::Null, |d| Value::String(strip_tags(&d))),
        );
    }
    new
}

/// Eloquent `getChanges()` for name/description, in column order.
fn changed_name_description(old: &Map<String, Value>, new: &Map<String, Value>) -> Vec<String> {
    NATIVE_FIELDS
        .iter()
        .filter(|f| old.get(**f) != new.get(**f))
        .map(|f| f.to_string())
        .collect()
}

fn str_of<'a>(m: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    m.get(k).and_then(Value::as_str)
}

async fn write_name_description(
    pool: &PgPool,
    table: &str,
    id: i64,
    new: &Map<String, Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        "UPDATE {table} SET name = $1, description = $2, updated_at = {NOW} WHERE id = $3"
    ))
    .bind(str_of(new, "name"))
    .bind(str_of(new, "description"))
    .bind(id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// PATCH /api/v1/applications/:uuid (`ApplicationsController::update_by_uuid`), name/description only.
pub async fn update_application(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (parts, bytes) = match read(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if !only_native_fields(&input) {
        return delegate(state.clone(), parts, bytes).await;
    }

    // `ownedByCurrentTeamAPI` + the destination server the controller dereferences.
    let row: Result<Option<(Value, bool)>, sqlx::Error> = sqlx::query_as(
        r"SELECT jsonb_build_object('id', a.id, 'name', a.name, 'description', a.description),
                 EXISTS (SELECT 1 FROM standalone_dockers d JOIN servers s ON s.id = d.server_id
                         WHERE a.destination_type = 'App\Models\StandaloneDocker' AND d.id = a.destination_id)
                 OR EXISTS (SELECT 1 FROM swarm_dockers d JOIN servers s ON s.id = d.server_id
                         WHERE a.destination_type = 'App\Models\SwarmDocker' AND d.id = a.destination_id)
          FROM applications a JOIN environments e ON e.id = a.environment_id JOIN projects p ON p.id = e.project_id
          WHERE p.team_id = $1 AND a.uuid = $2 AND a.deleted_at IS NULL LIMIT 1",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await;
    let old = match row {
        Ok(Some((Value::Object(old), true))) => old,
        // Laravel dereferences `destination->server`; a missing one is its own (error) behavior.
        Ok(Some(_)) => return delegate(state.clone(), parts, bytes).await,
        Ok(None) => return not_found("Application not found"),
        Err(e) => return query_err("update_application", e),
    };

    let mut messages: Vec<(&str, &str)> = NAME_DESCRIPTION_MESSAGES.to_vec();
    messages.push((
        "docker_compose_domains.*.array",
        "An item in the docker_compose_domains array has invalid fields. Only name, domain, and redirect fields are supported.",
    ));
    if let Some(r) = validate_name_description(&input, &messages) {
        return r;
    }

    let new = apply_safe_strings(&old, &input);
    let changed = changed_name_description(&old, &new);
    if !changed.is_empty() {
        let id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
        if let Err(e) = write_name_description(pool, "applications", id, &new).await {
            return query_err("update_application", e);
        }
        // The model save runs `withoutAuditLogging`; only the controller event is recorded.
        audit::record(
            pool,
            &ctx,
            "api.application.updated",
            json!({
                "team_id": team_id,
                "application_uuid": uuid,
                "application_name": new.get("name"),
                "changed_fields": changed,
            }),
            "info",
        )
        .await;
    }

    (StatusCode::OK, Json(json!({"uuid": uuid}))).into_response()
}

/// PATCH /api/v1/databases/:uuid (`DatabasesController::update_by_uuid`), name/description only.
pub async fn update_database(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (parts, bytes) = match read(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if !only_native_fields(&input) {
        return delegate(state.clone(), parts, bytes).await;
    }

    // `customApiValidator` runs before the lookup, so a bad name fails even for an unknown uuid.
    let mut messages: Vec<(&str, &str)> = CUSTOM_API_MESSAGES.to_vec();
    messages.extend_from_slice(NAME_DESCRIPTION_MESSAGES);
    if let Some(r) = validate_name_description(&input, &messages) {
        return r;
    }

    // `queryDatabaseByUuidWithinTeam`: first engine with the uuid; the instance database (id 0) is hidden.
    let mut found = None;
    for engine in ENGINES {
        match engine_rows(pool, engine, team_id, Some(&uuid)).await {
            Ok(rows) => {
                if let Some((_, row)) = rows.into_iter().next() {
                    found = Some((engine, row));
                    break;
                }
            }
            Err(e) => return query_err("update_database", e),
        }
    }
    let Some((engine, Value::Object(old))) = found else {
        return not_found("Database not found.");
    };
    if engine.table == "standalone_postgresqls" && old.get("id").and_then(Value::as_i64) == Some(0)
    {
        return not_found("Database not found.");
    }

    let new = apply_safe_strings(&old, &input);
    let changed = changed_name_description(&old, &new);
    let resource_type = engine.kind.replace('-', "_");
    if !changed.is_empty() {
        let id = old.get("id").and_then(Value::as_i64).unwrap_or_default();
        if let Err(e) = write_name_description(pool, engine.table, id, &new).await {
            return query_err("update_database", e);
        }
        // `Auditable::updated` on the model.
        let changes = audit_changes(Some(&old), Some(&new), &changed, engine.hidden);
        record_mutation(
            &state,
            pool,
            &ctx,
            team_id,
            &resource_type,
            "updated",
            Some(&uuid),
            str_of(&new, "name"),
            changed,
            changes,
        )
        .await;
    }

    // The controller event is recorded on every successful update, changed or not.
    let requested: Vec<&str> = NATIVE_FIELDS
        .iter()
        .copied()
        .filter(|f| input.contains_key(*f))
        .collect();
    audit::record(
        pool,
        &ctx,
        "api.database.updated",
        json!({
            "team_id": team_id,
            "database_uuid": uuid,
            "database_name": new.get("name"),
            "database_type": engine.kind,
            "changed_fields": requested,
        }),
        "info",
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({"message": "Database updated."})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn delegates_anything_but_name_and_description() {
        let only = json!({"name": "x", "description": null});
        assert!(only_native_fields(only.as_object().unwrap()));
        let other = json!({"name": "x", "instant_deploy": true});
        assert!(!only_native_fields(other.as_object().unwrap()));
    }

    #[test]
    fn safe_strings_and_changes() {
        let old = json!({"id": 1, "name": "app", "description": "d"});
        let input = json!({"name": "<b>new</b>", "description": null});
        let new = apply_safe_strings(old.as_object().unwrap(), input.as_object().unwrap());
        assert_eq!(new["name"], "new");
        assert_eq!(new["description"], Value::Null);
        assert_eq!(
            changed_name_description(old.as_object().unwrap(), &new),
            vec!["name", "description"]
        );
    }
}
