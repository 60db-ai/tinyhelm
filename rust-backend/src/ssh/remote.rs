//! Port of `instant_remote_process` (bootstrap/helpers/remoteProcess.php) and the SSH command builder
//! of `SshMultiplexingHelper`: run a command list on a managed server over the system `ssh` client,
//! with the same options, non-root sudo rewrite, host-key modes, multiplexing, retries and errors.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use tokio::io::AsyncWriteExt;

use super::sudo::parse_commands_by_line_for_sudo;

/// sshd runs this with the user's login shell (maybe fish/csh), so the script runs in bash or sh.
const REMOTE_SHELL: &str =
    r#"sh -c "if command -v bash >/dev/null 2>&1; then exec bash -se; else exec sh -se; fi""#;
pub const HOST_KEY_CHANGED_MESSAGE: &str = "The SSH host key of this server changed, so TinyHelm did not connect. An admin must check the new key on the Security page of the server.";

const RETRYABLE: &[&str] = &[
    "kex_exchange_identification",
    "connection reset by peer",
    "connection refused",
    "connection timed out",
    "connection closed by remote host",
    "ssh_exchange_identification",
    "bad file descriptor",
    "broken pipe",
    "no route to host",
    "network is unreachable",
    "host is down",
    "no buffer space available",
    "connection reset by",
    "permission denied, please try again",
    "received disconnect from",
    "disconnected from",
    "connection to .* closed",
    "ssh: connect to host .* port .*: connection",
    "lost connection",
    "timeout, server not responding",
    "cannot assign requested address",
    "network is down",
    "operation timed out",
    "connection closed unexpectedly",
    "remote host closed connection",
    "authentication failed",
    "too many authentication failures",
    "ssh command failed with exit code: 255",
];

