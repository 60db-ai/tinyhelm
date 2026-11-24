//! Native `/api/v1` handlers: projects and their environments (Laravel `ProjectController`).
//!
//! Also holds the request/validation/mutation-audit helpers every native write handler needs
//! (`json_input`, `Validator`, `record_mutation`, ...). ponytail: they live here until a second
//! domain needs them; then move them to `api/validate.rs` and `api/audit.rs`.

#[allow(unused_imports)]
use super::prelude::*;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::{body::Bytes, http::HeaderMap};
use rand::Rng;
use serde_json::{Map, Value};

// =====================================================================================================
// Request input: validateIncomingRequest() + TrimStrings + ConvertEmptyStringsToNull
// =====================================================================================================

/// Eloquent timestamps: app time zone (UTC), whole seconds (`Y-m-d H:i:s`). The Postgres session
/// time zone may differ, so plain `now()` would store local time.
pub(crate) const NOW: &str = "date_trunc('second', now() AT TIME ZONE 'UTC')";

fn invalid_request(error: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"message": "Invalid request.", "error": error})),
    )
        .into_response()
}

/// Laravel `validateIncomingRequest()` followed by the global `TrimStrings` (except `value`) and
/// `ConvertEmptyStringsToNull` middleware. Returns the input as Laravel's `$request->all()`.
pub(crate) fn json_input(headers: &HeaderMap, body: &[u8]) -> Result<Map<String, Value>, Response> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !(content_type.contains("/json") || content_type.contains("+json")) {
        return Err(invalid_request("Content-Type must be application/json."));
    }
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return Err(invalid_request("Invalid JSON.")),
    };
    // PHP `! json_decode($content)`: null, false, 0, "", "0" and [] are falsy; objects never are.
    let map = match parsed {
        Value::Object(map) => map,
        Value::Array(items) if !items.is_empty() => items
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => return Err(invalid_request("Invalid JSON.")),
    };
    if map.is_empty() {
        return Err(invalid_request("Empty JSON."));
    }
    Ok(map
        .into_iter()
        .map(|(k, v)| {
            let v = clean_input(&k, v);
            (k, v)
        })
        .collect())
}

/// `TrimStrings` (except keys named `value`) then `ConvertEmptyStringsToNull`, recursively.
fn clean_input(key: &str, value: Value) -> Value {
    match value {
        Value::String(s) => {
            let s = if key == "value" {
                s
            } else {
                laravel_trim(&s).to_string()
            };
            if s.is_empty() {
                Value::Null
            } else {
                Value::String(s)
            }
        }
        Value::Array(items) => {
            Value::Array(items.into_iter().map(|v| clean_input("", v)).collect())
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let v = clean_input(&k, v);
                    (k, v)
                })
                .collect(),
        ),
        other => other,
    }
}

/// `TrimStrings`: whitespace plus BOM, zero-width space and LRM.
fn laravel_trim(s: &str) -> &str {
    s.trim_matches(|c: char| {
        c.is_whitespace() || c == '\u{FEFF}' || c == '\u{200B}' || c == '\u{200E}'
    })
}

/// PHP `strip_tags`: drops `<...>` tags (quote-aware); a `<` followed by whitespace or the end of
/// input is kept, as PHP does.
// ponytail: no allowed-tags argument; add it when a caller passes one.
pub(crate) fn strip_tags(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '<' && chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) {
            let mut quote: Option<char> = None;
            i += 1;
            while i < chars.len() {
                let d = chars[i];
                match quote {
                    Some(q) if d == q => quote = None,
                    Some(_) => {}
                    None if d == '"' || d == '\'' => quote = Some(d),
                    None if d == '>' => break,
                    None => {}
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `new_public_id()`: `Str::lower(Str::random(24))`.
pub(crate) fn new_public_id() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

/// PHP `$request->boolean()`: FILTER_VALIDATE_BOOLEAN.
pub(crate) fn input_bool(input: &Map<String, Value>, key: &str) -> bool {
    match input.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() == Some(1.0),
        Some(Value::String(s)) => matches!(s.to_lowercase().as_str(), "1" | "true" | "on" | "yes"),
        _ => false,
    }
}

/// `$request->input($key)` as a PHP string (null when absent or null).
pub(crate) fn input_str(input: &Map<String, Value>, key: &str) -> Option<String> {
    match input.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "1".into() } else { String::new() }),
        _ => None,
    }
}

