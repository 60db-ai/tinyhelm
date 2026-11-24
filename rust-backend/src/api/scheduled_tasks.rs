//! Native `/api/v1` handlers: scheduled tasks of applications (Laravel `ScheduledTasksController`).
//!
//! ponytail: the `/services/{uuid}/scheduled-tasks` routes stay in Laravel: their JSON embeds the
//! lazy-loaded `service` model, which needs the services serializer exposed like
//! `embedded_application`. `.../execute` runs commands in containers and stays in Laravel too.

#[allow(unused_imports)]
use super::prelude::*;

use axum::{body::Bytes, http::HeaderMap};
use serde_json::{Map, Value};

/// `ScheduledTask` casts (`failing_since` datetime) and the controller's `makeHidden`.
const TASK_SPEC: ModelSpec = ModelSpec {
    hidden: &["id", "team_id", "application_id", "service_id"],
    datetimes: &["failing_since"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// `ScheduledTaskExecution` casts and the controller's `makeHidden`.
const EXECUTION_SPEC: ModelSpec = ModelSpec {
    hidden: &["id", "scheduled_task_id"],
    datetimes: &["started_at", "finished_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

const ALLOWED_FIELDS: &[&str] = &[
    "name",
    "command",
    "frequency",
    "container",
    "timeout",
    "use_shell",
    "enabled",
];

const VALID_CRON_STRINGS: &[&str] = &[
    "every_minute",
    "hourly",
    "daily",
    "weekly",
    "monthly",
    "yearly",
    "@hourly",
    "@daily",
    "@weekly",
    "@monthly",
    "@yearly",
];

/// `Application::ownedByCurrentTeamAPI($teamId)->where('uuid', ...)`: the application id.
async fn application_id(
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

fn app_not_found() -> Response {
    not_found("Application not found.")
}

fn task_not_found() -> Response {
    not_found("Scheduled task not found.")
}

/// `removeSensitiveData()`: hide keys, hide `command` without secret access, and add the lazy-loaded
/// `application` that `canReadResourceSecretsFromApi($task->application)` puts in the JSON.
async fn task_json(
    state: &crate::AppState,
    pool: &PgPool,
    ctx: &Ctx,
    row: Value,
    app_id: i64,
) -> Result<Value, sqlx::Error> {
    let mut task = match to_laravel(row, &TASK_SPEC) {
        Value::Object(m) => m,
        other => return Ok(other),
    };
    if !ctx.can_read_sensitive() {
        task.remove("command");
    }
    let app = embedded_application(
        pool,
        app_id,
        state.app_key.as_ref(),
        !ctx.can_read_sensitive(),
    )
    .await?;
    task.insert("application".into(), app.unwrap_or(Value::Null));
    Ok(Value::Object(task))
}

async fn task_row(
    pool: &PgPool,
    app_id: i64,
    task_uuid: &str,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM scheduled_tasks t WHERE t.application_id = $1 AND t.uuid = $2 LIMIT 1",
    )
    .bind(app_id)
    .bind(task_uuid)
    .fetch_optional(pool)
    .await
}

/// GET /api/v1/applications/{uuid}/scheduled-tasks
pub async fn list_app_scheduled_tasks(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let app_id = match application_id(pool, team_id, &uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return app_not_found(),
        Err(e) => return query_err("list_app_scheduled_tasks", e),
    };
    let rows: Vec<Value> = match sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM scheduled_tasks t WHERE t.application_id = $1 ORDER BY t.name ASC",
    )
    .bind(app_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_app_scheduled_tasks", e),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match task_json(&state, pool, &ctx, row, app_id).await {
            Ok(v) => out.push(v),
            Err(e) => return query_err("list_app_scheduled_tasks", e),
        }
    }
    ok_json(out)
}

/// GET /api/v1/applications/{uuid}/scheduled-tasks/{task_uuid}/executions
pub async fn list_app_task_executions(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, task_uuid)): Path<(String, String)>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let app_id = match application_id(pool, team_id, &uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return app_not_found(),
        Err(e) => return query_err("list_app_task_executions", e),
    };
    let task_id: Option<i64> = match sqlx::query_scalar(
        "SELECT id FROM scheduled_tasks WHERE application_id = $1 AND uuid = $2 LIMIT 1",
    )
    .bind(app_id)
    .bind(&task_uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => return query_err("list_app_task_executions", e),
    };
    let Some(task_id) = task_id else {
        return task_not_found();
    };
    let rows: Vec<Value> = match sqlx::query_scalar(
        "SELECT to_jsonb(x) FROM scheduled_task_executions x WHERE x.scheduled_task_id = $1 \
         ORDER BY x.created_at DESC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_app_task_executions", e),
    };
    let sensitive = ctx.can_read_sensitive();
    let out: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let mut v = to_laravel(row, &EXECUTION_SPEC);
            if let Value::Object(m) = &mut v {
                if !sensitive {
                    m.remove("message");
                }
                // `duration` has a `float` cast.
                if let Some(d) = m.get("duration").and_then(Value::as_f64) {
                    m.insert("duration".into(), json!(d));
                }
            }
            v
        })
        .collect();
    ok_json(out)
}