/// Everything needed to reach a server, loaded with [`ServerConnection::load`].
#[derive(Clone)]
pub struct ServerConnection {
    pub uuid: String,
    pub ip: String,
    pub port: i32,
    pub user: String,
    key_uuid: String,
    private_key: String,
    connection_timeout: u64,
    force_disabled: bool,
    cloudflare_tunnel: bool,
    pinned_host_key: Option<String>,
    host_key_check_disabled: bool,
    host_key_check_enforced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteError {
    pub message: String,
    pub exit_code: Option<i32>,
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RemoteError {}

fn err(message: impl Into<String>, exit_code: Option<i32>) -> RemoteError {
    RemoteError {
        message: message.into(),
        exit_code,
    }
}

fn opt_bool(row: &sqlx::postgres::PgRow, col: &str) -> bool {
    row.try_get::<Option<bool>, _>(col)
        .ok()
        .flatten()
        .unwrap_or(false)
}

impl ServerConnection {
    /// Load the server, its settings and its decrypted private key (the key must belong to the server's team).
    pub async fn load(
        pool: &PgPool,
        app_key: &[u8; 32],
        server_id: i64,
    ) -> Result<Self, RemoteError> {
        let row = sqlx::query(
            r#"SELECT s.uuid, s.ip, s.port, s."user", s.team_id, s.private_key_id,
                      ss.connection_timeout, ss.force_disabled, ss.is_cloudflare_tunnel, ss.ssh_host_key,
                      ss.is_ssh_host_key_check_disabled, ss.is_ssh_host_key_check_enforced
               FROM servers s LEFT JOIN server_settings ss ON ss.server_id = s.id WHERE s.id = $1"#,
        )
        .bind(server_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| err(format!("server lookup failed: {e}"), None))?
        .ok_or_else(|| err("Server not found.", None))?;

        let team_id: i64 = row
            .try_get("team_id")
            .map_err(|e| err(e.to_string(), None))?;
        let key_id: Option<i64> = row.try_get("private_key_id").ok().flatten();
        let key = sqlx::query(
            "SELECT uuid, private_key FROM private_keys WHERE id = $1 AND team_id = $2",
        )
        .bind(key_id.ok_or_else(|| err("Server has no private key.", None))?)
        .bind(team_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| err(format!("private key lookup failed: {e}"), None))?
        .ok_or_else(|| {
            err(
                "No query results for model [App\\Models\\PrivateKey].",
                None,
            )
        })?;
        let encrypted: String = key
            .try_get("private_key")
            .map_err(|e| err(e.to_string(), None))?;
        let private_key = crate::crypto::decrypt(&encrypted, app_key)
            .ok_or_else(|| err("The private key could not be decrypted.", None))?;

        let timeout: Option<i32> = row.try_get("connection_timeout").ok().flatten();
        Ok(Self {
            uuid: row.try_get("uuid").map_err(|e| err(e.to_string(), None))?,
            ip: row.try_get("ip").map_err(|e| err(e.to_string(), None))?,
            port: row.try_get("port").unwrap_or(22),
            user: row.try_get("user").unwrap_or_else(|_| "root".into()),
            key_uuid: key.try_get("uuid").map_err(|e| err(e.to_string(), None))?,
            private_key,
            connection_timeout: timeout.filter(|t| *t > 0).map_or(10, |t| t as u64),
            force_disabled: opt_bool(&row, "force_disabled"),
            cloudflare_tunnel: opt_bool(&row, "is_cloudflare_tunnel"),
            pinned_host_key: row
                .try_get::<Option<String>, _>("ssh_host_key")
                .ok()
                .flatten()
                .filter(|k| !k.trim().is_empty()),
            host_key_check_disabled: opt_bool(&row, "is_ssh_host_key_check_disabled"),
            host_key_check_enforced: opt_bool(&row, "is_ssh_host_key_check_enforced"),
        })
    }

    pub fn is_non_root(&self) -> bool {
        self.user != "root"
    }

    /// warn: connect also when the host key changed; enforce: refuse a changed key; off: no check.
    fn host_key_mode(&self) -> &'static str {
        let mode = std::env::var("SSH_HOST_KEY_CHECKING")
            .map(|m| m.trim().to_lowercase())
            .unwrap_or_else(|_| "warn".into());
        if mode == "off" || self.host_key_check_disabled {
            "off"
        } else if mode == "enforce" || self.host_key_check_enforced {
            "enforce"
        } else {
            "warn"
        }
    }
}

/// Options for one remote call (defaults match `instant_remote_process`).
#[derive(Clone, Copy, Default)]
pub struct RunOptions {
    pub no_sudo: bool,
    /// Seconds; `SSH_COMMAND_TIMEOUT` (default 3600) when None.
    pub timeout: Option<u64>,
    pub disable_multiplexing: bool,
    pub max_attempts: Option<u32>,
}

/// `instant_remote_process`: run the commands (one per line) and return trimmed stdout
/// (`None` for empty output or the literal `null`, like PHP).
pub async fn instant_remote_process(
    server: &ServerConnection,
    commands: &[String],
    opts: RunOptions,
) -> Result<Option<String>, RemoteError> {
    let commands = if server.is_non_root() && !opts.no_sudo {
        parse_commands_by_line_for_sudo(commands, &server.user)
    } else {
        commands.to_vec()
    };
    let script = commands.join("\n");
    let timeout = opts.timeout.unwrap_or_else(command_timeout);
    let max_attempts = opts
        .max_attempts
        .unwrap_or_else(|| env_u32("SSH_MAX_RETRIES", 3));

    let mut attempt: u32 = 0;
    loop {
        match run_once(server, &script, timeout, opts.disable_multiplexing).await {
            Ok(out) => return Ok(out),
            Err(e) => {
                let limit = attempt_limit(&e, max_attempts, opts.max_attempts.is_some());
                if is_retryable(&e) && attempt + 1 < limit {
                    tokio::time::sleep(retry_delay(attempt)).await;
                    attempt += 1;
                    continue;
                }
                if e.message.trim().is_empty() {
                    let after = if attempt > 0 {
                        format!(" after {} attempts", attempt + 1)
                    } else {
                        String::new()
                    };
                    return Err(err(
                        format!("SSH connection failed to server {}{after}", server.ip),
                        e.exit_code,
                    ));
                }
                return Err(e);
            }
        }
    }
}

fn ssh_args(server: &ServerConnection, disable_mux: bool) -> Result<Vec<String>, RemoteError> {
    let key_path = ensure_key_file(server)?;
    let mut args: Vec<String> = Vec::new();
    let opt = |args: &mut Vec<String>, value: String| args.extend(["-o".to_string(), value]);
    if !disable_mux && env_bool("MUX_ENABLED", true) {
        let socket = runtime_dir("mux")?.join(format!("mux_{}", server.uuid));
        opt(&mut args, "ControlMaster=auto".into());
        opt(&mut args, format!("ControlPath={}", socket.display()));
        opt(
            &mut args,
            format!("ControlPersist={}", env_u32("SSH_MUX_PERSIST_TIME", 3600)),
        );
    }
    if server.cloudflare_tunnel {
        opt(
            &mut args,
            "ProxyCommand=cloudflared access ssh --hostname %h".into(),
        );
    }
    args.extend(["-i".into(), key_path.display().to_string()]);
    args.extend(host_key_args(server)?);
    opt(&mut args, "PasswordAuthentication=no".into());
    opt(
        &mut args,
        format!("ConnectTimeout={}", server.connection_timeout),
    );
    opt(&mut args, "ServerAliveInterval=20".into());
    args.extend(["-F".into(), "/dev/null".into()]);
    for value in [
        "IdentitiesOnly=yes",
        "ClearAllForwardings=yes",
        "ForwardAgent=no",
        "RequestTTY=no",
        "LogLevel=ERROR",
    ] {
        opt(&mut args, value.into());
    }
    args.extend([
        "-p".into(),
        server.port.to_string(),
        "--".into(),
        format!("{}@{}", server.user, server.ip),
        REMOTE_SHELL.into(),
    ]);
    Ok(args)
}

async fn run_once(
    server: &ServerConnection,
    script: &str,
    timeout: u64,
    disable_mux: bool,
) -> Result<Option<String>, RemoteError> {
    if server.force_disabled {
        return Err(err("Server is disabled.", None));
    }
    if server.user.starts_with('-') || server.ip.starts_with('-') {
        return Err(err(
            "The SSH user and the server IP must not start with \"-\".",
            None,
        ));
    }
    let mut child = tokio::process::Command::new("ssh")
        .args(ssh_args(server, disable_mux)?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| err(format!("could not start ssh: {e}"), None))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{script}\n").as_bytes())
            .await
            .map_err(|e| err(format!("could not send the command to ssh: {e}"), None))?;
    }
    let output =
        match tokio::time::timeout(Duration::from_secs(timeout), child.wait_with_output()).await {
            Ok(result) => result.map_err(|e| err(format!("ssh failed: {e}"), None))?,
            Err(_) => {
                return Err(err(
                    format!("SSH command failed with exit code: 124 (timed out after {timeout}s)"),
                    Some(124),
                ))
            }
        };

