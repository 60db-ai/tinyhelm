# TinyHelm deploy engine (Rust) — design

The deploy engine is the hard core of the PHP→Rust migration. Until it exists in Rust, no
deploy/start/stop/restart can leave Laravel, and PHP cannot be removed. This is a large sub-project;
it is built incrementally, verified against the real DB at each step.

## Architecture

- **Binary:** `rust-backend` ships a second binary, `worker` (`src/bin/worker.rs`), run separately
  from the gateway. It is the Rust counterpart of Laravel's `ApplicationDeploymentJob` + Horizon.
- **Queue:** it consumes `application_deployment_queues` directly (not Redis/Horizon). A job is a row
  with `status = 'queued'`. The worker claims one atomically:
  `UPDATE ... SET status='in_progress' WHERE id = (SELECT id ... WHERE status='queued' ORDER BY id
  LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING ...` — safe for multiple workers, no double-run.
- **SSH:** shells out to the system `ssh` with a temp `0600` key file (matches Coolify). The server's
  private key is read from `private_keys.private_key` and **decrypted with the Laravel-compatible
  crypto** (`crypto.rs`) — same APP_KEY, so Rust and PHP share keys.
- **Status/logs:** the worker writes `status`, `finished_at`, and appends `logs` on the same row the
  UI/gateway reads. Terminal states: `finished` / `failed` (`cancelled-by-user` honored later).

## Status: skeleton done (verified live)

claim → in_progress → resolve server + decrypt key → SSH probe → finished/failed, logs persisted.
Proven against the real migrated `application_deployment_queues`.

## TODO (ordered) — porting `ApplicationDeploymentJob`

1. **Strategy dispatch** (mirror `decide_what_to_do`): dockerimage · dockerfile · compose · nixpacks ·
   railpack · static · PR-preview.
2. **Dockerimage path first** (simplest): pull image, generate compose, `docker compose up`, health check.
3. **Dockerfile/compose buildpacks:** clone repo (git over SSH), `generate_compose_file` (port the
   relevant parts of `parsers.php`), build in a helper container on the target, push to registry.
4. **Rolling update** + health checks + `GetContainersStatus` update.
5. **Log streaming** in Coolify's JSON-line format (so the existing UI tail works), not plain text.
6. **Broadcasting:** emit `ApplicationStatusChanged` so the SPA/Reverb updates live (or have the
   gateway poll — already does).
7. **Concurrency:** per-server in-flight limit + `queue_next_deployment` equivalent.
8. **Other jobs:** backups, scheduled tasks, docker cleanup, server checks — each a worker task.

## How to run (test)

```bash
cd rust-backend
APP_KEY=<laravel app key> DATABASE_URL=postgres://... cargo run --bin worker
```

Run it only in a test environment while Laravel's Horizon worker is also running — both consume the
same queue and would race. The cutover is: port a strategy, verify parity vs Laravel on a real
deploy, then route that path to the Rust worker exclusively.
