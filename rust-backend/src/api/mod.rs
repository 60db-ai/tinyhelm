//! Native `/api/v1` endpoints (the Rust replacement of `routes/api.php`), one module per domain.
//!
//! Every native route runs behind [`auth::native_guard`], which replicates Laravel's API middleware
//! and forwards the requests it cannot replicate yet to Laravel. Handlers then call
//! [`auth::Ctx::require`] with the route's ability and record writes with [`audit::record`].

pub mod applications;
pub mod audit;
pub mod auth;
pub mod cloud;
pub mod dashboard;
pub mod databases;
pub mod envs;
pub mod git_sources;
pub mod model;
pub mod projects;
pub mod respond;
pub mod security;
pub mod servers;
pub mod services;
pub mod support;
pub mod tags;
pub mod team;
pub mod throttle;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Shared imports for the handler modules (they also see each other's helpers through it).
pub(crate) mod prelude {
    pub(crate) use axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        Extension, Json,
    };
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::json;
    pub(crate) use sqlx::{PgPool, Row};

    pub(crate) use super::audit;
    pub(crate) use super::auth::Ctx;
    pub(crate) use super::model::{to_laravel, ModelSpec};
    pub(crate) use super::{
        applications::*, cloud::*, dashboard::*, databases::*, envs::*, git_sources::*,
        projects::*, respond::*, security::*, servers::*, services::*, support::*, tags::*,
        team::*,
    };
}

/// Pool on DATABASE_URL (the database Laravel uses). None if unset or unreachable.
pub async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    // Laravel's pgsql connection runs `SET TIME ZONE 'UTC'` (config/database.php); timestamps written
    // with now() must match what PHP writes.
    let options = PgPoolOptions::new()
        .max_connections(10)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::Executor::execute(conn, "SET TIME ZONE 'UTC'").await?;
                Ok(())
            })
        });
    match options.connect(&url).await {
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
    "security",
    "cloud",
    "git_sources",
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
    if on("security") {
        r = r
            .route("/api/v1/security/keys", get(list_keys).post(create_key))
            .route(
                "/api/v1/security/keys/:uuid",
                get(get_key).patch(update_key).delete(delete_key),
            )
            .route(
                "/api/v1/security/integration-tokens",
                get(list_integration_tokens),
            )
            .route(
                "/api/v1/security/integration-tokens/:uuid",
                get(get_integration_token).delete(delete_integration_token),
            );
    }
    if on("cloud") {
        r = r
            .route("/api/v1/cloud-tokens", get(list_cloud_tokens))
            .route(
                "/api/v1/cloud-tokens/:uuid",
                get(get_cloud_token)
                    .patch(update_cloud_token)
                    .delete(delete_cloud_token),
            )
            .route(
                "/api/v1/cloud-init-scripts",
                get(list_cloud_init_scripts).post(create_cloud_init_script),
            )
            .route(
                "/api/v1/cloud-init-scripts/:uuid",
                get(get_cloud_init_script)
                    .patch(update_cloud_init_script)
                    .delete(delete_cloud_init_script),
            )
            .route("/api/v1/s3-storages", get(list_s3_storages))
            .route(
                "/api/v1/s3-storages/:uuid",
                get(get_s3_storage).delete(delete_s3_storage),
            );
    }
    if on("git_sources") {
        r = r
            .route("/api/v1/github-apps", get(list_github_apps))
            .route("/api/v1/gitlab-apps", get(list_gitlab_apps))
            .route("/api/v1/gitlab-apps/:id", delete(delete_gitlab_app))
            .route("/api/v1/gitea-apps", get(list_gitea_apps));
    }
    // A path served natively for some methods must still reach Laravel for the others
    // (e.g. PATCH /applications/{uuid}/envs/bulk matches the DELETE-only {env_uuid} route).
    r.route_layer(middleware::from_fn_with_state(state, auth::native_guard))
        .method_not_allowed_fallback(crate::proxy)
}
