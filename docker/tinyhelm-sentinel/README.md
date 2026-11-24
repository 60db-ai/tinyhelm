# TinyHelm sentinel image

Sentinel is TinyHelm's per-server metrics/traffic agent. It is a **separate Rust project**, so there is
no Dockerfile to vendor here — it builds from its own repository.

Published as `ghcr.io/60db-ai/sentinel:<version>` (version pinned in `upstream.json` → `sentinel.version`).
`app/Actions/Server/StartSentinel.php` pulls and runs this image on each managed server.

## One-time setup
1. Fork the upstream sentinel repo to your org:
   `gh repo fork coollabsio/sentinel --org 60db-ai`
2. Publish it (pick one):
   - **No GitHub Actions:** `GH_TOKEN=<pat> scripts/publish-images.sh sentinel`
   - **GitHub Actions (free on public repos):** Actions → *Publish sentinel image* → Run workflow
3. Make the package public: https://github.com/orgs/60db-ai/packages

## Notes
- Sentinel is optional — without the image, deployments still work; only metrics/traffic monitoring is affected.
- Keep `upstream.json` `sentinel.version` in sync with the tag you publish.
