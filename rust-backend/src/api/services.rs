//! Native `/api/v1` handlers: services (`ServicesController::services` / `service_by_uuid`).
//!
//! Output matches Laravel's serialization of `Service`: `$hidden` compose attributes, casts, the
//! `server_status` and `status` appends, and the relations those accessors load and Laravel then
//! serializes (`server` with settings, `applications`, `databases`), plus `destination` for tokens that
//! can read server secrets. Cases that depend on PHP-only state are answered by Laravel.

#[allow(unused_imports)]
use super::prelude::*;
use axum::extract::Request;
use serde_json::{Map, Value};

const SERVICE_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &["deleted_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};
const SERVICE_APPLICATION_SPEC: ModelSpec = ModelSpec {
    hidden: &["domain_dns_statuses", "domain_port_overrides"],
    datetimes: &["deleted_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};
const SERVICE_DATABASE_SPEC: ModelSpec = ModelSpec {
    hidden: &[],
    datetimes: &["deleted_at"],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};
const COMPOSE_ATTRIBUTES: &[&str] = &["docker_compose_raw", "docker_compose"];

/// `ContainerStatusAggregator::aggregateFromStrings` with the default flags (no restart count,
/// restarting counts as degraded).
pub(crate) fn aggregate_statuses(statuses: &[String]) -> String {
    if statuses.is_empty() {
        return "exited".into();
    }
    let (mut running, mut restarting, mut unhealthy, mut unknown, mut health_starting) =
        (false, false, false, false, false);
    let (mut exited, mut starting, mut paused, mut dead, mut degraded) =
        (false, false, false, false, false);
    for s in statuses {
        if s.contains("degraded") {
            degraded = true;
            unhealthy |= s.contains("unhealthy");
        } else if s.contains("restarting") {
            restarting = true;
        } else if s.contains("running") {
            running = true;
            unhealthy |= s.contains("unhealthy");
            unknown |= s.contains("unknown");
            health_starting |= s.contains("starting");
        } else if s.contains("exited") {
            exited = true;
        } else if s.contains("created") || s.contains("starting") {
            starting = true;
        } else if s.contains("paused") {
            paused = true;
        } else if s.contains("dead") || s.contains("removing") {
            dead = true;
        }
    }
    let out = if degraded || restarting || (running && exited) {
        "degraded:unhealthy"
    } else if running && starting {
        "starting:unknown"
    } else if running {
        if unhealthy {
            "running:unhealthy"
        } else if health_starting {
            "running:starting"
        } else if unknown {
            "running:unknown"
        } else {
            "running:healthy"
        }
    } else if dead {
        "degraded:unhealthy"
    } else if paused {
        "paused:unknown"
    } else if starting {
        "starting:unknown"
    } else {
        "exited"
    };
    out.into()
}

/// `Service::aggregateResourceStatuses`: (status, health, has_non_excluded) over the sub-resources.
fn aggregate_resources(
    resources: &[&Map<String, Value>],
    excluded_only: bool,
) -> (Option<String>, Option<String>, bool) {
    let mut has_non_excluded = false;
    let mut strings = Vec::new();
    for r in resources {
        if r.get("is_removed_from_compose") == Some(&Value::Bool(true)) {
            continue;
        }
        let status = r.get("status").and_then(Value::as_str).unwrap_or("");
        let excluded = r.get("exclude_from_status") == Some(&Value::Bool(true))
            || status.contains(":excluded");
        if excluded != excluded_only {
            continue;
        }
        if !excluded_only {
            has_non_excluded = true;
        }
        strings.push(status.split(":excluded").next().unwrap_or("").to_string());
    }
    if strings.is_empty() {
        return (None, None, has_non_excluded);
    }
    let aggregated = aggregate_statuses(&strings);
    let mut parts = aggregated.split(':');
    let status = parts.next().map(str::to_string);
    let health = parts.next().map(str::to_string);
    (status, health, has_non_excluded)
}

/// `Service::containerStatus`.
pub(crate) fn container_status(resources: &[&Map<String, Value>]) -> Option<String> {
    let (status, health, has_non_excluded) = aggregate_resources(resources, false);
    if !has_non_excluded && status.is_none() && health.is_none() {
        let (ex_status, ex_health, _) = aggregate_resources(resources, true);
        return Some(match (ex_status, ex_health) {
            (Some(s), Some(h)) => format!("{s}:{h}:excluded"),
            (None, None) => "unknown:unknown:excluded".into(),
            _ => "exited".into(),
        });
    }
    match (status, health) {
        (s, None) => s,
        (s, Some(h)) if h.is_empty() => s,
        (Some(s), Some(h)) => Some(format!("{s}:{h}")),
        (None, Some(h)) => Some(format!(":{h}")),
    }
}

/// `ResourceStartActivity::latestRunning($uuid) !== null`: the latest activity for the resource is
/// queued or in progress and not stale.
async fn is_starting(pool: &PgPool, uuid: &str) -> Result<bool, sqlx::Error> {
    let row: Option<(Option<String>, Option<String>, Option<bool>)> = sqlx::query_as(
        "SELECT properties::jsonb->>'status', properties::jsonb->>'operation', \
                COALESCE(updated_at, created_at) > (now() AT TIME ZONE 'UTC') - make_interval(secs => \
                  CASE WHEN properties::jsonb->>'status' = 'queued' THEN 600 ELSE 900 END) \
         FROM activity_log WHERE properties::jsonb->>'type_uuid' = $1 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(uuid)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some((Some(status), operation, fresh)) => {
            let active = status == "queued" || status == "in_progress";
            // ponytail: imports use a config-driven stale window; treat an active import as running.
            active && (fresh == Some(true) || operation.as_deref() == Some("database_import"))
        }
        _ => false,
    })
}

/// `ServiceApplication::url`. Port overrides need DomainUrlParts, so those rows are answered by Laravel.
fn service_application_url(m: &Map<String, Value>) -> Result<Value, Fail> {
    let fqdn = m.get("fqdn").and_then(Value::as_str).unwrap_or("");
    if fqdn.trim().is_empty() {
        return Ok(Value::Null);
    }
    let has_overrides = match m.get("domain_port_overrides") {
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        _ => false,
    };
    if has_overrides {
        // ponytail: port overrides rewrite URLs with DomainUrlParts; port with the domain helpers.
        return Err(Fail::Delegate);
    }
    Ok(Value::String(
        fqdn.split(',').map(str::trim).collect::<Vec<_>>().join(","),
    ))
}

/// True when `redactLockedValuesInGeneratedCompose` might change the compose: a shown-once env var,
/// or a shared-variable reference in the compose or in an env var value.
async fn compose_may_hold_locked_values(
    pool: &PgPool,
    service_id: i64,
    m: &Map<String, Value>,
) -> Result<bool, sqlx::Error> {
    let raw = m
        .get("docker_compose_raw")
        .and_then(Value::as_str)
        .unwrap_or("");
    if raw.contains("{{") {
        return Ok(true);
    }
    sqlx::query_scalar(
        r"SELECT EXISTS (SELECT 1 FROM environment_variables WHERE resourceable_type = 'App\Models\Service'
          AND resourceable_id = $1 AND (is_shown_once OR value LIKE '%{{%'))",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
}

/// Serialize one service like `removeSensitiveData` + `serializeApiResponse`.
async fn service_json(
    pool: &PgPool,
    row: Value,
    ctx: &Ctx,
    app_key: Option<&[u8; 32]>,
    through_key: Option<i64>,
) -> Result<Value, Fail> {
    let sensitive = ctx.can_read_sensitive();
    let Value::Object(mut m) = to_laravel(row, &SERVICE_SPEC) else {
        return Err(Fail::Delegate);
    };
    let service_id = m.get("id").and_then(Value::as_i64).unwrap_or(0);
    let uuid = m
        .get("uuid")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    if sensitive {
        let compose_filled = m
            .get("docker_compose")
            .and_then(Value::as_str)
            .is_some_and(|c| !c.trim().is_empty());
        if compose_filled && compose_may_hold_locked_values(pool, service_id, &m).await? {
            // ponytail: locked values in the generated compose are redacted by Laravel's helper.
            return Err(Fail::Delegate);
        }
    } else {
        for k in COMPOSE_ATTRIBUTES {
            m.remove(*k);
        }
    }
    for k in ["id", "resourceable", "resourceable_id", "resourceable_type"] {
        m.remove(k);
    }
    if let Some(key) = through_key {
        m.insert("laravel_through_key".into(), Value::from(key));
    }

    // Sub-resources (loaded by the `status` accessor).
    let applications: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM service_applications a WHERE a.service_id = $1 AND a.deleted_at IS NULL ORDER BY a.id")
            .bind(service_id)
            .fetch_all(pool)
            .await?;
    let databases: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(d) FROM service_databases d WHERE d.service_id = $1 AND d.deleted_at IS NULL ORDER BY d.id")
            .bind(service_id)
            .fetch_all(pool)
            .await?;

    let resources: Vec<&Map<String, Value>> = applications
        .iter()
        .chain(databases.iter())
        .filter_map(Value::as_object)
        .collect();
    let status = if is_starting(pool, &uuid).await? {
        Some("starting:unhealthy".to_string())
    } else {
        container_status(&resources)
    };

    let mut app_json = Vec::with_capacity(applications.len());
    for a in &applications {
        let url = service_application_url(a.as_object().ok_or(Fail::Delegate)?)?;
        let Value::Object(mut am) = to_laravel(a.clone(), &SERVICE_APPLICATION_SPEC) else {
            return Err(Fail::Delegate);
        };
        am.insert("url".into(), url);
        app_json.push(Value::Object(am));
    }
    let db_json: Vec<Value> = databases
        .into_iter()
        .map(|d| to_laravel(d, &SERVICE_DATABASE_SPEC))
        .collect();

    // `destination.server.settings` is eager-loaded for server-secret readers; secrets are exposed on the
    // destination's server, or on the service's own server when there is no destination server.
    let destination = if sensitive {
        Some(destination_json(pool, &m, true, app_key).await?.0)
    } else {
        None
    };
    let dest_has_server = destination
        .as_ref()
        .is_some_and(|d| d.get("server").is_some_and(|s| !s.is_null()));
    let expose_own_server = sensitive && !dest_has_server;

    let (server, functional) = match m.get("server_id").and_then(Value::as_i64) {
        Some(sid) => server_json(pool, sid, expose_own_server, app_key)
            .await?
            .unwrap_or((Value::Null, false)),
        None => (Value::Null, false),
    };

    m.insert("server_status".into(), Value::Bool(functional));
    m.insert("status".into(), status.map_or(Value::Null, Value::String));
    m.insert("server".into(), server);
    m.insert("applications".into(), Value::Array(app_json));
    m.insert("databases".into(), Value::Array(db_json));
    if let Some(dest) = destination {
        m.insert("destination".into(), dest);
    }

    let mut out = Value::Object(m);
    if !sensitive {
        remove_server_proxy_secrets(&mut out);
    }
    Ok(out)
}

/// GET /api/v1/services
pub async fn list_services(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match list_json(pool, &ctx, team_id, state.app_key.as_ref()).await {
        Ok(v) => ok_json(v),
        Err(fail) => fail_response(state.clone(), req, fail, "list_services").await,
    }
}

async fn list_json(
    pool: &PgPool,
    ctx: &Ctx,
    team_id: i64,
    app_key: Option<&[u8; 32]>,
) -> Result<Vec<Value>, Fail> {
    // `$project->services()` (hasManyThrough environments) per project; soft-deleted services excluded.
    let rows: Vec<(i64, Value)> = sqlx::query_as(
        "SELECT p.id, to_jsonb(s) FROM services s JOIN environments e ON e.id = s.environment_id \
         JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1 AND s.deleted_at IS NULL ORDER BY p.id, s.id",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (project_id, row) in rows {
        out.push(service_json(pool, row, ctx, app_key, Some(project_id)).await?);
    }
    Ok(out)
}

/// GET /api/v1/services/:uuid
pub async fn get_service(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let row: Result<Option<Value>, sqlx::Error> = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM services s JOIN environments e ON e.id = s.environment_id \
         JOIN projects p ON p.id = e.project_id WHERE p.team_id = $1 AND s.uuid = $2 AND s.deleted_at IS NULL LIMIT 1",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await;
    match row {
        Ok(Some(row)) => match service_json(pool, row, &ctx, state.app_key.as_ref(), None).await {
            Ok(v) => (StatusCode::OK, Json(v)).into_response(),
            Err(fail) => fail_response(state.clone(), req, fail, "get_service").await,
        },
        Ok(None) => not_found("Service not found."),
        Err(e) => query_err("get_service", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn statuses(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn aggregator_matches_laravel_priorities() {
        assert_eq!(aggregate_statuses(&[]), "exited");
        assert_eq!(
            aggregate_statuses(&statuses(&["running:healthy", "exited"])),
            "degraded:unhealthy"
        );
        assert_eq!(
            aggregate_statuses(&statuses(&["running:healthy", "running:unhealthy"])),
            "running:unhealthy"
        );
        assert_eq!(
            aggregate_statuses(&statuses(&["running:healthy", "starting"])),
            "starting:unknown"
        );
        assert_eq!(
            aggregate_statuses(&statuses(&["restarting"])),
            "degraded:unhealthy"
        );
        assert_eq!(aggregate_statuses(&statuses(&["paused"])), "paused:unknown");
    }

    #[test]
    fn container_status_rules() {
        assert_eq!(
            container_status(&[]).as_deref(),
            Some("unknown:unknown:excluded")
        );
        let excluded = json!({"status": "running:healthy", "exclude_from_status": true});
        assert_eq!(
            container_status(&[excluded.as_object().unwrap()]).as_deref(),
            Some("running:healthy:excluded")
        );
        let exited = json!({"status": "exited", "exclude_from_status": false});
        assert_eq!(
            container_status(&[exited.as_object().unwrap()]).as_deref(),
            Some("exited")
        );
    }
}