// =====================================================================================================
// Validator: the subset of Laravel's Validator these routes use, with the same messages
// =====================================================================================================

#[derive(Clone, Copy)]
pub(crate) enum Rule {
    Required,
    Nullable,
    Str,
    Min(usize),
    Max(usize),
    Regex(&'static str),
    Boolean,
}

pub(crate) const NAME_PATTERN: &str = r"^[\p{L}\p{M}\p{N}\s\-_.@/&()#,:+]+$";
pub(crate) const DESCRIPTION_PATTERN: &str = r#"^[\p{L}\p{M}\p{N}\s\-_.,!?()'"+=*@/&]+$"#;
pub(crate) const ENV_KEY_PATTERN: &str = r"\A[A-Za-z_][A-Za-z0-9_.]*\z";

/// `ValidationPatterns::combinedMessages()` (name + description).
pub(crate) const NAME_DESCRIPTION_MESSAGES: &[(&str, &str)] = &[
    ("name.regex", "The name may only contain letters (including Unicode), numbers, spaces, and these characters: - _ . / @ & ( ) # , : +"),
    ("name.min", "The name must be at least :min characters."),
    ("name.max", "The name may not be greater than :max characters."),
    ("description.regex", "The description may only contain letters (including Unicode), numbers, spaces, and common punctuation: - _ . , ! ? ( ) ' \" + = * / @ &"),
    ("description.max", "The description may not be greater than :max characters."),
];

/// `customApiValidator()` messages.
pub(crate) const CUSTOM_API_MESSAGES: &[(&str, &str)] = &[("required", "This field is required.")];

fn compiled(pattern: &'static str) -> regex::Regex {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, regex::Regex>>> = OnceLock::new();
    let mut map = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.entry(pattern)
        .or_insert_with(|| regex::Regex::new(pattern).expect("valid pattern"))
        .clone()
}

/// Laravel validation errors: field -> messages, in rule order.
pub(crate) struct Validator<'a> {
    input: &'a Map<String, Value>,
    messages: &'a [(&'a str, &'a str)],
    errors: Map<String, Value>,
}

impl<'a> Validator<'a> {
    pub(crate) fn new(input: &'a Map<String, Value>, messages: &'a [(&'a str, &'a str)]) -> Self {
        Self {
            input,
            messages,
            errors: Map::new(),
        }
    }

    /// Validate one attribute like `Validator::passes()`: rules run in order, non-implicit rules are
    /// skipped for absent or blank values (and for null when `nullable`), a failed `required` stops.
    pub(crate) fn field(&mut self, name: &str, rules: &[Rule]) -> &mut Self {
        let value = self.input.get(name);
        let nullable = rules.iter().any(|r| matches!(r, Rule::Nullable));
        let blank_string = matches!(value, Some(Value::String(s)) if s.trim().is_empty());
        let present = value.is_some() && !blank_string;
        for rule in rules {
            let implicit = matches!(rule, Rule::Required);
            if matches!(rule, Rule::Nullable) {
                continue;
            }
            if !implicit && (!present || (nullable && matches!(value, Some(Value::Null)))) {
                continue;
            }
            if !passes(*rule, value) {
                let msg = self.message(name, *rule, value);
                self.add(name, &msg);
                if implicit {
                    break;
                }
            }
        }
        self
    }

