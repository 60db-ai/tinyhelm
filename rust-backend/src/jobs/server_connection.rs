//! Port of `app/Jobs/ServerConnectionCheckJob.php`: is the server reachable over SSH, and is Docker usable?
//!
//! Branches that need code not ported yet go to the PHP job through [`super::laravel_dispatch`]:
//! host key learning (no pinned key) and host key changes (`recordHostKeyMismatch`), servers being
//! claimed by a transfer (`ServerTransferClaimer`), and the reachability notifications
//! (`ServerReachabilityChanged` -> `isReachableChanged`).

use std::time::Duration;

use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::ssh::{run_raw, sudo::parse_commands_by_line_for_sudo, ServerConnection};

pub const UNREACHABLE_THRESHOLD: i32 = 2;
const SSH_ATTEMPTS: u32 = 2;
const SSH_TIMEOUT_SECONDS: u64 = 10;
const SSH_RETRY_DELAY_SECONDS: u64 = 3;
const DOCKER_TIMEOUT_SECONDS: u64 = 15;
pub const JOB_TIMEOUT_SECONDS: u64 = 60;

/// `ServerConnectionCheckJob::probeCommand()`.
pub const PROBE: &str =
    "cat /data/tinyhelm/transfer/owner.json 2>/dev/null || true; ls -la / > /dev/null";
/// `remoteCommandWithTimeout($command, DOCKER_TIMEOUT_SECONDS - 5)`.
pub const DOCKER_VERSION: &str = "timeout -k 5s 10s docker version --format json";
pub const COMPOSE_VERSION: &str = "timeout -k 5s 10s docker compose version --short";

const PLACEHOLDER_IPS: &[&str] = &["1.2.3.4", "0.0.0.0", "::"];
const UNREACHABLE_NOTIFICATION: &str = r"App\Notifications\Server\Unreachable";
const NOW: &str = "date_trunc('second', now())";

pub fn is_placeholder_ip(ip: &str) -> bool {
    ip.trim().is_empty() || PLACEHOLDER_IPS.contains(&ip)
}

/// `parseDockerEngineVersion`: first `N.N[.N]` in the text, normalized to `N.N.N`.
pub fn parse_docker_version(raw: &str) -> Option<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\d+\.\d+(?:\.\d+)?").expect("valid regex"));
    let m = re.find(raw)?.as_str();
    let mut parts = m.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    Some(format!(
        "{}.{}.{}",
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0)
    ))
}

/// `SshMultiplexingHelper::isHostKeyMismatch`.
fn is_host_key_mismatch(stderr: &str) -> bool {
    stderr.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
        || stderr.contains("Host key verification failed")
}

struct State {
    ip: String,
    unreachable_count: i32,
    was_reachable: bool,
    was_notified: bool,
    force_disabled: bool,
    needs_laravel: bool,
}

async fn load_state(pool: &PgPool, server_id: i64) -> Result<State, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT s.ip, s.unreachable_count, s.server_metadata->'transfer'->>'status' AS transfer_status,
                  COALESCE(ss.is_reachable, false) AS is_reachable,
                  COALESCE(ss.force_disabled, false) AS force_disabled,
                  NULLIF(btrim(COALESCE(ss.ssh_host_key, '')), '') IS NOT NULL AS has_host_key,
                  NULLIF(btrim(COALESCE(ss.ssh_host_key_pending, '')), '') IS NOT NULL AS has_pending_key,
                  COALESCE(ss.is_ssh_host_key_check_disabled, false) AS check_disabled,
                  EXISTS (SELECT 1 FROM notification_throttles nt WHERE nt.notifiable_type = 'App\Models\Server'
                          AND nt.notifiable_id = s.id AND nt.notification = $2) AS was_notified
           FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id WHERE s.id = $1"#,
    )
    .bind(server_id)
    .bind(UNREACHABLE_NOTIFICATION)
    .fetch_one(pool)
    .await?;
    let check_off = row.get::<bool, _>("check_disabled")
        || std::env::var("SSH_HOST_KEY_CHECKING")
            .is_ok_and(|m| m.trim().eq_ignore_ascii_case("off"));
    let learning = !check_off && !row.get::<bool, _>("has_host_key");
    let claimed = row.get::<Option<String>, _>("transfer_status").as_deref() == Some("claimed");
    Ok(State {
        ip: row.get("ip"),
        unreachable_count: row.get::<Option<i32>, _>("unreachable_count").unwrap_or(0),
        was_reachable: row.get("is_reachable"),
        was_notified: row.get("was_notified"),
        force_disabled: row.get("force_disabled"),
        needs_laravel: learning || row.get::<bool, _>("has_pending_key") || claimed,
    })
}