/// POST /api/v1/applications/{uuid}/scheduled-tasks
pub async fn create_app_scheduled_task(
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
    let app_id = match application_id(pool, team_id, &uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return app_not_found(),
        Err(e) => return query_err("create_app_scheduled_task", e),
    };
    let input = match json_input(&headers, &body) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if let Some(r) = validate_task(&input, true) {
        return r;
    }
    let frequency = input_str(&input, "frequency").unwrap_or_default();
    if !validate_cron_expression(&frequency) {
        return invalid_cron();
    }
    let enabled = if input.contains_key("enabled") {
        input_bool(&input, "enabled")
    } else {
        true
    };
    if let Some(r) = enabled_task_forbidden(&ctx, enabled) {
        return r;
    }

    let name = strip_tags(&input_str(&input, "name").unwrap_or_default());
    let command = input_str(&input, "command").unwrap_or_default();
    let container = input_str(&input, "container");
    let timeout = if input.contains_key("timeout") {
        php_int(input.get("timeout"))
    } else {
        300
    };
    let use_shell = if input.contains_key("use_shell") {
        input_bool(&input, "use_shell")
    } else {
        true
    };
    let task_uuid = new_public_id();

    let row: Value = match sqlx::query_scalar(&format!(
        "INSERT INTO scheduled_tasks (uuid, name, command, frequency, container, timeout, use_shell, enabled, \
         team_id, application_id, consecutive_failures, failing_since, disabled_reason, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,0,NULL,NULL,{NOW},{NOW}) RETURNING to_jsonb(scheduled_tasks)"
    ))
    .bind(&task_uuid)
    .bind(&name)
    .bind(&command)
    .bind(&frequency)
    .bind(&container)
    .bind(timeout)
    .bind(use_shell)
    .bind(enabled)
    .bind(team_id)
    .bind(app_id)
    .fetch_one(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("create_app_scheduled_task", e),
    };

    audit::record(
        pool,
        &ctx,
        "api.scheduled_task.created",
        json!({
            "team_id": team_id,
            "task_uuid": task_uuid,
            "task_name": name,
            "resource_type": "application",
            "resource_uuid": uuid,
        }),
        "info",
    )
    .await;

    // The saved model holds only the attributes Laravel set: the DB defaults of the failure columns are
    // not loaded, except that the saving hook sets them when the task is enabled.
    let mut row = row;
    if let Value::Object(m) = &mut row {
        if !enabled {
            for k in ["consecutive_failures", "failing_since", "disabled_reason"] {
                m.remove(k);
            }
        }
    }
    match task_json(&state, pool, &ctx, row, app_id).await {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) => query_err("create_app_scheduled_task", e),
    }
}

