//! Native `/api/v1` handler: audit events of the team (Laravel `AuditEventsController`), with Laravel's
//! `LengthAwarePaginator` JSON and the query helpers other paginated routes reuse.

#[allow(unused_imports)]
use super::prelude::*;

use axum::{
    extract::RawQuery,
    http::{header::HOST, HeaderMap},
};
use serde_json::{Map, Value};

const BASE_COLUMNS: &str =
    "id, team_id, event, source, action, level, actor_type, actor_id, actor_name, \
     resource_type, resource_uuid, resource_name, description, created_at";
const SENSITIVE_COLUMNS: &str =
    ", actor_email, actor_token_id, actor_token_name, metadata, changes, ip_address, user_agent";
const SOURCES: &[&str] = &["all", "ui", "api", "mcp", "webhook", "system", "scheduler"];

/// `$request->query()` after the global `TrimStrings` and `ConvertEmptyStringsToNull` middleware.
/// `key[]=v` pairs become arrays, like PHP's query parsing.
pub(crate) fn query_input(raw: Option<&str>) -> Map<String, Value> {
    let mut out = Map::new();
    for pair in raw.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(&k.replace('+', " "));
        let value = percent_decode(&v.replace('+', " "));
        let trimmed = value.trim();
        let value = if trimmed.is_empty() {
            Value::Null
        } else {
            Value::String(trimmed.to_string())
        };
        if let Some(base) = key.strip_suffix("[]") {
            let entry = out
                .entry(base.to_string())
                .or_insert_with(|| Value::Array(vec![]));
            if let Value::Array(list) = entry {
                list.push(value);
            }
        } else {
            out.insert(key, value);
        }
    }
    out
}

/// `Request::url()` (the paginator's `path`): scheme + Host header + path, without the query.
/// Laravel trusts no proxy headers here, so the scheme is the one of the request itself.
pub(crate) fn request_url(headers: &HeaderMap, path: &str) -> String {
    let host = headers
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    format!("http://{host}{path}")
}

/// `LengthAwarePaginator::toArray()` with `?page=N` URLs on `path` (onEachSide = 3).
pub(crate) fn paginator_json(
    path: &str,
    data: Vec<Value>,
    total: i64,
    per_page: i64,
    current: i64,
) -> Value {
    let last = ((total + per_page - 1) / per_page).max(1);
    let url = |page: i64| format!("{path}?page={}", page.max(1));
    let count = data.len() as i64;
    let from = (count > 0).then(|| (current - 1) * per_page + 1);
    let to = from.map(|f| f + count - 1);
    let prev = (current > 1).then(|| url(current - 1));
    let next = (current < last).then(|| url(current + 1));

    let page_link = |page: i64| json!({"url": url(page), "label": page.to_string(), "page": page, "active": page == current});
    let range = |a: i64, b: i64| (a..=b).map(page_link).collect::<Vec<_>>();
    let dots = json!({"url": null, "label": "...", "active": false});
    let on_each_side = 3;
    let mut links = vec![json!({
        "url": prev, "label": "&laquo; Previous", "page": (current > 1).then_some(current - 1), "active": false
    })];
    if last < on_each_side * 2 + 8 {
        links.extend(range(1, last));
    } else {
        let window = on_each_side + 4;
        if current <= window {
            links.extend(range(1, window + on_each_side));
            links.push(dots.clone());
            links.extend(range(last - 1, last));
        } else if current > last - window {
            links.extend(range(1, 2));
            links.push(dots.clone());
            links.extend(range(last - (window + (on_each_side - 1)), last));
        } else {
            links.extend(range(1, 2));
            links.push(dots.clone());
            links.extend(range(current - on_each_side, current + on_each_side));
            links.push(dots);
            links.extend(range(last - 1, last));
        }
    }
    links.push(json!({
        "url": next, "label": "Next &raquo;", "page": (current < last).then_some(current + 1), "active": false
    }));

    json!({
        "current_page": current,
        "data": data,
        "first_page_url": url(1),
        "from": from,
        "last_page": last,
        "last_page_url": url(last),
        "links": links,
        "next_page_url": next,
        "path": path,
        "per_page": per_page,
        "prev_page_url": prev,
        "to": to,
        "total": total,
    })
}

/// `nullable|string|max:n` on a query value.
fn string_max_errors(field: &str, value: Option<&Value>, max: usize) -> Vec<String> {
    match value {
        None | Some(Value::Null) => vec![],
        Some(Value::String(s)) if s.chars().count() > max => {
            vec![format!(
                "The {field} field must not be greater than {max} characters."
            )]
        }
        Some(Value::String(_)) => vec![],
        Some(Value::Array(items)) => {
            let mut errors = vec![format!("The {field} field must be a string.")];
            if items.len() > max {
                errors.push(format!(
                    "The {field} field must not have more than {max} items."
                ));
            }
            errors
        }
        Some(_) => vec![format!("The {field} field must be a string.")],
    }
}

