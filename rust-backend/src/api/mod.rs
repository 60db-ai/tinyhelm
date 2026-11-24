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
pub mod instance;
pub mod model;
pub mod notifications;
pub mod projects;
pub mod resources;
pub mod respond;
pub mod servers;
pub mod services;
pub mod shared_envs;
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
    pub(crate) use super::model::{all_to_laravel, to_laravel, ModelSpec};
    pub(crate) use super::{
        applications::*, dashboard::*, databases::*, envs::*, instance::*, notifications::*, projects::*, resources::*, respond::*, servers::*, services::*, shared_envs::*, support::*, tags::*, team::*,
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
    routing::{delete, get, patch, post},
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
    "notifications",
    "instance",
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
            .route(
                "/api/v1/team",
                get(get_current_team).patch(update_current_team),
            )
            .route(
                "/api/v1/team/envs",
                get(list_team_envs).post(create_team_env),
            )
            .route(
                "/api/v1/team/envs/:id",
                patch(update_team_env).delete(delete_team_env),
            )
            .route("/api/v1/team/members", get(list_current_team_members))
            .route("/api/v1/teams", get(list_teams))
            .route("/api/v1/teams/current", get(get_current_team))
            .route(
                "/api/v1/teams/current/members",
                get(list_current_team_members),
            )
            .route("/api/v1/teams/:id", get(get_team_by_id))
            .route("/api/v1/teams/:id/members", get(list_team_members_by_id));
    }
    if on("notifications") {
        r = r
            .route(
                "/api/v1/notifications/email",
                get(get_email_notifications).patch(update_email_notifications),
            )
            .route(
                "/api/v1/notifications/discord",
                get(get_discord_notifications).patch(update_discord_notifications),
            )
            .route(
                "/api/v1/notifications/slack",
                get(get_slack_notifications).patch(update_slack_notifications),
            )
            .route(
                "/api/v1/notifications/telegram",
                get(get_telegram_notifications).patch(update_telegram_notifications),
            )
            .route(
                "/api/v1/notifications/pushover",
                get(get_pushover_notifications).patch(update_pushover_notifications),
            )
            .route(
                "/api/v1/notifications/webhook",
                get(get_webhook_notifications).patch(update_webhook_notifications),
            );
    }
    if on("instance") {
        r = r
            .route("/api/v1/version", get(get_version))
            .route("/api/v1/realtime/config", get(get_realtime_config))
            .route("/api/v1/disable", get(post_required).post(disable_api))
            .route("/api/v1/mcp/enable", axum::routing::post(enable_mcp))
            .route("/api/v1/mcp/disable", axum::routing::post(disable_mcp));
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
    // Step 3: project detail/environments, scoped shared variables, tag update, resource tags, resources.
    if on("projects") {
        r = r
            .route("/api/v1/projects/:uuid", get(get_project))
            .route(
                "/api/v1/projects/:uuid/:environment",
                get(environment_details),
            )
            .route(
                "/api/v1/projects/:uuid/environments",
                post(create_environment),
            )
            .route(
                "/api/v1/projects/:uuid/environments/:environment",
                patch(update_environment).delete(delete_environment),
            )
            .route(
                "/api/v1/projects/:uuid/envs",
                get(list_project_envs).post(create_project_env),
            )
            .route(
                "/api/v1/projects/:uuid/envs/:env_id",
                patch(update_project_env).delete(delete_project_env),
            )
            .route(
                "/api/v1/projects/:uuid/environments/:environment/envs",
                get(list_environment_envs).post(create_environment_env),
            )
            .route(
                "/api/v1/projects/:uuid/environments/:environment/envs/:env_id",
                patch(update_environment_env).delete(delete_environment_env),
            )
            .route("/api/v1/resources", get(list_resources));
    }
    if on("servers") {
        r = r
            .route(
                "/api/v1/servers/:uuid/envs",
                get(list_server_envs).post(create_server_env),
            )
            .route(
                "/api/v1/servers/:uuid/envs/:env_id",
                patch(update_server_env).delete(delete_server_env),
            );
    }
    if on("tags") {
        r = r
            .route("/api/v1/tags/:uuid", patch(update_tag))
            .route(
                "/api/v1/applications/:uuid/tags",
                get(list_application_tags).post(attach_application_tags),
            )
            .route(
                "/api/v1/applications/:uuid/tags/:tag_uuid",
                delete(detach_application_tag),
            )
            .route(
                "/api/v1/services/:uuid/tags",
                get(list_service_tags).post(attach_service_tags),
            )
            .route(
                "/api/v1/services/:uuid/tags/:tag_uuid",
                delete(detach_service_tag),
            )
            .route(
                "/api/v1/databases/:uuid/tags",
                get(list_database_tags).post(attach_database_tags),
            )
            .route(
                "/api/v1/databases/:uuid/tags/:tag_uuid",
                delete(detach_database_tag),
            );
    }
    // A path served natively for some methods must still reach Laravel for the others
    // (e.g. PATCH /applications/{uuid}/envs/bulk matches the DELETE-only {env_uuid} route).
    let guarded = r
        .route_layer(middleware::from_fn_with_state(state, auth::native_guard))
        .method_not_allowed_fallback(crate::proxy);
    if !on("instance") {
        return guarded;
    }
    // Outside the auth group in routes/api.php (only `throttle:api`, applied by the handler).
    guarded.merge(
        Router::new()
            .route("/api/health", get(health))
            .route("/api/v1/health", get(health))
            .method_not_allowed_fallback(crate::proxy),
    )
}