    let code = output.status.code();
    if code != Some(0) {
        return Err(command_error(
            &String::from_utf8_lossy(&output.stderr),
            code,
        ));
    }
    let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if out.is_empty() || out == "null" {
        None
    } else {
        Some(out)
    })
}

/// `excludeCertainErrors`.
fn command_error(stderr: &str, code: Option<i32>) -> RemoteError {
    let fallback = format!("SSH command failed with exit code: {}", code.unwrap_or(-1));
    let mut message = stderr.trim().to_string();
    if message.is_empty() {
        message = fallback.clone();
    }
    if code == Some(255) && message.contains("Host key verification failed") {
        message = HOST_KEY_CHANGED_MESSAGE.into();
    }
    let message = strip_host_key_changed_warning(&message);
    let message = message.trim();
    err(
        if message.is_empty() {
            fallback
        } else {
            message.to_string()
        },
        code,
    )
}

/// Drop OpenSSH's leading "REMOTE HOST IDENTIFICATION HAS CHANGED" banner (warn mode still connects).
fn strip_host_key_changed_warning(s: &str) -> String {
    if !s.starts_with('@') || !s.contains("REMOTE HOST IDENTIFICATION HAS CHANGED") {
        return s.to_string();
    }
    // The banner ends with the "...key for <host> has changed and you have requested strict checking."
    // or "Agent forwarding is disabled ..." lines; everything after the last banner line is the real error.
    let lines: Vec<&str> = s.lines().collect();
    let banner_end = lines
        .iter()
        .rposition(|l| {
            l.starts_with('@')
                || l.contains("IT IS POSSIBLE THAT SOMEONE")
                || l.contains("Someone could be eavesdropping")
                || l.contains("It is also possible that a host key")
                || l.contains("The fingerprint for the")
                || l.trim_start().starts_with("SHA256:")
                || l.contains("Please contact your system administrator")
                || l.contains("Add correct host key in")
                || l.contains("Offending ")
                || l.contains("is disabled to avoid man-in-the-middle attacks")
                || l.contains("UpdateHostkeys is disabled")
        })
        .map_or(0, |i| i + 1);
    lines[banner_end..].join("\n")
}