/// PATCH /api/v1/applications/{uuid}/scheduled-tasks/{task_uuid}
pub async fn update_app_scheduled_task(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, task_uuid)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let app_id = match application_id(pool, team_id, &uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return app_not_found(),
        Err(e) => return query_err("update_app_scheduled_task", e),
    };
    let input = match json_input(&headers, &body) {
        Ok(i) => i,
        Err(r) => return r,
    };
    if input.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "At least one field must be provided."})),
        )
            .into_response();
    }
    if let Some(r) = validate_task(&input, false) {
        return r;
    }
    if input.contains_key("frequency")
        && !validate_cron_expression(&input_str(&input, "frequency").unwrap_or_default())
    {
        return invalid_cron();
    }
    let current = match task_row(pool, app_id, &task_uuid).await {
        Ok(Some(r)) => r,
        Ok(None) => return task_not_found(),
        Err(e) => return query_err("update_app_scheduled_task", e),
    };
    let original = match &current {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };
    let enabled = if input.contains_key("enabled") {
        input_bool(&input, "enabled")
    } else {
        original
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    if let Some(r) = enabled_task_forbidden(&ctx, enabled) {
        return r;
    }

    // `$task->fill($request->only($allowedFields))` + `auditChangedFields()` (loose compare after casts).
    let mut changed: Vec<&str> = Vec::new();
    let mut updates: Map<String, Value> = Map::new();
    for field in ALLOWED_FIELDS {
        if !input.contains_key(*field) {
            continue;
        }
        let new = cast_field(field, &input);
        if !loosely_equal(original.get(*field).unwrap_or(&Value::Null), &new) {
            changed.push(field);
        }
        updates.insert(field.to_string(), new);
    }

    let row = if changed.is_empty() {
        current
    } else {
        let reset_failures = changed.contains(&"enabled") && enabled;
        match sqlx::query_scalar::<_, Value>(&format!(
            "UPDATE scheduled_tasks SET \
               name = CASE WHEN $2 THEN $3 ELSE name END, \
               command = CASE WHEN $4 THEN $5 ELSE command END, \
               frequency = CASE WHEN $6 THEN $7 ELSE frequency END, \
               container = CASE WHEN $8 THEN $9 ELSE container END, \
               timeout = CASE WHEN $10 THEN $11 ELSE timeout END, \
               use_shell = CASE WHEN $12 THEN $13 ELSE use_shell END, \
               enabled = CASE WHEN $14 THEN $15 ELSE enabled END, \
               consecutive_failures = CASE WHEN $16 THEN 0 ELSE consecutive_failures END, \
               failing_since = CASE WHEN $16 THEN NULL ELSE failing_since END, \
               disabled_reason = CASE WHEN $16 THEN NULL ELSE disabled_reason END, \
               updated_at = {NOW} \
             WHERE application_id = $1 AND uuid = $17 RETURNING to_jsonb(scheduled_tasks)"
        ))
        .bind(app_id)
        .bind(updates.contains_key("name"))
        .bind(updates.get("name").and_then(Value::as_str))
        .bind(updates.contains_key("command"))
        .bind(updates.get("command").and_then(Value::as_str))
        .bind(updates.contains_key("frequency"))
        .bind(updates.get("frequency").and_then(Value::as_str))
        .bind(updates.contains_key("container"))
        .bind(updates.get("container").and_then(Value::as_str))
        .bind(updates.contains_key("timeout"))
        .bind(updates.get("timeout").and_then(Value::as_i64))
        .bind(updates.contains_key("use_shell"))
        .bind(updates.get("use_shell").and_then(Value::as_bool))
        .bind(updates.contains_key("enabled"))
        .bind(updates.get("enabled").and_then(Value::as_bool))
        .bind(reset_failures)
        .bind(&task_uuid)
        .fetch_one(pool)
        .await
        {
            Ok(r) => r,
            Err(e) => return query_err("update_app_scheduled_task", e),
        }
    };

    if !changed.is_empty() {
        let task_name = row.get("name").cloned().unwrap_or(Value::Null);
        audit::record(
            pool,
            &ctx,
            "api.scheduled_task.updated",
            json!({
                "team_id": team_id,
                "task_uuid": task_uuid,
                "task_name": task_name,
                "resource_type": "application",
                "resource_uuid": uuid,
                "changed_fields": changed,
            }),
            "info",
        )
        .await;
    }
    match task_json(&state, pool, &ctx, row, app_id).await {
        Ok(v) => ok_json(v),
        Err(e) => query_err("update_app_scheduled_task", e),
    }
}

