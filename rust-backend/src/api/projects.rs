//! Native `/api/v1` handlers: projects and their environments (Laravel `ProjectController`).

#[allow(unused_imports)]
use super::prelude::*;

use axum::{body::Bytes, http::HeaderMap};
use serde_json::{Map, Value};

// =====================================================================================================
// Projects
// =====================================================================================================

/// `Project::$auditExclude`.
const PROJECT_AUDIT_EXCLUDE: &[&str] = &["canvas_position"];
/// `Environment::$auditExclude`.
const ENVIRONMENT_AUDIT_EXCLUDE: &[&str] = &["canvas_positions"];

/// GET /api/v1/projects — `select('id', 'name', 'description', 'uuid')`, unordered (as Laravel).
pub async fn list_projects(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object('id', id, 'name', name, 'description', description, 'uuid', uuid) \
         FROM projects WHERE team_id = $1",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(rows),
        Err(e) => query_err("list_projects", e),
    }
}

async fn project_row(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<Option<Map<String, Value>>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(p) FROM projects p WHERE team_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
    .map(|r| r.map(as_map))
}

/// GET /api/v1/projects/:uuid/environments — `select('id', 'name', 'uuid')`.
pub async fn list_environments(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let project_id = match project_row(pool, team_id, &uuid).await {
        Ok(Some(p)) => p.get("id").and_then(Value::as_i64),
        Ok(None) => return not_found("Project not found."),
        Err(e) => return query_err("list_environments", e),
    };
    match sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object('id', id, 'name', name, 'uuid', uuid) FROM environments WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(rows),
        Err(e) => query_err("list_environments", e),
    }
}

