//! Port of the server-check parts of `app/Jobs/ServerManagerJob.php` (every minute, self-hosted):
//! connection checks with unreachable backoff, and the per-server storage check schedule.
//! The other dispatches of that job (cloud provider status, Sentinel, patch checks, image scans,
//! `ServerCheckJob`) stay in Laravel until they are ported.

use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Timelike, Utc};
use sqlx::{PgPool, Row};

use super::{cron, owns, run_exclusive, server_connection};
use crate::ssh::{instant_remote_process, RunOptions, ServerConnection};

/// `Server::storageCheck()`.
pub const STORAGE_COMMAND: &str = "df / --output=pcent | tr -cd 0-9";

struct ServerRow {
    id: i64,
    uuid: String,
    ip: String,
    unreachable_count: i32,
    sentinel_updated_at: Option<NaiveDateTime>,
    transferred_away: bool,
    is_reachable: bool,
    is_usable: bool,
    force_disabled: bool,
    build_server: bool,
    swarm: bool,
    push_interval: i64,
    server_timezone: String,
    disk_check_frequency: Option<String>,
}

impl ServerRow {
    /// `waitBeforeDoingSshCheck`: three Sentinel push intervals, at least 120 seconds.
    fn wait_seconds(&self) -> i64 {
        (self.push_interval * 3).max(120)
    }

    /// `Carbon::parse($server->sentinel_updated_at)`; Carbon parses null as "now".
    fn sentinel_time(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        self.sentinel_updated_at.map_or(now, |t| t.and_utc())
    }

    fn sentinel_out_of_sync(&self, now: DateTime<Utc>) -> bool {
        self.sentinel_time(now) < now - chrono::Duration::seconds(self.wait_seconds())
    }

    fn sentinel_live(&self, now: DateTime<Utc>) -> bool {
        self.sentinel_time(now) > now - chrono::Duration::seconds(self.wait_seconds())
    }

    /// `isSentinelEnabled`.
    fn sentinel_enabled(&self) -> bool {
        !self.build_server && !self.swarm && !self.force_disabled && !self.transferred_away
    }

    /// `isFunctional` (without the mux file cleanup).
    fn functional(&self) -> bool {
        self.is_reachable
            && self.is_usable
            && !self.force_disabled
            && !server_connection::is_placeholder_ip(&self.ip)
    }
}

async fn load_servers(pool: &PgPool) -> Result<Vec<ServerRow>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT s.id, s.uuid, s.ip, COALESCE(s.unreachable_count, 0) AS unreachable_count, s.sentinel_updated_at,
                  COALESCE(s.server_metadata->'transfer'->>'status' = 'transferred', false) AS transferred_away,
                  COALESCE(ss.is_reachable, false) AS is_reachable, COALESCE(ss.is_usable, false) AS is_usable,
                  COALESCE(ss.force_disabled, false) AS force_disabled,
                  COALESCE(ss.server_role = 'build' OR (ss.server_role IS NULL AND ss.is_build_server), false) AS build_server,
                  COALESCE(ss.is_swarm_manager OR ss.is_swarm_worker, false) AS swarm,
                  COALESCE(ss.sentinel_push_interval_seconds, 60)::bigint AS push_interval,
                  COALESCE(ss.server_timezone, '') AS server_timezone, ss.server_disk_usage_check_frequency
           FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id ORDER BY s.id"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ServerRow {
            id: r.get("id"),
            uuid: r.get("uuid"),
            ip: r.get("ip"),
            unreachable_count: r.get("unreachable_count"),
            sentinel_updated_at: r.get("sentinel_updated_at"),
            transferred_away: r.get("transferred_away"),
            is_reachable: r.get("is_reachable"),
            is_usable: r.get("is_usable"),
            force_disabled: r.get("force_disabled"),
            build_server: r.get("build_server"),
            swarm: r.get("swarm"),
            push_interval: r.get("push_interval"),
            server_timezone: r.get("server_timezone"),
            disk_check_frequency: r.get("server_disk_usage_check_frequency"),
        })
        .collect())
}

/// `getBackoffCycleInterval`.
fn backoff_interval(unreachable_count: i32) -> u32 {
    match unreachable_count {
        i32::MIN..=2 => 1,
        3..=5 => 3,
        6..=11 => 6,
        _ => 12,
    }
}

/// `shouldSkipDueToBackoff` (self-hosted: one-minute cycles).
fn skip_due_to_backoff(server_id: i64, unreachable_count: i32, minute: u32) -> bool {
    let interval = backoff_interval(unreachable_count);
    if interval <= 1 {
        return false;
    }
    let hash = u64::from(crc32(server_id.to_string().as_bytes()));
    (u64::from(minute) + hash) % u64::from(interval) != 0
}

/// PHP `crc32()` (IEEE 802.3).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

async fn instance_timezone(pool: &PgPool) -> String {
    let tz: Option<String> =
        sqlx::query_scalar("SELECT instance_timezone FROM instance_settings WHERE id = 0")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();
    tz.filter(|t| t.parse::<chrono_tz::Tz>().is_ok())
        .unwrap_or_else(|| "UTC".into())
}