/// DELETE /api/v1/applications/{uuid}/scheduled-tasks/{task_uuid}
pub async fn delete_app_scheduled_task(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path((uuid, task_uuid)): Path<(String, String)>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let app_id = match application_id(pool, team_id, &uuid).await {
        Ok(Some(id)) => id,
        Ok(None) => return app_not_found(),
        Err(e) => return query_err("delete_app_scheduled_task", e),
    };
    let name: Option<String> = match sqlx::query_scalar(
        "DELETE FROM scheduled_tasks WHERE application_id = $1 AND uuid = $2 RETURNING name",
    )
    .bind(app_id)
    .bind(&task_uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => return query_err("delete_app_scheduled_task", e),
    };
    let Some(name) = name else {
        return task_not_found();
    };
    audit::record(
        pool,
        &ctx,
        "api.scheduled_task.deleted",
        json!({
            "team_id": team_id,
            "task_uuid": task_uuid,
            "task_name": name,
            "resource_type": "application",
            "resource_uuid": uuid,
        }),
        "info",
    )
    .await;
    ok_json(json!({"message": "Scheduled task deleted."}))
}

/// `customApiValidator` rules of create (`required` on name/command/frequency) and update.
fn validate_task(input: &Map<String, Value>, creating: bool) -> Option<Response> {
    let mut v = Validator::new(input, CUSTOM_API_MESSAGES);
    let with_required = |rules: &[Rule]| -> Vec<Rule> {
        if creating {
            std::iter::once(Rule::Required)
                .chain(rules.iter().copied())
                .collect()
        } else {
            rules.to_vec()
        }
    };
    v.field("name", &with_required(&[Rule::Str, Rule::Max(255)]));
    v.field("command", &with_required(&[Rule::Str]));
    v.field("frequency", &with_required(&[Rule::Str]));
    v.field("container", &[Rule::Str, Rule::Nullable]);
    for message in integer_rule_errors("timeout", input.get("timeout"), 1, 36000) {
        v.add("timeout", &message);
    }
    v.field("use_shell", &[Rule::Boolean]);
    v.field("enabled", &[Rule::Boolean]);
    v.reject_extra(ALLOWED_FIELDS);
    v.fails().then(|| v.response())
}

/// `integer|min:a|max:b` without `nullable`: an explicit null still runs the rules.
pub(crate) fn integer_rule_errors(
    field: &str,
    value: Option<&Value>,
    min: i64,
    max: i64,
) -> Vec<String> {
    let Some(value) = value else {
        return vec![];
    };
    if matches!(value, Value::String(s) if s.trim().is_empty()) {
        return vec![];
    }
    let label = field.replace('_', " ");
    let mut errors = Vec::new();
    if php_filter_int(value).is_none() {
        errors.push(format!("The {label} field must be an integer."));
    }
    // getSize(): the number when numeric (the attribute has a numeric rule), else the string length.
    let size: f64 = match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) if php_is_numeric(s) => s.trim().parse().unwrap_or(0.0),
        Value::String(s) => s.chars().count() as f64,
        Value::Bool(true) => 1.0,
        Value::Array(a) => a.len() as f64,
        _ => 0.0,
    };
    if size < min as f64 {
        errors.push(format!("The {label} field must be at least {min}."));
    }
    if size > max as f64 {
        errors.push(format!("The {label} field must not be greater than {max}."));
    }
    errors
}