fn validate_project(input: &Map<String, Value>, name_required: bool) -> Option<Response> {
    let presence = if name_required {
        Rule::Required
    } else {
        Rule::Nullable
    };
    let mut v = Validator::new(input, NAME_DESCRIPTION_MESSAGES);
    v.field(
        "name",
        &[
            presence,
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
    v.reject_extra(&["name", "description"]);
    v.fails().then(|| v.response())
}

/// `HasSafeStringAttribute`: `strip_tags` on name (PHP 8 turns null into "") and description.
fn safe_name(value: Option<String>) -> String {
    strip_tags(&value.unwrap_or_default())
}

fn safe_description(value: Option<String>) -> Option<String> {
    value.map(|d| strip_tags(&d))
}

/// POST /api/v1/projects — creates the project, its settings row and a `production` environment.
pub async fn create_project(
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
    if let Some(r) = validate_project(&input, true) {
        return r;
    }
    let name = safe_name(input_str(&input, "name"));
    let description = safe_description(input_str(&input, "description"));
    let uuid = new_public_id();
    let env_uuid = new_public_id();

    let created: Result<i64, sqlx::Error> = async {
        let mut tx = pool.begin().await?;
        let project_id: i64 = sqlx::query_scalar(&format!(
            "INSERT INTO projects (uuid, name, description, team_id, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,{NOW},{NOW}) RETURNING id"
        ))
        .bind(&uuid)
        .bind(&name)
        .bind(&description)
        .bind(team_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(&format!("INSERT INTO project_settings (project_id, created_at, updated_at) VALUES ($1, {NOW}, {NOW})"))
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(&format!(
            "INSERT INTO environments (name, project_id, uuid, created_at, updated_at) \
             VALUES ('production', $1, $2, {NOW}, {NOW})"
        ))
        .bind(project_id)
        .bind(&env_uuid)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(project_id)
    }
    .await;
    let project_id = match created {
        Ok(id) => id,
        Err(e) => return query_err("create_project", e),
    };

    let new =
        as_map(json!({"name": name, "description": description, "team_id": team_id, "uuid": uuid}));
    let changes = audit_changes(None, Some(&new), &fields_of(&new), PROJECT_AUDIT_EXCLUDE);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "project",
        "created",
        Some(&uuid),
        Some(&name),
        vec![],
        changes,
    )
    .await;
    let env_new = as_map(json!({"name": "production", "project_id": project_id, "uuid": env_uuid}));
    let env_changes = audit_changes(
        None,
        Some(&env_new),
        &fields_of(&env_new),
        ENVIRONMENT_AUDIT_EXCLUDE,
    );
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "environment",
        "created",
        Some(&env_uuid),
        Some("production"),
        vec![],
        env_changes,
    )
    .await;

    (StatusCode::CREATED, Json(json!({ "uuid": uuid }))).into_response()
}

/// PATCH /api/v1/projects/:uuid
pub async fn update_project(
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
    if let Some(r) = validate_project(&input, false) {
        return r;
    }
    let old = match project_row(pool, team_id, &uuid).await {
        Ok(Some(p)) => p,
        Ok(None) => return not_found("Project not found."),
        Err(e) => return query_err("update_project", e),
    };

    let mut new = old.clone();
    if input.contains_key("name") {
        new.insert(
            "name".into(),
            Value::String(safe_name(input_str(&input, "name"))),
        );
    }
    if input.contains_key("description") {
        new.insert(
            "description".into(),
            json!(safe_description(input_str(&input, "description"))),
        );
    }
    let changed = changed_fields(
        &old,
        &new,
        PROJECT_AUDIT_EXCLUDE,
        &["id", "uuid", "name", "description"],
    );
    if new != old {
        if let Err(e) = sqlx::query(&format!(
            "UPDATE projects SET name = $1, description = $2, updated_at = {NOW} WHERE id = $3"
        ))
        .bind(new.get("name").and_then(Value::as_str))
        .bind(new.get("description").and_then(Value::as_str))
        .bind(old.get("id").and_then(Value::as_i64))
        .execute(pool)
        .await
        {
            return query_err("update_project", e);
        }
        if !changed.is_empty() {
            let changes = audit_changes(Some(&old), Some(&new), &changed, PROJECT_AUDIT_EXCLUDE);
            let name = new.get("name").and_then(Value::as_str).map(str::to_string);
            record_mutation(
                &state,
                pool,
                &ctx,
                team_id,
                "project",
                "updated",
                Some(&uuid),
                name.as_deref(),
                changed,
                changes,
            )
            .await;
        }
    }

    (
        StatusCode::CREATED,
        Json(json!({"uuid": uuid, "name": new.get("name"), "description": new.get("description")})),
    )
        .into_response()
}

/// Resource tables counted by `Project::isEmpty()`.
const PROJECT_RESOURCE_TABLES: &[&str] = &[
    "applications",
    "standalone_redis",
    "standalone_postgresqls",
    "standalone_mysqls",
    "standalone_keydbs",
    "standalone_dragonflies",
    "standalone_clickhouses",
    "standalone_mariadbs",
    "standalone_mongodbs",
    "standalone_sqlites",
    "services",
];

/// DELETE /api/v1/projects/:uuid — only empty projects; environments and settings go first.
pub async fn delete_project(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let project = match project_row(pool, team_id, &uuid).await {
        Ok(Some(p)) => p,
        Ok(None) => return not_found("Project not found."),
        Err(e) => return query_err("delete_project", e),
    };
    let project_id = project
        .get("id")
        .and_then(Value::as_i64)
        .unwrap_or_default();

    let count_sql = PROJECT_RESOURCE_TABLES
        .iter()
        .map(|t| format!("(SELECT count(*) FROM {t} WHERE environment_id IN (SELECT id FROM environments WHERE project_id = $1))"))
        .collect::<Vec<_>>()
        .join(" + ");
    match sqlx::query_scalar::<_, i64>(&format!("SELECT {count_sql}"))
        .bind(project_id)
        .fetch_one(pool)
        .await
    {
        Ok(0) => {}
        Ok(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"message": "Project has resources, so it cannot be deleted."})),
            )
                .into_response();
        }
        Err(e) => return query_err("delete_project", e),
    }

    let environments = match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(e) FROM environments e WHERE project_id = $1 ORDER BY id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => return query_err("delete_project", e),
    };
    for env in environments.into_iter().map(as_map) {
        if let Err(e) = sqlx::query("DELETE FROM environments WHERE id = $1")
            .bind(env.get("id").and_then(Value::as_i64))
            .execute(pool)
            .await
        {
            return query_err("delete_project", e);
        }
        let changes = audit_changes(
            Some(&env),
            None,
            &fields_of(&env),
            ENVIRONMENT_AUDIT_EXCLUDE,
        );
        let env_uuid = env.get("uuid").and_then(Value::as_str);
        let env_name = env.get("name").and_then(Value::as_str);
        record_mutation(
            &state,
            pool,
            &ctx,
            team_id,
            "environment",
            "deleted",
            env_uuid,
            env_name,
            vec![],
            changes,
        )
        .await;
    }
    let deleted: Result<(), sqlx::Error> = async {
        sqlx::query("DELETE FROM project_settings WHERE project_id = $1")
            .bind(project_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(project_id)
            .execute(pool)
            .await?;
        Ok(())
    }
    .await;
    if let Err(e) = deleted {
        return query_err("delete_project", e);
    }
    let changes = audit_changes(
        Some(&project),
        None,
        &fields_of(&project),
        PROJECT_AUDIT_EXCLUDE,
    );
    let name = project.get("name").and_then(Value::as_str);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "project",
        "deleted",
        Some(&uuid),
        name,
        vec![],
        changes,
    )
    .await;

    (StatusCode::OK, Json(json!({"message": "Project deleted."}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tags_matches_php() {
        assert_eq!(strip_tags("a<b>c</b>d"), "acd");
        assert_eq!(strip_tags("a < b"), "a < b");
        assert_eq!(strip_tags("x<script src='a>b'>y"), "xy");
        assert_eq!(strip_tags("tail<open"), "tail");
    }

    #[test]
    fn validator_matches_laravel_rules() {
        let input = as_map(json!({"name": "ab", "description": null, "extra": 1}));
        let r = validate_project(&input, true);
        assert!(r.is_some());
        let mut v = Validator::new(&input, NAME_DESCRIPTION_MESSAGES);
        v.field(
            "name",
            &[
                Rule::Required,
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
        v.reject_extra(&["name", "description"]);
        assert_eq!(
            v.errors["name"],
            json!(["The name must be at least 3 characters."])
        );
        assert!(
            v.errors.get("description").is_none(),
            "nullable null skips the rules"
        );
        assert_eq!(v.errors["extra"], json!(["This field is not allowed."]));

        let missing = as_map(json!({"description": "ok"}));
        let mut v = Validator::new(&missing, CUSTOM_API_MESSAGES);
        v.field("name", &[Rule::Required, Rule::Str]);
        assert_eq!(v.errors["name"], json!(["This field is required."]));

        let bad_bool = as_map(json!({"is_literal": "yes"}));
        let mut v = Validator::new(&bad_bool, &[]);
        v.field("is_literal", &[Rule::Boolean]);
        assert_eq!(
            v.errors["is_literal"],
            json!(["The is literal field must be true or false."])
        );
    }

    #[test]
    fn audit_changes_excludes_like_auditable() {
        let new = as_map(
            json!({"name": "p", "description": null, "team_id": 1, "uuid": "u", "canvas_position": [1]}),
        );
        let c = audit_changes(None, Some(&new), &fields_of(&new), PROJECT_AUDIT_EXCLUDE);
        let keys: Vec<&String> = c.keys().collect();
        assert_eq!(keys, vec!["description", "name"]);
        assert_eq!(c["name"], json!({"old": null, "new": "p"}));
    }

    #[test]
    fn input_cleaning_trims_and_nulls() {
        let mut h = HeaderMap::new();
        h.insert("content-type", "application/json".parse().unwrap());
        let m = json_input(
            &h,
            br#"{"name":"  x  ","description":"   ","value":"  v  "}"#,
        )
        .unwrap();
        assert_eq!(m["name"], "x");
        assert_eq!(m["description"], Value::Null);
        assert_eq!(m["value"], "  v  ");
        assert!(json_input(&h, b"{}").is_err());
        h.insert("content-type", "text/plain".parse().unwrap());
        assert!(json_input(&h, br#"{"a":1}"#).is_err());
    }
}
