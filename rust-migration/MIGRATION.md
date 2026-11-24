# TinyHelm → all-Rust + shadcn migration

Goal: replace the Laravel/Livewire/Blade stack with a **Rust backend (axum)** + **TypeScript/React SPA (shadcn/ui)**,
incrementally, without breaking the running product. UI quality bar: **ElevenLabs-grade** — light-first
surfaces, high-contrast type, generous spacing, thin borders, rounded cards, one restrained accent (TinyHelm
purple/orange), subtle motion.

## Strategy: strangler

The Rust gateway (`rust-backend/`) already proxies `/api/*` to Laravel. We port **page by page**: build the SPA
page, serve its data from the Rust gateway (a new route added *before* the proxy fallback), and once a Livewire
page has a shadcn replacement, stop linking to the Blade one. Blade is deleted only when nothing references it.

## Design direction (ElevenLabs-like)

- Base: neutral near-black (`oklch(~14%)`), elevated surfaces one step lighter, hairline borders.
- Type: Inter/Geist, tight headings, muted secondary text, tabular numbers for metrics.
- Accent: TinyHelm purple (primary) + orange (highlight), used sparingly.
- Layout: fixed left sidebar + top bar + content; rounded-xl cards; 8px spacing scale.
- Motion: 150–250ms ease-out on hover/enter; no gratuitous animation.
- Charts: evilcharts/Recharts, gradient fills, minimal gridlines.

## Port plan (step by step)

Scale at the start of the port: 349 API routes (434 route+method pairs, see [API-LEDGER.md](API-LEDGER.md)),
84 jobs, 110 actions, 291 Livewire components. Responses stay Laravel-compatible (same paths, fields, status
codes) so existing API clients, the CLI and MCP keep working.

Each step: port, run `scripts/parity/compare.py` until every role matches, regenerate the ledger with
`scripts/parity/ledger.py`, push to `main`.

1. [x] **Foundations.** `rust-backend/src/api/` split by domain; `native_guard` replicates the API middleware
   (Sanctum token lookup, `api.token.team`, API enabled, abilities with root bypass, `read:sensitive`,
   throttle, audit events). Requests it cannot replicate yet (members with access grants, scoped tokens,
   IP allow-lists) are forwarded to Laravel. Parity fixtures (`scripts/parity/fixtures.php`) and harness.
2. [x] **Full parity for the 33 native routes** (payload shapes, sensitive masking, validation, audit events on
   writes): `compare.py` 119/119 across 8 roles. Rare cases are delegated to Laravel and marked `ponytail:`.
3. [x] **DB-only controllers**: notifications, team, instance (version/health/realtime/disable/MCP),
   security keys, cloud-init scripts, cloud tokens, integration tokens, S3 storages, git apps, project
   environments, project/environment/server shared variables, tag update and resource tags, resources,
   scheduled tasks (application), audit events, deployments, destinations, storages, resource name/description
   updates, service parts reads. Requests needing external calls, SSH, or unported serializers are forwarded
   to Laravel (marked `ponytail:`). Parity 633/633 across all case files; failed-request limiter ported.
4. [~] **SSH module** (done: `rust-backend/src/ssh`, sudo rewrite verified on 4128 PHP golden checks, live test
   passes) and the actions
   on it: start/stop/restart, logs, server validate, proxy, docker cleanup, registries.
5. [~] **Status jobs and scheduler** (WIP branch `wip/rust-step5-jobs`, unverified) (ServerCheck, container status, backups, scheduled tasks, cleanup);
   Postgres-backed queue replaces Horizon.
6. [~] **Deploy engine** (compose parser foundation merged: 3/12 golden cases): all build packs, compose parser (oracle: `tests/Fixtures/parser/expected/`),
   proxy labels, previews, rollback.