fn host_key_args(server: &ServerConnection) -> Result<Vec<String>, RemoteError> {
    let mode = server.host_key_mode();
    let insecure = || {
        vec![
            "-o".into(),
            "StrictHostKeyChecking=no".into(),
            "-o".into(),
            "UserKnownHostsFile=/dev/null".into(),
        ]
    };
    if mode == "off" {
        return Ok(insecure());
    }
    let Some(pinned) = &server.pinned_host_key else {
        if mode == "enforce" {
            return Err(err(
                "SSH connection failed: TinyHelm could not read the SSH host key of the server.",
                Some(255),
            ));
        }
        // ponytail: Laravel learns and pins the key here (ssh-keyscan); Rust connects unpinned until that is ported.
        return Ok(insecure());
    };
    let alias = format!("tinyhelm-{}", server.uuid);
    let content: String = pinned
        .split('\n')
        .filter(|k| !k.is_empty())
        .map(|k| format!("{alias} {k}\n"))
        .collect();
    let hash = hex::encode(Sha256::digest(content.as_bytes()));
    let path =
        runtime_dir("known_hosts")?.join(format!("known_hosts_{}_{}", server.uuid, &hash[..16]));
    write_private_file(&path, &content)?;
    Ok(vec![
        "-o".into(),
        format!("HostKeyAlias={alias}"),
        "-o".into(),
        format!("UserKnownHostsFile={}", path.display()),
        "-o".into(),
        format!(
            "StrictHostKeyChecking={}",
            if mode == "enforce" { "yes" } else { "no" }
        ),
    ])
}

/// `PrivateKey::getKeyLocation()`: `ssh_key@<uuid>`, mode 0600, rewritten when the key changes.
fn ensure_key_file(server: &ServerConnection) -> Result<PathBuf, RemoteError> {
    let path = runtime_dir("keys")?.join(format!("ssh_key@{}", server.key_uuid));
    let mut content = server.private_key.clone();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    write_private_file(&path, &content)?;
    Ok(path)
}

fn write_private_file(path: &std::path::Path, content: &str) -> Result<(), RemoteError> {
    use std::os::unix::fs::PermissionsExt;
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(());
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, content)
        .map_err(|e| err(format!("could not write {}: {e}", tmp.display()), None))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| err(e.to_string(), None))?;
    std::fs::rename(&tmp, path).map_err(|e| err(e.to_string(), None))
}