/// PHP `filter_var($v, FILTER_VALIDATE_INT)`.
fn php_filter_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)),
        Value::Bool(true) => Some(1),
        Value::String(s) => {
            let t = s.trim();
            let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
            let ok = !digits.is_empty()
                && digits.chars().all(|c| c.is_ascii_digit())
                && (digits == "0" || !digits.starts_with('0'));
            ok.then(|| t.parse().ok()).flatten()
        }
        _ => None,
    }
}

/// PHP `is_numeric()` for strings.
fn php_is_numeric(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.parse::<f64>().is_ok() && !t.contains(['x', 'X', 'n', 'N', 'i', 'I'])
}

/// The cast value an allowed field takes after `fill()` (the `name` setter strips tags).
fn cast_field(field: &str, input: &Map<String, Value>) -> Value {
    match field {
        "name" => json!(strip_tags(&input_str(input, "name").unwrap_or_default())),
        "timeout" => json!(php_int(input.get("timeout"))),
        "use_shell" | "enabled" => json!(input_bool(input, field)),
        _ => match input_str(input, field) {
            Some(s) => json!(s),
            None => Value::Null,
        },
    }
}

/// PHP `(int)` cast.
fn php_int(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n
            .as_i64()
            .unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64),
        Some(Value::String(s)) => s.trim().parse::<f64>().map(|f| f as i64).unwrap_or(0),
        Some(Value::Bool(b)) => i64::from(*b),
        _ => 0,
    }
}

/// PHP `==` between the stored (cast) value and the new cast value.
fn loosely_equal(old: &Value, new: &Value) -> bool {
    match (old, new) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Null, Value::String(s)) | (Value::String(s), Value::Null) => s.is_empty(),
        (a, b) => a == b,
    }
}

fn invalid_cron() -> Response {
    validation_failed(json!({"frequency": ["Invalid cron expression or frequency format."]}))
}

/// `RequiresDeployAbility::deployAbilityForbiddenResponse()` for enabled tasks.
fn enabled_task_forbidden(ctx: &Ctx, enabled: bool) -> Option<Response> {
    if !enabled || ctx.can("deploy") || ctx.can("root") {
        return None;
    }
    Some(
        (
            StatusCode::FORBIDDEN,
            Json(json!({"message": "Missing required permissions: deploy. An enabled scheduled task runs commands in the resource containers, so it needs a token with the deploy permission."})),
        )
            .into_response(),
    )
}

/// `validate_cron_expression()`: a `VALID_CRON_STRINGS` key or a valid dragonmantank expression.
fn validate_cron_expression(expression: &str) -> bool {
    // PHP empty(): "" and "0" are empty.
    if expression.is_empty() || expression == "0" {
        return false;
    }
    VALID_CRON_STRINGS.contains(&expression) || cron::is_valid(expression)
}