/// One scheduler minute.
pub async fn tick(pool: &PgPool, app_key: &[u8; 32], now: DateTime<Utc>) {
    let servers = match load_servers(pool).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("server manager: could not load servers: {e}");
            return;
        }
    };
    let instance_tz = instance_timezone(pool).await;
    let run_connection_checks = owns("server-connection-check")
        && cron::should_run_now("* * * * *", &instance_tz, "server-connection-checks", now);

    let mut tasks = Vec::new();
    for server in servers
        .into_iter()
        .filter(|s| !server_connection::is_placeholder_ip(&s.ip))
    {
        let sentinel_covers = server.sentinel_enabled()
            && server.sentinel_live(now)
            && server.is_reachable
            && server.is_usable;
        if run_connection_checks
            && !sentinel_covers
            && !skip_due_to_backoff(server.id, server.unreachable_count, now.minute())
        {
            let (pool, key, id) = (pool.clone(), *app_key, server.id);
            let lock = format!("server-connection-check-{}", server.uuid);
            tasks.push(tokio::spawn(async move {
                let timeout = Duration::from_secs(server_connection::JOB_TIMEOUT_SECONDS + 30);
                run_exclusive(
                    &pool,
                    &lock,
                    timeout,
                    server_connection::run(&pool, &key, id),
                )
                .await;
            }));
        }
        if owns("server-storage-check") && server.sentinel_out_of_sync(now) {
            // `data_get($settings, 'server_timezone', $instanceTz)`; invalid zones fall back to UTC.
            let server_tz = if server.server_timezone.parse::<chrono_tz::Tz>().is_ok() {
                server.server_timezone.clone()
            } else {
                "UTC".to_string()
            };
            let freq = server
                .disk_check_frequency
                .clone()
                .unwrap_or_else(|| "0 23 * * *".into());
            let key = format!("server-storage-check:{}", server.id);
            if cron::should_run_now(&freq, &server_tz, &key, now) {
                let (pool, app_key) = (pool.clone(), *app_key);
                tasks.push(tokio::spawn(async move {
                    storage_check(&pool, &app_key, &server).await;
                }));
            }
        }
    }
    for task in tasks {
        let _ = task.await;
    }
}

/// `ServerStorageCheckJob`: the disk usage comes from Rust; the notification decision and sending
/// stay in the PHP job, which accepts the percentage as its second argument.
async fn storage_check(pool: &PgPool, app_key: &[u8; 32], server: &ServerRow) {
    if !server.functional() {
        return;
    }
    let Ok(conn) = ServerConnection::load(pool, app_key, server.id).await else {
        return;
    };
    // instant_remote_process($commands, $this, false): errors give null.
    let pct = instant_remote_process(&conn, &[STORAGE_COMMAND.to_string()], RunOptions::default())
        .await
        .ok()
        .flatten()
        .filter(|p| !p.is_empty() && p != "0" && p.bytes().all(|b| b.is_ascii_digit()));
    let Some(pct) = pct else {
        return;
    };
    let code = format!(
        "\\App\\Jobs\\ServerStorageCheckJob::dispatch(\\App\\Models\\Server::findOrFail({}), '{pct}');",
        server.id
    );
    if let Err(e) = super::laravel_dispatch(&code).await {
        tracing::error!("server {}: storage check hand-off: {e}", server.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Golden {
        commands: std::collections::HashMap<String, String>,
        backoff: Vec<Backoff>,
    }

    #[derive(serde::Deserialize)]
    struct Backoff {
        id: i64,
        count: i32,
        minute: u32,
        skip: bool,
    }

    fn golden() -> Golden {
        serde_json::from_str(include_str!("../../tests/fixtures/server_jobs_golden.json")).unwrap()
    }

    #[test]
    fn storage_command_matches_laravel() {
        assert_eq!(golden().commands["storage"], STORAGE_COMMAND);
    }

    #[test]
    fn backoff_matches_laravel() {
        let g = golden();
        assert_eq!(g.backoff.len(), 280);
        for b in g.backoff {
            assert_eq!(
                skip_due_to_backoff(b.id, b.count, b.minute),
                b.skip,
                "id {} count {} minute {}",
                b.id,
                b.count,
                b.minute
            );
        }
    }

    #[test]
    fn crc32_matches_php() {
        assert_eq!(
            crc32(b"The quick brown fox jumped over the lazy dog."),
            2191738434
        );
    }

    #[test]
    fn sentinel_null_timestamp_counts_as_now() {
        let now = Utc::now();
        let row = ServerRow {
            id: 1,
            uuid: "u".into(),
            ip: "10.0.0.1".into(),
            unreachable_count: 0,
            sentinel_updated_at: None,
            transferred_away: false,
            is_reachable: true,
            is_usable: true,
            force_disabled: false,
            build_server: false,
            swarm: false,
            push_interval: 60,
            server_timezone: "UTC".into(),
            disk_check_frequency: None,
        };
        assert!(row.sentinel_live(now) && !row.sentinel_out_of_sync(now));
        let stale = ServerRow {
            sentinel_updated_at: Some((now - chrono::Duration::minutes(10)).naive_utc()),
            ..row
        };
        assert!(!stale.sentinel_live(now) && stale.sentinel_out_of_sync(now));
    }
}
