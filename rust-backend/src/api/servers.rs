//! Native `/api/v1` handlers: servers.

#[allow(unused_imports)]
use super::prelude::*;

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct ServerRow {
    uuid: String,
    name: String,
    description: Option<String>,
    ip: String,
    port: i32,
    #[sqlx(rename = "user")]
    user: String,
    is_reachable: Option<bool>,
    is_usable: Option<bool>,
}

pub(crate) const SERVER_SELECT: &str = r#"SELECT s.uuid, s.name, s.description, s.ip, s.port, s."user",
    ss.is_reachable, ss.is_usable
    FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id"#;

/// GET /api/v1/servers — team-scoped, mirrors the Laravel endpoint's core fields.
pub async fn list_servers(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let sql = format!("{SERVER_SELECT} WHERE s.team_id = $1 ORDER BY s.id");
    match sqlx::query_as::<_, ServerRow>(&sql)
        .bind(team_id)
        .fetch_all(pool)
        .await
    {
        Ok(r) => (
            StatusCode::OK,
            Json(serde_json::to_value(r).unwrap_or(json!([]))),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("list_servers query failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"message": "Query failed."})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/servers/:uuid — single server, team-scoped.
pub async fn get_server(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let sql = format!("{SERVER_SELECT} WHERE s.team_id = $1 AND s.uuid = $2 LIMIT 1");
    match sqlx::query_as::<_, ServerRow>(&sql)
        .bind(team_id)
        .bind(&uuid)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"message": "Server not found."})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("get_server query failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"message": "Query failed."})),
            )
                .into_response()
        }
    }
}