/// Port of `Cron\CronExpression::isValid()` (dragonmantank/cron-expression 3.x field validators).
mod cron {
    const MAPPINGS: &[(&str, &str)] = &[
        ("@yearly", "0 0 1 1 *"),
        ("@annually", "0 0 1 1 *"),
        ("@monthly", "0 0 1 * *"),
        ("@weekly", "0 0 * * 0"),
        ("@daily", "0 0 * * *"),
        ("@midnight", "0 0 * * *"),
        ("@hourly", "0 * * * *"),
    ];
    const MONTHS: &[&str] = &[
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    const DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

    pub fn is_valid(expression: &str) -> bool {
        let lower = expression.to_lowercase();
        let expanded = MAPPINGS
            .iter()
            .find(|(alias, _)| *alias == lower)
            .map_or(expression, |(_, cron)| *cron);
        let parts: Vec<&str> = expanded.split_whitespace().collect();
        if parts.len() < 5 {
            return false;
        }
        (0..5).all(|position| validate_part(position, parts[position]))
    }

    fn validate_part(position: usize, value: &str) -> bool {
        match position {
            0 => standard(value, 0, 59),
            1 => standard(value, 0, 23),
            2 => day_of_month(value),
            3 => standard(&translate(value, MONTHS, 1), 1, 12),
            _ => day_of_week(value),
        }
    }

    /// JAN..DEC / MON..SUN to numbers, case-insensitively.
    fn translate(value: &str, names: &[&str], first: usize) -> String {
        let mut out = value.to_uppercase();
        for (i, name) in names.iter().enumerate() {
            out = out.replace(name, &(i + first).to_string());
        }
        out
    }

    /// `AbstractField::validate()`: `*`, lists, ranges, steps, plain numbers within bounds.
    fn standard(value: &str, min: i64, max: i64) -> bool {
        if value == "*" {
            return true;
        }
        if value.contains(',') {
            return value
                .split(',')
                .all(|v| !v.is_empty() && standard(v, min, max));
        }
        if let Some((range, step)) = value.split_once('/') {
            if range.is_empty() || step.contains('/') {
                return false;
            }
            if !(range == "*" || standard(range, min, max)) {
                return false;
            }
            return number(step).is_some_and(|s| s > 0);
        }
        if let Some((a, b)) = value.split_once('-') {
            if b.contains('-') {
                return false;
            }
            let (Some(a), Some(b)) = (number(a), number(b)) else {
                return false;
            };
            return (min..=max).contains(&a) && (min..=max).contains(&b);
        }
        number(value).is_some_and(|n| (min..=max).contains(&n))
    }

    fn number(value: &str) -> Option<i64> {
        (!value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
            .then(|| value.parse().ok())
            .flatten()
    }

    /// `DayOfMonthField::validate()`: also `?`, `L`, and `<n>W`.
    fn day_of_month(value: &str) -> bool {
        if value == "?" || value == "L" {
            return true;
        }
        if let Some(day) = value.strip_suffix('W') {
            return number(day).is_some_and(|d| (1..=31).contains(&d));
        }
        standard(value, 1, 31)
    }

    /// `DayOfWeekField::validate()`: names, `?`, `<n>L`, `<n>#<k>`, 0-7.
    fn day_of_week(value: &str) -> bool {
        let value = translate(value, DAYS, 1);
        if value == "?" {
            return true;
        }
        if let Some(day) = value.strip_suffix('L') {
            return number(day).is_some_and(|d| (0..=7).contains(&d));
        }
        if let Some((day, nth)) = value.split_once('#') {
            return number(day).is_some_and(|d| (0..=7).contains(&d))
                && number(nth).is_some_and(|n| (1..=5).contains(&n));
        }
        standard(&value, 0, 7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_validation_matches_laravel_cases() {
        for ok in [
            "every_minute",
            "@daily",
            "*/5 * * * *",
            "0 0 1 1 *",
            "0 9 * * MON-FRI",
            "15 2 L * *",
            "0 0 * * 5#2",
            "0 0 * JAN *",
        ] {
            assert!(validate_cron_expression(ok), "{ok} should be valid");
        }
        for bad in [
            "nonsense",
            "61 * * * *",
            "* * * *",
            "",
            "*/0 * * * *",
            "0 24 * * *",
        ] {
            assert!(!validate_cron_expression(bad), "{bad} should be invalid");
        }
    }

    #[test]
    fn timeout_rules() {
        let f = |v: Value| integer_rule_errors("timeout", Some(&v), 1, 36000);
        assert!(f(json!(60)).is_empty());
        assert_eq!(
            f(json!("abc")),
            vec!["The timeout field must be an integer."]
        );
        assert_eq!(f(json!(0)), vec!["The timeout field must be at least 1."]);
        assert_eq!(
            f(json!(40000)),
            vec!["The timeout field must not be greater than 36000."]
        );
        assert_eq!(
            f(Value::Null),
            vec![
                "The timeout field must be an integer.",
                "The timeout field must be at least 1."
            ]
        );
    }
}
