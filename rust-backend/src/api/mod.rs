//! Native `/api/v1` endpoints (the Rust replacement of `routes/api.php`), one module per domain.
//!
//! Every native route runs behind [`auth::native_guard`], which replicates Laravel's API middleware
//! and forwards the requests it cannot replicate yet to Laravel. Handlers then call
//! [`auth::Ctx::require`] with the route's ability and record writes with [`audit::record`].

pub mod applications;
pub mod audit;
pub mod auth;
pub mod dashboard;
pub mod databases;
pub mod envs;
pub mod projects;
pub mod respond;
pub mod servers;
pub mod services;
pub mod tags;
pub mod team;
pub mod throttle;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Shared imports for the handler modules (they also see each other's helpers through it).
pub(crate) mod prelude {
    pub(crate) use axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        Extension, Json,
    };
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::json;
    pub(crate) use sqlx::{PgPool, Row};

    pub(crate) use super::audit;
    pub(crate) use super::auth::Ctx;
    pub(crate) use super::{
        applications::*, dashboard::*, databases::*, envs::*, projects::*, respond::*, servers::*,
        services::*, tags::*, team::*,
    };
}

/// Pool on DATABASE_URL (the database Laravel uses). None if unset or unreachable.
pub async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    match PgPoolOptions::new().max_connections(10).connect(&url).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::error!("native db connect failed: {e}");
            None
        }
    }
}

use axum::{
    middleware,
    routing::{delete, get, patch},
    Router,
};

use crate::AppState;

/// Route groups that can be switched to Rust one at a time with `RUST_NATIVE_ROUTES`.
pub const GROUPS: &[&str] = &[
    "servers",
    "projects",
    "applications",
    "services",
    "databases",
    "tags",
    "team",
];

/// Native routes for the enabled groups, all behind [`auth::native_guard`].
pub fn router(state: AppState, enabled: &[&str]) -> Router<AppState> {
    use prelude::*;
    let on = |g: &str| enabled.contains(&g);
    let mut r = Router::new();
    if on("servers") {
        r = r
            .route("/api/v1/servers", get(list_servers))
            .route("/api/v1/servers/:uuid", get(get_server));
    }
    if on("projects") {
        r = r
            .route("/api/v1/projects", get(list_projects).post(create_project))
            .route(
                "/api/v1/projects/:uuid",
                patch(update_project).delete(delete_project),
            )
            .route(
                "/api/v1/projects/:uuid/environments",
                get(list_environments),
            )
            .route("/api/v1/dashboard", get(dashboard));
    }
    if on("team") {
        r = r
            .route("/api/v1/team", get(get_current_team))
            .route(
                "/api/v1/team/envs",
                get(list_team_envs).post(create_team_env),
            )
            .route(
                "/api/v1/team/envs/:id",
                patch(update_team_env).delete(delete_team_env),
            );
    }
    if on("applications") {
        r = r
            .route("/api/v1/applications", get(list_applications))
            .route("/api/v1/applications/:uuid", get(get_application))
            .route(
                "/api/v1/applications/:uuid/envs",
                get(list_app_envs)
                    .post(create_app_env)
                    .patch(update_app_env),
            )
            .route(
                "/api/v1/applications/:uuid/envs/:env_uuid",
                delete(delete_app_env),
            );
    }
    if on("services") {
        r = r
            .route("/api/v1/services", get(list_services))
            .route("/api/v1/services/:uuid", get(get_service))
            .route(
                "/api/v1/services/:uuid/envs",
                get(list_service_envs)
                    .post(create_service_env)
                    .patch(update_service_env),
            )
            .route(
                "/api/v1/services/:uuid/envs/:env_uuid",
                delete(delete_service_env),
            );
    }
    if on("databases") {
        r = r
            .route("/api/v1/databases", get(list_databases))
            .route("/api/v1/databases/:uuid", get(get_database))
            .route(
                "/api/v1/databases/:uuid/envs",
                get(list_database_envs)
                    .post(create_database_env)
                    .patch(update_database_env),
            )
            .route(
                "/api/v1/databases/:uuid/envs/:env_uuid",
                delete(delete_database_env),
            );
    }
    if on("tags") {
        r = r
            .route("/api/v1/tags", get(list_tags).post(create_tag))
            .route("/api/v1/tags/:uuid", delete(delete_tag));
    }
    r.route_layer(middleware::from_fn_with_state(state, auth::native_guard))
}
