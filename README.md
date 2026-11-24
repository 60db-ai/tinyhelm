<p align="center">
  <img src="public/tinyhelm-logo.png" alt="TinyHelm" width="420" />
</p>

<p align="center">
  <b>Deploy · Manage · Scale</b> — an open-source, self-hostable PaaS.
</p>

<p align="center">
  <img src="https://img.shields.io/github/license/60db-ai/tinyhelm?style=flat-square" alt="License" />
  <img src="https://img.shields.io/github/last-commit/60db-ai/tinyhelm?style=flat-square" alt="Last commit" />
  <img src="https://img.shields.io/badge/runtime-Kiri-5a2a82?style=flat-square" alt="Runtime: Kiri" />
</p>

<br />

## What is TinyHelm?

TinyHelm is an open-source, self-hostable platform for deploying **any application, database, or service** on your own servers — an alternative to Heroku, Netlify, and Vercel.

It gives you the experience of a managed cloud platform while running on infrastructure you control: a VPS, a bare-metal server, a Raspberry Pi, or a spare Linux box. You only need an SSH connection.

Your configurations stay on your servers. If you stop using TinyHelm, your running resources keep working and stay manageable.

<br />

## Features

- **Any workload** — applications (Git, Dockerfile, Docker image, Compose), 9 standalone databases, and one-click service stacks.
- **Git-driven deploys** — push to deploy from GitHub, GitLab, Gitea, and Bitbucket, with pull-request preview environments.
- **Multi-server, multi-team** — manage a fleet of servers over SSH, scoped by team with fine-grained access grants.
- **Kiri runtime** — managed servers run on [Kiri](https://github.com/libracoder/kiri), a fast Rust container runtime that speaks the Docker Engine API (set `SERVER_CONTAINER_RUNTIME=docker` to use Docker instead).
- **Batteries included** — automatic HTTPS via Traefik, backups to S3, scheduled tasks, image vulnerability scanning, real-time logs and metrics, and a web terminal.
- **REST API + MCP** — a full `/api/v1` surface and a built-in MCP server for agents.

<br />

## Quick start

> Self-hosted install and update tooling is served from this repository's own release pipeline.
> See [`docs/`](docs/) for the current install instructions.

```bash
# Managed-server runtime defaults to Kiri; override per environment:
#   SERVER_CONTAINER_RUNTIME=docker
```

<br />

## Tech stack

- **Backend:** Laravel 12 (Laravel 10 file structure), Livewire 3, Horizon queues, Reverb WebSockets, Sanctum.
- **Runtime:** Kiri (default) or Docker on each managed server; Traefik reverse proxy.
- **Database:** PostgreSQL + Redis.
- A Rust + TypeScript rewrite is planned — see [`rust-migration/`](rust-migration/).

<br />

## Documentation

See [`docs/`](docs/) and [`AGENTS.md`](AGENTS.md) for architecture and development.

<br />

## Development

```bash
./scripts/dev start      # start the dev stack for this branch
php artisan test --compact
vendor/bin/pint --dirty --format agent
```

See [`AGENTS.md`](AGENTS.md) for the full guide.

<br />

## License

Licensed under the terms in [`LICENSE`](LICENSE).
