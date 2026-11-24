//! TinyHelm deploy worker — Phase 3, EXPERIMENTAL skeleton.
//!
//! Claims queued rows from `application_deployment_queues`, resolves the target server, decrypts its
//! SSH private key (Laravel-compatible), connects over SSH and runs a probe. This proves the engine
//! lifecycle end to end: claim -> in_progress -> connect -> finished/failed, with logs persisted.
//!
//! NOT a full deployment yet: the real buildpack / compose-generation / build / rolling-update steps
//! are TODO (see rust-migration/DEPLOY-ENGINE.md). Run it alongside (not instead of) the Laravel
//! Horizon worker only in a test environment, or it will race the PHP worker for the same queue.
//!
//! Env: DATABASE_URL (required), APP_KEY (to decrypt keys), WORKER_POLL_SECS (default 5).

#[path = "../crypto.rs"]
mod crypto;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let app_key = std::env::var("APP_KEY").ok().and_then(|k| crypto::parse_key(&k));
    let poll = Duration::from_secs(
        std::env::var("WORKER_POLL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(5),
    );
    let pool = PgPoolOptions::new().max_connections(5).connect(&url).await.expect("db connect");

    tracing::info!("TinyHelm deploy worker started (poll {}s)", poll.as_secs());
    loop {
        match claim_and_run(&pool, app_key.as_ref()).await {
            Ok(true) => {} // ran one; immediately look for the next
            Ok(false) => tokio::time::sleep(poll).await,
            Err(e) => {
                tracing::error!("worker loop error: {e}");
                tokio::time::sleep(poll).await;
            }
        }
    }
}

/// Claim at most one queued deployment and run it. Returns Ok(false) when nothing was queued.
async fn claim_and_run(pool: &PgPool, app_key: Option<&[u8; 32]>) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "UPDATE application_deployment_queues SET status='in_progress', deploy_started_at=now(), updated_at=now() \
         WHERE id = (SELECT id FROM application_deployment_queues WHERE status='queued' ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED) \
         RETURNING id, deployment_uuid, application_id, server_id",
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else { return Ok(false) };
    let id: i64 = row.get("id");
    let deployment_uuid: String = row.get("deployment_uuid");
    let server_id: Option<i32> = row.get("server_id");
    tracing::info!("claimed deployment {deployment_uuid} (id {id})");

    let (status, log) = match run_deployment(pool, app_key, server_id).await {
        Ok(l) => ("finished", l),
        Err(e) => ("failed", format!("[tinyhelm-worker] error: {e}")),
    };

    sqlx::query(
        "UPDATE application_deployment_queues SET status=$1, finished_at=now(), updated_at=now(), \
         logs = COALESCE(logs,'') || $2 WHERE id=$3",
    )
    .bind(status)
    .bind(format!("{log}\n"))
    .bind(id)
    .execute(pool)
    .await?;
    tracing::info!("deployment {deployment_uuid} -> {status}");
    Ok(true)
}

/// Resolve the server + key and run a probe over SSH. Placeholder for the real build/deploy steps.
async fn run_deployment(pool: &PgPool, app_key: Option<&[u8; 32]>, server_id: Option<i32>) -> Result<String, String> {
    let sid = server_id.ok_or("deployment has no server_id")?;
    let srow = sqlx::query(r#"SELECT ip, port, "user", private_key_id FROM servers WHERE id=$1"#)
        .bind(sid as i64)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("server not found")?;
    let ip: String = srow.get("ip");
    let port: i32 = srow.get("port");
    let user: String = srow.get("user");
    let pkid: Option<i64> = srow.get("private_key_id");
    let pkid = pkid.ok_or("server has no private key")?;

    let enc_key: String = sqlx::query_scalar("SELECT private_key FROM private_keys WHERE id=$1")
        .bind(pkid)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("private key not found")?;
    let app_key = app_key.ok_or("APP_KEY not configured (cannot decrypt key)")?;
    let private_key = crypto::decrypt(&enc_key, app_key).ok_or("private key decrypt failed")?;

    let probe = ssh_run(
        &private_key,
        &user,
        &ip,
        port,
        "docker version --format '{{.Server.Version}}' 2>/dev/null || docker --version",
    )
    .await?;
    // TODO: real steps — clone/pull, generate compose, build image, push, rolling update, health check.
    Ok(format!("[tinyhelm-worker] connected {user}@{ip}:{port}; container runtime: {probe}"))
}

/// Shell out to the system `ssh` (matches Coolify's approach) with a temp key file.
async fn ssh_run(private_key: &str, user: &str, ip: &str, port: i32, cmd: &str) -> Result<String, String> {
    let path = std::env::temp_dir().join(format!("th-worker-key-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        f.write_all(private_key.as_bytes()).map_err(|e| e.to_string())?;
        if !private_key.ends_with('\n') {
            let _ = f.write_all(b"\n");
        }
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();

    let out = tokio::process::Command::new("ssh")
        .args([
            "-i",
            path.to_str().ok_or("bad key path")?,
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "BatchMode=yes",
            "-p",
            &port.to_string(),
            &format!("{user}@{ip}"),
            cmd,
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!("ssh failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}