    pub(crate) fn add(&mut self, field: &str, message: &str) {
        let entry = self
            .errors
            .entry(field.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(list) = entry {
            list.push(Value::String(message.to_string()));
        }
    }

    /// Adds "This field is not allowed." for every input key outside `allowed`.
    pub(crate) fn reject_extra(&mut self, allowed: &[&str]) -> &mut Self {
        let extra: Vec<String> = self
            .input
            .keys()
            .filter(|k| !allowed.contains(&k.as_str()))
            .cloned()
            .collect();
        for field in extra {
            self.add(&field, "This field is not allowed.");
        }
        self
    }

    pub(crate) fn fails(&self) -> bool {
        !self.errors.is_empty()
    }

    /// The 422 every API controller returns.
    pub(crate) fn response(&self) -> Response {
        validation_failed(Value::Object(self.errors.clone()))
    }

    fn message(&self, field: &str, rule: Rule, value: Option<&Value>) -> String {
        let is_array = value.is_some_and(Value::is_array);
        let (key, default, param): (&str, &str, Option<(&str, usize)>) = match rule {
            Rule::Required => ("required", "The :attribute field is required.", None),
            Rule::Str => ("string", "The :attribute field must be a string.", None),
            Rule::Min(n) if is_array => (
                "min",
                "The :attribute field must have at least :min items.",
                Some(("min", n)),
            ),
            Rule::Min(n) => (
                "min",
                "The :attribute field must be at least :min characters.",
                Some(("min", n)),
            ),
            Rule::Max(n) if is_array => (
                "max",
                "The :attribute field must not have more than :max items.",
                Some(("max", n)),
            ),
            Rule::Max(n) => (
                "max",
                "The :attribute field must not be greater than :max characters.",
                Some(("max", n)),
            ),
            Rule::Regex(_) => ("regex", "The :attribute field format is invalid.", None),
            Rule::Boolean => (
                "boolean",
                "The :attribute field must be true or false.",
                None,
            ),
            Rule::Nullable => ("nullable", "", None),
        };
        let specific = format!("{field}.{key}");
        let template = self
            .messages
            .iter()
            .find(|(k, _)| *k == specific)
            .or_else(|| self.messages.iter().find(|(k, _)| *k == key))
            .map(|(_, m)| *m)
            .unwrap_or(default);
        let mut msg = template.replace(":attribute", &field.replace('_', " "));
        if let Some((name, n)) = param {
            msg = msg.replace(&format!(":{name}"), &n.to_string());
        }
        msg
    }
}

pub(crate) fn validation_failed(errors: Value) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": "Validation failed.", "errors": errors})),
    )
        .into_response()
}

/// PHP string form of a scalar for size/regex checks.
fn php_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn passes(rule: Rule, value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return !matches!(rule, Rule::Required);
    };
    match rule {
        Rule::Required => match value {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        },
        Rule::Nullable => true,
        Rule::Str => value.is_string(),
        Rule::Min(n) | Rule::Max(n) => {
            let size = match value {
                Value::Array(a) => a.len(),
                v => php_string(v).map(|s| s.chars().count()).unwrap_or(0),
            };
            if matches!(rule, Rule::Min(_)) {
                size >= n
            } else {
                size <= n
            }
        }
        Rule::Regex(pattern) => php_string(value).is_some_and(|s| compiled(pattern).is_match(&s)),
        Rule::Boolean => match value {
            Value::Bool(_) => true,
            Value::Number(n) => n.as_i64().is_some_and(|i| i == 0 || i == 1),
            Value::String(s) => s == "0" || s == "1",
            _ => false,
        },
    }
}

// =====================================================================================================
// Model mutation audit (the `Auditable` trait): `api.<resource>.<action>` with encrypted changes
// =====================================================================================================

/// `Auditable::isExcludedAuditField` (key, uuid, timestamps, hidden, `*_id`, `*_at`, sensitive).
fn audit_excluded(field: &str, hidden: &[&str]) -> bool {
    const ALWAYS: &[&str] = &[
        "id",
        "uuid",
        "created_at",
        "updated_at",
        "deleted_at",
        "order",
        "status",
    ];
    ALWAYS.contains(&field)
        || hidden.contains(&field)
        || field.ends_with("_id")
        || field.ends_with("_at")
        || audit::redact(json!({ field: 0 }), None)
            .get(field)
            .is_some_and(|v| v == "[REDACTED]")
}