7. [ ] **Git webhooks and provider APIs** (GitHub/GitLab/Gitea/Bitbucket, Hetzner/DigitalOcean/Vultr/Hostinger, DNS).
8. [ ] **Access grants** (`AccessResolver`) so members and scoped tokens stop delegating to Laravel.
9. [ ] **Web auth**: login, sessions, 2FA, OAuth, password reset, invitations, API token management.
10. [ ] **Realtime** (Pusher protocol server replacing Reverb), terminal WebSocket, MCP server.
11. [ ] **Schema ownership** (sqlx migrations from the current schema), then remove Laravel.

### Livewire-only features (no REST route; must be ported as new API + SPA)
- API token create/revoke/pause, team rename, invitations and member roles, integration setup flows
  (GitHub App, GitLab, S3, cloud providers), instance settings, onboarding, subscription/billing.

## Progress log

### 2026-11-24 — paused at the user's request

All agents, gateways, Laravel, Vite and test containers were stopped. State at the pause:

- **Native API:** 132 of 358 `/api` route+method pairs (see [API-LEDGER.md](API-LEDGER.md)). Parity suite:
  633/633 cases across 8 token roles (`scripts/parity/compare.py` with every file in `scripts/parity/cases/`).
- **Done:** steps 1–3 (foundations, full parity of the first 33 routes, all DB-only controllers), the SSH module
  of step 4, `ThrottleFailedApiRequests`, the SPA redesign (shadcn, every page).
- **Parked on branches (not merged, review first):**
  - `wip/rust-step5-jobs` — job scheduler (cron, advisory locks), ServerManagerJob and ServerConnectionCheckJob
    ports, Pusher HTTP client, server-job command golden script, `TINYHELM_RUST_JOBS` gate. Builds; not verified
    end to end.
- **Started, nothing saved:** step 4 actions (database/service lifecycle, logs, server actions), the rest of
  the compose parser (`serviceParser` main loop, `applicationParser`). Restart these from the plan above.
- **Still forwarded to Laravel inside native routes** (search `ponytail:` in `rust-backend/src`): requests that
  need external calls (DNS/private-network checks, S3, cloud providers), SSH work, compose re-parsing, the
  `serializeApiResource` for every resource type, databases with backup configs, members and scoped tokens
  (access grants, step 8), IP allow-lists.
- **Known gaps:** rate-limit and failed-request counters are per process (Laravel keeps its own); Rust renames
  do not clear Laravel's global-search cache; the deploy worker writes plain-text logs (Laravel stores a JSON
  array); team notifications (`ApiTokenFailingNotification`, reachability) are not ported.

### How to resume locally

```bash
# Laravel (8 workers: parity runs are concurrent)
PHP_CLI_SERVER_WORKERS=8 php artisan serve --host=127.0.0.1 --port=8000
# Rust gateway, every ported group
cd rust-backend && APP_KEY="$(grep '^APP_KEY=' ../.env | cut -d= -f2-)" RUST_NATIVE_ROUTES=all \
  API_RATE_LIMIT=100000 DATABASE_URL=postgres://postgres:postgres@127.0.0.1/coolify_app \
  TINYHELM_LISTEN=127.0.0.1:8080 TINYHELM_API_ORIGIN=http://127.0.0.1:8000 cargo run
# SPA
cd frontend && npm run dev -- --host 127.0.0.1 --port 5173
# Parity: fixtures once, then the suite; regenerate the ledger after merging
php artisan tinker --execute 'require base_path("scripts/parity/fixtures.php");'
python3 -I scripts/parity/compare.py
php artisan route:list --path=api --json > /tmp/routes.json && python3 -I scripts/parity/ledger.py /tmp/routes.json
```

The dev token for the SPA sign-in is the `TINYHELM_DEV_API_TOKEN` line in `.env` (gitignored). `.env` also has
`API_RATE_LIMIT=100000` for parity runs; remove it outside development.

## Rules
- Never remove a Blade page until its SPA replacement is live and linked.
- Every ported endpoint: `python3 -I scripts/parity/compare.py` must show every role matching.
- Keep the app shippable at every step.