/// `$server->settings->update([...])`: write only when a value changes (Eloquent skips clean models).
async fn set_reachability(
    pool: &PgPool,
    server_id: i64,
    reachable: bool,
    usable: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        "UPDATE server_settings SET is_reachable = $2, is_usable = $3, updated_at = {NOW} \
         WHERE server_id = $1 AND (is_reachable IS DISTINCT FROM $2 OR is_usable IS DISTINCT FROM $3)"
    ))
    .bind(server_id)
    .bind(reachable)
    .bind(usable)
    .execute(pool)
    .await?;
    Ok(())
}

enum HandOff {
    /// The whole check runs in the PHP job.
    Check,
    /// `ServerReachabilityChanged` (notifications).
    ReachabilityChanged,
}

async fn hand_to_laravel(server_id: i64, what: HandOff) {
    let code = match what {
        HandOff::Check => format!(
            "\\App\\Jobs\\ServerConnectionCheckJob::dispatch(\\App\\Models\\Server::findOrFail({server_id}));"
        ),
        HandOff::ReachabilityChanged => format!(
            "\\App\\Events\\ServerReachabilityChanged::dispatch(\\App\\Models\\Server::findOrFail({server_id}));"
        ),
    };
    if let Err(e) = super::laravel_dispatch(&code).await {
        tracing::error!("server {server_id}: {e}");
    }
}

/// Run the check for one server. Errors are logged, like the PHP job's catch-all.
pub async fn run(pool: &PgPool, app_key: &[u8; 32], server_id: i64) {
    if let Err(e) = check(pool, app_key, server_id).await {
        tracing::error!("ServerConnectionCheckJob failed for server {server_id}: {e}");
    }
}

async fn check(pool: &PgPool, app_key: &[u8; 32], server_id: i64) -> Result<(), sqlx::Error> {
    let state = load_state(pool, server_id).await?;
    if is_placeholder_ip(&state.ip) {
        return Ok(());
    }
    if state.force_disabled {
        return set_reachability(pool, server_id, false, false).await;
    }
    if state.needs_laravel {
        hand_to_laravel(server_id, HandOff::Check).await;
        return Ok(());
    }

    let server = ServerConnection::load(pool, app_key, server_id).await.ok();
    let reachable = match &server {
        Some(server) => match check_connection(server).await {
            Probe::Reachable => true,
            Probe::Unreachable => false,
            Probe::HostKeyChanged => {
                hand_to_laravel(server_id, HandOff::Check).await;
                return Ok(());
            }
        },
        None => false, // no usable key: the PHP job's ssh call fails the same way
    };

    let Some(server) = server.filter(|_| reachable) else {
        let count: i32 = sqlx::query_scalar(&format!(
            "UPDATE servers SET unreachable_count = COALESCE(unreachable_count, 0) + 1, \
             updated_at = {NOW} WHERE id = $1 RETURNING unreachable_count"
        ))
        .bind(server_id)
        .fetch_one(pool)
        .await?;
        if count < UNREACHABLE_THRESHOLD {
            return Ok(());
        }
        set_reachability(pool, server_id, false, false).await?;
        if !state.was_notified {
            hand_to_laravel(server_id, HandOff::ReachabilityChanged).await;
        }
        return Ok(());
    };

    let usable = check_docker(pool, &server, server_id).await?;
    set_reachability(pool, server_id, true, usable).await?;
    if state.unreachable_count > 0 {
        sqlx::query(&format!(
            "UPDATE servers SET unreachable_count = 0, updated_at = {NOW} WHERE id = $1"
        ))
        .bind(server_id)
        .execute(pool)
        .await?;
    }
    if !state.was_reachable || state.was_notified {
        hand_to_laravel(server_id, HandOff::ReachabilityChanged).await;
    }
    Ok(())
}

enum Probe {
    Reachable,
    Unreachable,
    HostKeyChanged,
}

fn script(server: &ServerConnection, command: &str) -> String {
    if server.is_non_root() {
        parse_commands_by_line_for_sudo(&[command.to_string()], &server.user).remove(0)
    } else {
        command.to_string()
    }
}

async fn check_connection(server: &ServerConnection) -> Probe {
    for attempt in 1..=SSH_ATTEMPTS {
        if attempt > 1 {
            tokio::time::sleep(Duration::from_secs(SSH_RETRY_DELAY_SECONDS)).await;
        }
        match run_raw(server, &script(server, PROBE), SSH_TIMEOUT_SECONDS, true).await {
            Ok(out) if is_host_key_mismatch(&out.stderr) => return Probe::HostKeyChanged,
            Ok(out) if out.code == Some(0) => return Probe::Reachable,
            Ok(out) => tracing::debug!(
                "server {}: ssh exit {:?}: {}",
                server.uuid,
                out.code,
                out.stderr.trim()
            ),
            Err(e) => tracing::debug!("server {}: connection check failed: {e}", server.uuid),
        }
    }
    Probe::Unreachable
}

