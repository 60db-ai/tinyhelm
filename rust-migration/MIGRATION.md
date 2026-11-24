# TinyHelm → all-Rust + shadcn migration

Goal: replace the Laravel/Livewire/Blade stack with a **Rust backend (axum)** + **TypeScript/React SPA (shadcn/ui)**,
incrementally, without breaking the running product. UI quality bar: **ElevenLabs-grade** — neutral near-black
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

## TODO (ordered)

### Phase 0 — foundation  (done / in progress)
- [x] Rust axum gateway serving SPA + proxying `/api`.
- [x] Vite + React + TS + Tailwind v4 + shadcn Button + chart scaffold.
- [ ] ElevenLabs-grade design tokens + app shell (sidebar + topbar).
- [ ] Typed API client (`src/lib/api.ts`) using a Sanctum token.
- [ ] Auth: login page → Sanctum token, stored + attached to API calls.

### Phase 1 — read-only pages (SPA on top of existing API)
- [ ] Dashboard (servers/resources/deployments counts + metrics).
- [ ] Servers list + server detail (status, resources).
- [ ] Projects → environments → resources browser.
- [ ] Application/Database/Service detail (read: config, env, logs, deployments).
- [ ] Deploy canvas parity (later).

### Phase 2 — actions (port write endpoints to Rust gateway)
- [ ] Deploy / start / stop / restart (wire to existing `/api/v1` first, then port the endpoint to Rust).
- [ ] Env var CRUD, domains, scheduled tasks, backups.
- [ ] Real-time status via WebSocket (replace Livewire/Reverb consumption with a Rust WS or proxy Reverb).

### Phase 3 — port backend to Rust, retire Laravel
- [ ] Port read endpoints into axum (sqlx against the same Postgres) behind the gateway, one resource at a time.
- [ ] Port the deploy/SSH worker to Rust (russh + bollard) — highest-value, most Rust-fit.
- [ ] Port auth (Sanctum-compatible tokens), policies/access grants, queues.
- [ ] Port webhooks (git + stripe), MCP.
- [ ] Delete Livewire/Blade once no route serves them; drop PHP.

### Phase 4 — polish
- [ ] Accessibility, responsive, dark/light, loading/empty/error states.
- [ ] E2E (Playwright), visual regression.

## Rules
- Never remove a Blade page until its SPA replacement is live and linked.
- Every ported endpoint: diff behavior against the Laravel one (rust-anything `record`/`diff` where it fits).
- Keep the app shippable at every step.