/// GET /api/v1/audit-events
pub async fn list_audit_events(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    headers: HeaderMap,
    RawQuery(raw): RawQuery,
) -> Response {
    // `isAdminOfTeam` holds: the guard only serves admin and owner tokens natively.
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let input = query_input(raw.as_deref());

    let mut errors = Map::new();
    let mut add = |field: &str, messages: Vec<String>| {
        if !messages.is_empty() {
            errors.insert(field.into(), json!(messages));
        }
    };
    add(
        "per_page",
        integer_rule_errors("per_page", input.get("per_page"), 1, 100),
    );
    add(
        "page",
        integer_rule_errors("page", input.get("page"), 1, i64::MAX),
    );
    add(
        "search",
        string_max_errors("search", input.get("search"), 255),
    );
    add(
        "action",
        string_max_errors("action", input.get("action"), 255),
    );
    let mut source_errors = string_max_errors("source", input.get("source"), usize::MAX);
    match input.get("source") {
        Some(Value::String(s)) if !SOURCES.contains(&s.as_str()) => {
            source_errors.push("The selected source is invalid.".into());
        }
        Some(Value::Array(_)) => source_errors.push("The selected source is invalid.".into()),
        _ => {}
    }
    add("source", source_errors);
    if !errors.is_empty() {
        return validation_failed(Value::Object(errors));
    }

    let int_of = |key: &str, default: i64| match input.get(key) {
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(default),
        _ => default,
    };
    let per_page = int_of("per_page", 25);
    let page = int_of("page", 1).max(1);
    let text = |key: &str, default: &str| match input.get(key) {
        Some(Value::String(s)) => s.clone(),
        _ => default.to_string(),
    };
    let search = text("search", "");
    let action = text("action", "all");
    let source = text("source", "all");
    let sensitive = ctx.can_read_sensitive();

    // scopeVisibleToTeam + scopeFiltered (Laravel's `like` is a case-sensitive LIKE on PostgreSQL).
    let filter = format!(
        "team_id = $1 AND ($2 = 'all' OR action = $2) AND ($3 = 'all' OR source = $3) \
         AND ($4 = '' OR event LIKE $5 OR description LIKE $5 OR resource_name LIKE $5 OR actor_name LIKE $5{})",
        if sensitive {
            " OR actor_email LIKE $5"
        } else {
            ""
        }
    );
    let like = format!("%{search}%");
    let total: i64 =
        match sqlx::query_scalar(&format!("SELECT count(*) FROM audit_events WHERE {filter}"))
            .bind(team_id)
            .bind(&action)
            .bind(&source)
            .bind(&search)
            .bind(&like)
            .fetch_one(pool)
            .await
        {
            Ok(n) => n,
            Err(e) => return query_err("list_audit_events", e),
        };
    let columns = if sensitive {
        format!("{BASE_COLUMNS}{SENSITIVE_COLUMNS}")
    } else {
        BASE_COLUMNS.to_string()
    };
    let rows: Vec<Value> = match sqlx::query_scalar(&format!(
        "SELECT to_jsonb(x) FROM (SELECT {columns} FROM audit_events WHERE {filter} \
         ORDER BY created_at DESC, id DESC LIMIT $6 OFFSET $7) x"
    ))
    .bind(team_id)
    .bind(&action)
    .bind(&source)
    .bind(&search)
    .bind(&like)
    .bind(per_page)
    .bind((page - 1).saturating_mul(per_page))
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_audit_events", e),
    };

    let data: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let mut v = to_laravel(row, &ModelSpec::default());
            // `changes` has an `encrypted:array` cast.
            if let Some(c) = v.get_mut("changes") {
                if let Value::String(payload) = c {
                    let decrypted = state
                        .app_key
                        .as_ref()
                        .and_then(|key| crate::crypto::decrypt(payload, key))
                        .and_then(|plain| serde_json::from_str::<Value>(&plain).ok());
                    *c = decrypted.unwrap_or(Value::Null);
                }
            }
            v
        })
        .collect();

    let path = request_url(&headers, "/api/v1/audit-events");
    ok_json(paginator_json(&path, data, total, per_page, page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_paginator_links() {
        let p = paginator_json("http://h/x", vec![json!(1), json!(2)], 5, 2, 2);
        assert_eq!(p["from"], 3);
        assert_eq!(p["to"], 4);
        assert_eq!(p["last_page"], 3);
        assert_eq!(p["links"].as_array().unwrap().len(), 5);
        assert_eq!(p["links"][0]["page"], 1);
        assert_eq!(p["links"][2]["active"], true);
        assert_eq!(p["next_page_url"], "http://h/x?page=3");
    }

    #[test]
    fn empty_paginator() {
        let p = paginator_json("http://h/x", vec![], 0, 25, 1);
        assert_eq!(p["from"], Value::Null);
        assert_eq!(p["last_page"], 1);
        assert_eq!(p["links"][0]["page"], Value::Null);
        assert_eq!(p["links"][2]["page"], Value::Null);
    }

    #[test]
    fn large_paginator_window() {
        let p = paginator_json("http://h/x", vec![json!(1)], 1000, 10, 50);
        let labels: Vec<String> = p["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["label"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            labels,
            [
                "&laquo; Previous",
                "1",
                "2",
                "...",
                "47",
                "48",
                "49",
                "50",
                "51",
                "52",
                "53",
                "...",
                "99",
                "100",
                "Next &raquo;"
            ]
        );
    }

    #[test]
    fn query_parsing_trims_and_nulls() {
        let q = query_input(Some("search=+a%20b+&per_page=&tag[]=x&tag[]=y"));
        assert_eq!(q["search"], "a b");
        assert_eq!(q["per_page"], Value::Null);
        assert_eq!(q["tag"], json!(["x", "y"]));
    }
}
