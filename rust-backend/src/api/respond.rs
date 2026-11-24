//! Native `/api/v1` handlers: respond.

#[allow(unused_imports)]
use super::prelude::*;

// ---- shared response helpers ----
pub(crate) fn ok_json<T: Serialize>(rows: T) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::to_value(rows).unwrap_or(json!([]))),
    )
        .into_response()
}
pub(crate) fn not_found(msg: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "message": msg }))).into_response()
}
pub(crate) fn query_err(what: &str, e: sqlx::Error) -> Response {
    tracing::error!("{what} query failed: {e}");
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({"message": "Query failed."})),
    )
        .into_response()
}
