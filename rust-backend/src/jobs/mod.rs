//! Background jobs and their scheduler (the Rust replacement of `app/Console/Kernel.php` + Horizon).
//!
//! A job runs in Rust only when `TINYHELM_RUST_JOBS` lists it; Laravel's `ServerManagerJob` skips the
//! same names (`ServerManagerJob::rustOwns`), so a job never runs twice. Ported:
//! - `server-connection-check`: `ServerConnectionCheckJob`, dispatched by the server manager loop.
//! - `server-storage-check`: the SSH part of `ServerStorageCheckJob` (Laravel sends the notification).
//! - `server-manager`: shorthand for both.
//!
//! Work Rust cannot do yet (notifications, host key learning, transfer claims) is handed to Laravel
//! through [`laravel_dispatch`], so behavior stays complete during the port.

pub mod cron;
// ponytail: no ported job broadcasts yet (connection checks only fire non-broadcast events);
// ServerCheckJob's *StatusChanged / ServiceChecked events will use it.
#[allow(dead_code)]
pub mod pusher;
pub mod server_connection;
pub mod server_manager;

use std::time::Duration;

use sqlx::{Connection, PgPool};

/// True when `TINYHELM_RUST_JOBS` hands `job` to Rust (`server-manager` covers the server jobs).
pub fn owns(job: &str) -> bool {
    owns_in(
        &std::env::var("TINYHELM_RUST_JOBS").unwrap_or_default(),
        job,
    )
}

fn owns_in(list: &str, job: &str) -> bool {
    let jobs: Vec<&str> = list.split(',').map(str::trim).collect();
    jobs.contains(&job)
        || (jobs.contains(&"server-manager")
            && matches!(job, "server-connection-check" | "server-storage-check"))
}

/// A Postgres session advisory lock on a dedicated connection. Dropping the guard closes that
/// connection, which releases the lock even if the job panicked (`WithoutOverlapping`, `onOneServer`).
pub struct AdvisoryLock(Option<sqlx::PgConnection>);

impl AdvisoryLock {
    /// Try to take the lock named `key`; None when another process or task holds it.
    pub async fn try_acquire(pool: &PgPool, key: &str) -> Option<Self> {
        let mut conn = pool.acquire().await.ok()?.detach();
        let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
            .bind(key)
            .fetch_one(&mut conn)
            .await
            .ok()?;
        got.then(|| Self(Some(conn)))
    }

    pub async fn release(mut self) {
        if let Some(conn) = self.0.take() {
            let _ = conn.close().await;
        }
    }
}

/// Run a job under a `WithoutOverlapping` lock with a timeout (Laravel `$timeout`, `dontRelease`).
pub async fn run_exclusive<F>(pool: &PgPool, lock_key: &str, timeout: Duration, job: F)
where
    F: std::future::Future<Output = ()>,
{
    let Some(lock) = AdvisoryLock::try_acquire(pool, lock_key).await else {
        tracing::debug!("{lock_key}: already running, skipped");
        return;
    };
    if tokio::time::timeout(timeout, job).await.is_err() {
        tracing::warn!("{lock_key}: timed out after {}s", timeout.as_secs());
    }
    lock.release().await;
}

/// Hand work to Laravel: `php artisan tinker --execute <code>` in `TINYHELM_LARAVEL_DIR` (default `..`).
/// Callers pass only fixed code with integer ids, never user input.
// ponytail: boots PHP per call (~1s); fine for rare hand-offs, replaced as each listener is ported.
pub async fn laravel_dispatch(code: &str) -> Result<(), String> {
    let dir = std::env::var("TINYHELM_LARAVEL_DIR").unwrap_or_else(|_| "..".into());
    let php = std::env::var("TINYHELM_PHP").unwrap_or_else(|_| "php".into());
    let out = tokio::process::Command::new(php)
        .args(["artisan", "tinker", "--execute", code])
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("could not run php in {dir}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "laravel hand-off failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Every minute, on one server only, run the owned schedules. Returns when no job is owned.
pub async fn run_scheduler(pool: PgPool, app_key: [u8; 32]) {
    if !owns("server-connection-check") && !owns("server-storage-check") {
        tracing::info!("scheduler: no jobs owned by Rust (TINYHELM_RUST_JOBS), not started");
        return;
    }
    tracing::info!(
        "scheduler: started (TINYHELM_RUST_JOBS={})",
        std::env::var("TINYHELM_RUST_JOBS").unwrap_or_default()
    );
    loop {
        let now = chrono::Utc::now();
        let wait = 60 - u64::from(chrono::Timelike::second(&now));
        tokio::time::sleep(Duration::from_secs(wait)).await;
        let pool = pool.clone();
        // Each minute runs on its own task, so a slow minute never delays the next (schedule:work).
        tokio::spawn(async move {
            let Some(lock) = AdvisoryLock::try_acquire(&pool, "tinyhelm-scheduler-minute").await
            else {
                return; // another scheduler instance runs this minute (onOneServer)
            };
            server_manager::tick(&pool, &app_key, chrono::Utc::now()).await;
            lock.release().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TINYHELM_LARAVEL_DIR=… TINYHELM_PHP=… cargo test --bin worker live_laravel -- --ignored`
    #[tokio::test]
    #[ignore = "needs a Laravel checkout with PHP"]
    async fn live_laravel_dispatch() {
        laravel_dispatch("echo 'ok';").await.unwrap();
        assert!(laravel_dispatch("throw new RuntimeException('x');")
            .await
            .is_err());
    }

    #[test]
    fn ownership_list() {
        assert!(owns_in("server-manager", "server-connection-check"));
        assert!(owns_in("server-manager", "server-storage-check"));
        assert!(owns_in(
            " server-connection-check ,x",
            "server-connection-check"
        ));
        assert!(!owns_in("server-connection-check", "server-storage-check"));
        assert!(!owns_in("", "server-connection-check"));
    }
}