/// `checkDockerAvailability`: usable when `docker version` reports a server version; remembers the
/// Docker and Compose versions on the settings row.
async fn check_docker(
    pool: &PgPool,
    server: &ServerConnection,
    server_id: i64,
) -> Result<bool, sqlx::Error> {
    let Some(output) = docker_command(server, DOCKER_VERSION).await else {
        return Ok(false);
    };
    let info: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    let server_version = info.pointer("/Server/Version").filter(|v| !v.is_null());
    if let Some(version) = server_version
        .and_then(Value::as_str)
        .and_then(parse_docker_version)
    {
        remember(pool, server_id, "docker_version", &version).await?;
    }
    let compose = docker_command(server, COMPOSE_VERSION).await;
    if let Some(version) = compose.as_deref().and_then(parse_docker_version) {
        remember(pool, server_id, "compose_version", &version).await?;
    }
    Ok(server_version.is_some())
}

/// `rememberDockerVersion` / `rememberComposeVersion`.
async fn remember(
    pool: &PgPool,
    server_id: i64,
    column: &str,
    version: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        "UPDATE server_settings SET {column} = $2, {column}_checked_at = {NOW}, updated_at = {NOW} \
         WHERE server_id = $1"
    ))
    .bind(server_id)
    .bind(version)
    .execute(pool)
    .await?;
    Ok(())
}

async fn docker_command(server: &ServerConnection, command: &str) -> Option<String> {
    let out = run_raw(
        server,
        &script(server, command),
        DOCKER_TIMEOUT_SECONDS,
        false,
    )
    .await
    .ok()?;
    let text = out.stdout.trim();
    (out.code == Some(0) && !text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Golden {
        commands: std::collections::HashMap<String, String>,
        non_root_commands: std::collections::HashMap<String, String>,
        versions: Vec<VersionCase>,
    }

    #[derive(serde::Deserialize)]
    struct VersionCase {
        raw: String,
        parsed: Option<String>,
    }

    fn golden() -> Golden {
        serde_json::from_str(include_str!("../../tests/fixtures/server_jobs_golden.json")).unwrap()
    }

    #[test]
    fn commands_match_laravel() {
        let g = golden();
        let ours = [
            ("probe", PROBE),
            ("docker_version", DOCKER_VERSION),
            ("compose_version", COMPOSE_VERSION),
        ];
        for (name, cmd) in ours {
            assert_eq!(g.commands[name], cmd, "{name}");
            let rewritten = parse_commands_by_line_for_sudo(&[cmd.to_string()], "ubuntu").remove(0);
            assert_eq!(g.non_root_commands[name], rewritten, "{name} (non-root)");
        }
    }

    #[test]
    fn docker_version_parsing_matches_laravel() {
        for case in golden().versions {
            assert_eq!(parse_docker_version(&case.raw), case.parsed, "{}", case.raw);
        }
    }

    /// `TEST_DATABASE_URL=… TEST_APP_KEY=… TEST_SERVER_ID=1 cargo test --bin worker live_probe -- --ignored`
    /// Runs the probe and `docker version` without the sudo rewrite (for a test host whose SSH user
    /// lacks passwordless sudo; Laravel would send the sudo form there and fail the same way).
    #[tokio::test]
    #[ignore = "needs a reachable server with Docker, the TinyHelm database and APP_KEY"]
    async fn live_probe_and_docker_version() {
        let pool = PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let key = crate::crypto::parse_key(&std::env::var("TEST_APP_KEY").unwrap()).unwrap();
        let id: i64 = std::env::var("TEST_SERVER_ID").unwrap().parse().unwrap();
        let server = ServerConnection::load(&pool, &key, id).await.unwrap();
        let probe = run_raw(&server, PROBE, SSH_TIMEOUT_SECONDS, true)
            .await
            .unwrap();
        assert_eq!(probe.code, Some(0), "{}", probe.stderr);
        let path = "export PATH=\"$PATH:/usr/local/bin:/opt/homebrew/bin\"\n";
        let docker = run_raw(
            &server,
            &format!("{path}{DOCKER_VERSION}"),
            DOCKER_TIMEOUT_SECONDS,
            false,
        )
        .await
        .unwrap();
        let info: Value = serde_json::from_str(docker.stdout.trim()).unwrap();
        let version = info
            .pointer("/Server/Version")
            .and_then(Value::as_str)
            .and_then(parse_docker_version);
        assert!(version.is_some(), "no server version in {}", docker.stdout);
    }

    #[test]
    fn docker_version_json_recorded_from_a_real_engine() {
        let raw = include_str!("../../tests/fixtures/docker_version.json");
        let info: Value = serde_json::from_str(raw).unwrap();
        let v = info
            .pointer("/Server/Version")
            .and_then(Value::as_str)
            .and_then(parse_docker_version);
        assert!(v.is_some_and(|v| v.split('.').count() == 3));
        assert!(is_host_key_mismatch(
            "@@@ WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! @@@"
        ));
        assert!(is_placeholder_ip("1.2.3.4") && !is_placeholder_ip("10.0.0.1"));
    }
}
