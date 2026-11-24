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
    let app_key = std::env::var("APP_KEY")
        .ok()
        .and_then(|k| crypto::parse_key(&k));
    let poll = Duration::from_secs(
        std::env::var("WORKER_POLL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5),
    );
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("db connect");

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
    let application_id: String = row.get("application_id");
    tracing::info!("claimed deployment {deployment_uuid} (id {id})");

    let (status, log) = match run_deployment(pool, app_key, &application_id, server_id).await {
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

/// Resolve server + app, then dispatch to a deploy strategy. Only the dockerimage path is ported.
async fn run_deployment(
    pool: &PgPool,
    app_key: Option<&[u8; 32]>,
    application_id: &str,
    server_id: Option<i32>,
) -> Result<String, String> {
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
    let pkid: i64 = srow
        .try_get::<Option<i64>, _>("private_key_id")
        .ok()
        .flatten()
        .ok_or("server has no private key")?;

    let enc_key: String = sqlx::query_scalar("SELECT private_key FROM private_keys WHERE id=$1")
        .bind(pkid)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("private key not found")?;
    let app_key = app_key.ok_or("APP_KEY not configured (cannot decrypt key)")?;
    let private_key = crypto::decrypt(&enc_key, app_key).ok_or("private key decrypt failed")?;

    let arow = sqlx::query(
        r#"SELECT id, uuid, docker_registry_image_name, docker_registry_image_tag, ports_mappings,
                  custom_docker_run_options, destination_id, build_pack FROM applications
           WHERE id::text = $1 OR uuid = $1 LIMIT 1"#,
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("application not found")?;

    let app_db_id: i64 = arow.get("id");
    let app_uuid: String = arow.get("uuid");
    let image_name: Option<String> = arow
        .try_get::<Option<String>, _>("docker_registry_image_name")
        .ok()
        .flatten();
    let Some(image_name) = image_name.filter(|s| !s.is_empty()) else {
        let bp: Option<String> = arow.try_get("build_pack").ok().flatten();
        return Err(format!("only the dockerimage strategy is ported; build_pack={bp:?} / no image set — not yet supported"));
    };
    let tag: Option<String> = arow
        .try_get::<Option<String>, _>("docker_registry_image_tag")
        .ok()
        .flatten();
    let image = format!(
        "{}:{}",
        image_name,
        tag.filter(|s| !s.is_empty())
            .unwrap_or_else(|| "latest".into())
    );
    let ports_mappings: Option<String> = arow
        .try_get::<Option<String>, _>("ports_mappings")
        .ok()
        .flatten();
    let custom_opts: Option<String> = arow
        .try_get::<Option<String>, _>("custom_docker_run_options")
        .ok()
        .flatten();
    let dest_id: Option<i64> = arow
        .try_get::<Option<i64>, _>("destination_id")
        .ok()
        .flatten();

    let network = match dest_id {
        Some(d) => {
            sqlx::query_scalar::<_, String>("SELECT network FROM standalone_dockers WHERE id=$1")
                .bind(d)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
        }
        None => None,
    }
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "coolify".into());

    // Decrypted runtime env vars -> -e flags.
    let envs = fetch_app_envs(pool, app_key, app_db_id).await;
    let mut flags = String::new();
    for (k, v) in &envs {
        flags.push_str(&format!(" -e {}", shell_escape(&format!("{k}={v}"))));
    }
    if let Some(pm) = ports_mappings.filter(|s| !s.is_empty()) {
        for p in pm.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            flags.push_str(&format!(" -p {p}"));
        }
    }
    let custom = custom_opts
        .filter(|s| !s.is_empty())
        .map(|s| format!(" {s}"))
        .unwrap_or_default();

    // Simplified dockerimage deploy: ensure network, pull, replace container, run, report status.
    // TODO: Traefik/Caddy labels for domains, full compose generation, rolling update, health gating.
    let script = format!(
        "set -e\n\
         export PATH=\"$PATH:/usr/local/bin:/opt/homebrew/bin\"\n\
         docker network create {net} >/dev/null 2>&1 || true\n\
         echo '[pull] {img}'; docker pull {img}\n\
         echo '[replace]'; docker rm -f {name} >/dev/null 2>&1 || true\n\
         echo '[run]'; docker run -d --name {name} --network {net} --restart unless-stopped{flags}{custom} {img}\n\
         echo '[status]'; docker inspect -f '{{{{.State.Status}}}}' {name}",
        net = network,
        img = image,
        name = app_uuid,
        flags = flags,
        custom = custom,
    );
    let out = ssh_run(&private_key, &user, &ip, port, &script).await?;
    Ok(format!(
        "[tinyhelm-worker] dockerimage deploy of {app_uuid} ({image}) on {user}@{ip}:{port}\n{out}"
    ))
}

/// Fetch an application's env vars, decrypted to plaintext.
async fn fetch_app_envs(
    pool: &PgPool,
    app_key: &[u8; 32],
    app_db_id: i64,
) -> Vec<(String, String)> {
    let rows = sqlx::query(
        r"SELECT key, value FROM environment_variables WHERE resourceable_type='App\Models\Application' AND resourceable_id=$1",
    )
    .bind(app_db_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|r| {
            let k: String = r.get("key");
            let v: Option<String> = r.try_get("value").ok().flatten();
            let plain = v
                .and_then(|e| crypto::decrypt(&e, app_key))
                .unwrap_or_default();
            (k, plain)
        })
        .collect()
}

/// Single-quote a value for safe use in a POSIX shell command.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Shell out to the system `ssh` (matches Coolify's approach) with a temp key file.
async fn ssh_run(
    private_key: &str,
    user: &str,
    ip: &str,
    port: i32,
    cmd: &str,
) -> Result<String, String> {
    let path = std::env::temp_dir().join(format!("th-worker-key-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        f.write_all(private_key.as_bytes())
            .map_err(|e| e.to_string())?;
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
        Err(format!(
            "ssh failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
