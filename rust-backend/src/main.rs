//! TinyHelm Rust front-door (strangler gateway).
//!
//! Today it serves the TypeScript SPA and transparently proxies every `/api/*`
//! request to the existing Laravel backend. As endpoints are reimplemented in
//! Rust, route them here before the proxy fallback — the SPA never notices.
//!
//! Env:
//!   TINYHELM_LISTEN     bind address        (default 0.0.0.0:8080)
//!   TINYHELM_API_ORIGIN Laravel origin      (default http://127.0.0.1:8000)
//!   TINYHELM_SPA_DIR    built SPA directory (default ../frontend/dist)

use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{uri::Uri, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::{services::ServeDir, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    api_origin: String,
    http: reqwest::Client,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let listen = std::env::var("TINYHELM_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let api_origin =
        std::env::var("TINYHELM_API_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8000".into());
    let spa_dir = std::env::var("TINYHELM_SPA_DIR").unwrap_or_else(|_| "../frontend/dist".into());

    let state = AppState {
        api_origin: api_origin.trim_end_matches('/').to_string(),
        http: reqwest::Client::new(),
    };

    // SPA static files with index.html fallback for client-side routing.
    let spa = ServeDir::new(&spa_dir).append_index_html_on_directories(true);

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        // Everything under /api and /webhooks still goes to Laravel until ported.
        .route("/api/*path", get(proxy).post(proxy).patch(proxy).put(proxy).delete(proxy))
        .route("/webhooks/*path", get(proxy).post(proxy))
        .with_state(state)
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = listen.parse().expect("invalid TINYHELM_LISTEN");
    tracing::info!("TinyHelm gateway on http://{addr} -> api {}", api_origin);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Transparent reverse proxy to the Laravel backend.
async fn proxy(State(state): State<AppState>, req: Request) -> Response {
    let path_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(req.uri().path());
    let target = format!("{}{}", state.api_origin, path_query);

    let Ok(uri) = target.parse::<Uri>() else {
        return (StatusCode::BAD_GATEWAY, "bad upstream uri").into_response();
    };

    let (parts, body) = req.into_parts();
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "unreadable body").into_response(),
    };

    let mut out = state
        .http
        .request(parts.method, uri.to_string())
        .body(body_bytes);
    for (name, value) in parts.headers.iter() {
        if name != reqwest::header::HOST {
            out = out.header(name, value);
        }
    }

    match out.send().await {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            let stream = resp.bytes_stream();
            let mut builder = Response::builder().status(status);
            for (name, value) in headers.iter() {
                builder = builder.header(name, value);
            }
            builder
                .body(Body::from_stream(stream))
                .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
        }
        Err(err) => {
            tracing::error!("proxy error: {err}");
            (StatusCode::BAD_GATEWAY, "upstream unreachable").into_response()
        }
    }
}