/// `$TINYHELM_SSH_DIR/<name>` (default `<tmp>/tinyhelm-ssh`), created with mode 0700.
fn runtime_dir(name: &str) -> Result<PathBuf, RemoteError> {
    use std::os::unix::fs::PermissionsExt;
    let base = std::env::var("TINYHELM_SSH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("tinyhelm-ssh"));
    let dir = base.join(name);
    std::fs::create_dir_all(&dir)
        .map_err(|e| err(format!("could not create {}: {e}", dir.display()), None))?;
    let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    Ok(dir)
}

fn is_retryable(e: &RemoteError) -> bool {
    if matches!(e.exit_code, Some(c) if c > 0 && c != 255) {
        return false;
    }
    let lower = e.message.to_lowercase();
    RETRYABLE.iter().any(|p| lower.contains(p))
}

fn attempt_limit(e: &RemoteError, max: u32, explicit: bool) -> u32 {
    if explicit {
        return max;
    }
    if e.message.contains("kex_exchange_identification")
        || e.message.contains("ssh_exchange_identification")
    {
        return max.max(env_u32("SSH_HANDSHAKE_MAX_RETRIES", 5));
    }
    max
}

/// `min(base * multiplier^attempt, max)` seconds.
fn retry_delay(attempt: u32) -> Duration {
    let base = u64::from(env_u32("SSH_RETRY_BASE_DELAY", 2));
    let max = u64::from(env_u32("SSH_RETRY_MAX_DELAY", 30));
    let mult = u64::from(env_u32("SSH_RETRY_MULTIPLIER", 2));
    Duration::from_secs(base.saturating_mul(mult.saturating_pow(attempt)).min(max))
}

fn command_timeout() -> u64 {
    std::env::var("SSH_COMMAND_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|t| *t > 0)
        .unwrap_or(3600)
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name).ok().map_or(default, |v| {
        !matches!(
            v.trim().to_lowercase().as_str(),
            "false" | "0" | "off" | "no" | ""
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_matches_laravel_rules() {
        assert!(is_retryable(&err(
            "ssh: connect to host 1.2.3.4 port 22: Connection refused",
            Some(255)
        )));
        assert!(
            !is_retryable(&err("Connection refused", Some(1))),
            "non-255 exit codes are command errors"
        );
        assert!(!is_retryable(&err("docker: command not found", Some(255))));
    }

    #[test]
    fn retry_delay_is_exponential_and_capped() {
        assert_eq!(retry_delay(0), Duration::from_secs(2));
        assert_eq!(retry_delay(1), Duration::from_secs(4));
        assert_eq!(retry_delay(10), Duration::from_secs(30));
    }

    #[test]
    fn host_key_failure_message() {
        assert_eq!(
            command_error("Host key verification failed.", Some(255)).message,
            HOST_KEY_CHANGED_MESSAGE
        );
        assert_eq!(
            command_error("", Some(3)).message,
            "SSH command failed with exit code: 3"
        );
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// `TEST_DATABASE_URL=... TEST_APP_KEY=... TEST_SERVER_ID=1 cargo test --bin tinyhelm-gateway live_ -- --ignored`
    #[tokio::test]
    #[ignore = "needs a reachable server, the TinyHelm database and APP_KEY"]
    async fn live_instant_remote_process() {
        let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let key = crate::crypto::parse_key(&std::env::var("TEST_APP_KEY").unwrap()).unwrap();
        let id: i64 = std::env::var("TEST_SERVER_ID").unwrap().parse().unwrap();
        let server = ServerConnection::load(&pool, &key, id).await.unwrap();
        let out = instant_remote_process(
            &server,
            &["echo tinyhelm-ok".to_string()],
            RunOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(out.as_deref(), Some("tinyhelm-ok"));
        // no_sudo: this host's SSH user may lack passwordless sudo; exit codes must pass through unchanged.
        let opts = RunOptions {
            no_sudo: true,
            ..RunOptions::default()
        };
        let failed = instant_remote_process(&server, &["exit 3".to_string()], opts).await;
        assert_eq!(failed.unwrap_err().exit_code, Some(3));
    }
}