/// `Auditable::AUDIT_IGNORED_FIELDS` that occur on these models (for `changed_fields`).
const AUDIT_IGNORED_FIELDS: &[&str] = &["updated_at", "order", "status"];

/// The `changes` map: `created` (new values), `deleted` (old values) or `updated` (both).
/// `hidden` is the model's `$hidden` plus its `$auditExclude`.
pub(crate) fn audit_changes(
    old: Option<&Map<String, Value>>,
    new: Option<&Map<String, Value>>,
    fields: &[String],
    hidden: &[&str],
) -> Map<String, Value> {
    let pick = |m: Option<&Map<String, Value>>, f: &str| {
        m.and_then(|m| m.get(f))
            .cloned()
            .map(|v| audit::redact(v, None))
            .unwrap_or(Value::Null)
    };
    fields
        .iter()
        .filter(|f| !audit_excluded(f, hidden))
        .map(|f| (f.clone(), json!({"old": pick(old, f), "new": pick(new, f)})))
        .collect()
}

/// Changed attribute names for an update (`getChanges()` minus ignored fields and `$auditExclude`),
/// in table column order like Eloquent's attribute array.
pub(crate) fn changed_fields(
    old: &Map<String, Value>,
    new: &Map<String, Value>,
    audit_exclude: &[&str],
    column_order: &[&str],
) -> Vec<String> {
    let mut fields: Vec<String> = new
        .iter()
        .filter(|(k, v)| old.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .filter(|k| {
            !AUDIT_IGNORED_FIELDS.contains(&k.as_str()) && !audit_exclude.contains(&k.as_str())
        })
        .collect();
    fields.sort_by_key(|f| {
        column_order
            .iter()
            .position(|c| c == f)
            .unwrap_or(usize::MAX)
    });
    fields
}

/// Write the `Auditable` row: event `api.<resource_type>.<action>`, metadata with
/// `<type>_uuid` / `<type>_name` / `changed_fields`, and `changes` as `encrypted:array`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_mutation(
    state: &crate::AppState,
    pool: &PgPool,
    ctx: &Ctx,
    team_id: i64,
    resource_type: &str,
    action: &str,
    uuid: Option<&str>,
    name: Option<&str>,
    changed: Vec<String>,
    changes: Map<String, Value>,
) {
    let event = format!("api.{resource_type}.{action}");
    let metadata = audit::redact(
        json!({
            "team_id": team_id,
            format!("{resource_type}_uuid"): uuid,
            format!("{resource_type}_name"): name,
            "changed_fields": changed,
        }),
        None,
    );
    let subject = name
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| audit::headline(resource_type));
    let description = format!("{subject} {}", audit::headline(action));
    let encrypted = state
        .app_key
        .as_ref()
        .and_then(|key| crate::crypto::encrypt_string(&Value::Object(changes).to_string(), key));
    let result = sqlx::query(&format!(
        "INSERT INTO audit_events (team_id, event, source, action, level, actor_type, actor_id, actor_name, \
         actor_email, actor_token_id, actor_token_name, resource_type, resource_uuid, resource_name, description, \
         metadata, ip_address, user_agent, changes, created_at) \
         VALUES ($1,$2,'api',$3,'info','api_token',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::json,$14,$15,$16,{NOW})"
    ))
    .bind(team_id)
    .bind(&event)
    .bind(action)
    .bind(ctx.user_id)
    .bind(&ctx.user_name)
    .bind(&ctx.user_email)
    .bind(ctx.token_id)
    .bind(&ctx.token_name)
    .bind(resource_type)
    .bind(uuid)
    .bind(name)
    .bind(description.trim())
    .bind(metadata.to_string())
    .bind(&ctx.ip)
    .bind(&ctx.user_agent)
    .bind(encrypted)
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::warn!("audit mutation persistence failed for {event}: {e}");
    }
}

pub(crate) fn as_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

pub(crate) fn fields_of(m: &Map<String, Value>) -> Vec<String> {
    m.keys().cloned().collect()
}

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
