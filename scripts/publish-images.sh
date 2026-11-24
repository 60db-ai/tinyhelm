#!/usr/bin/env bash
# Build and push every TinyHelm image to GHCR from your own machine — no GitHub Actions needed.
# (GitHub Actions is only free for public repos; this path avoids it entirely.)
#
# Prereqs:
#   - docker (or kiri — it ships a Docker-compatible CLI + buildx) with buildx
#   - a GitHub Personal Access Token with `write:packages`, exported as GH_TOKEN
#
# Usage:
#   GH_TOKEN=ghp_xxx scripts/publish-images.sh [app|helper|sentinel|all]   # default: all
#
# Env overrides: REGISTRY (ghcr.io), OWNER (60db-ai), PLATFORMS (linux/amd64,linux/arm64),
#   GH_USER (github username for login), SENTINEL_REPO (https://github.com/60db-ai/sentinel.git).
set -euo pipefail
cd "$(dirname "$0")/.."

REGISTRY="${REGISTRY:-ghcr.io}"
OWNER="${OWNER:-60db-ai}"
PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"
TARGET="${1:-all}"

jqv() { # read a dotted path from a json file: jqv <file> <a.b.c>
  python3 -c "import json,sys
d=json.load(open(sys.argv[1]))
for k in sys.argv[2].split('.'): d=d[k]
print(d)" "$1" "$2"
}

APP_VERSION="$(jqv release.json stable.tinyhelm 2>/dev/null || echo latest)"
HELPER_VERSION="$(jqv upstream.json helper.version)"
SENTINEL_VERSION="$(jqv upstream.json sentinel.version)"
SENTINEL_REPO="${SENTINEL_REPO:-https://github.com/${OWNER}/sentinel.git}"

login() {
  : "${GH_TOKEN:?set GH_TOKEN to a PAT with write:packages}"
  echo "$GH_TOKEN" | docker login "$REGISTRY" -u "${GH_USER:-$OWNER}" --password-stdin
}

build() { # <context> <dockerfile> <image> <version> [extra build args...]
  local ctx="$1" file="$2" image="$3" ver="$4"; shift 4
  echo "==> $image:$ver"
  docker buildx build --platform "$PLATFORMS" \
    -f "$file" -t "$image:$ver" -t "$image:latest" \
    "$@" --push "$ctx"
}

publish_app() {
  build . docker/production/Dockerfile "$REGISTRY/$OWNER/tinyhelm" "$APP_VERSION" \
    --build-arg "TINYHELM_VERSION=$APP_VERSION"
}

publish_helper() {
  build docker/tinyhelm-helper docker/tinyhelm-helper/Dockerfile \
    "$REGISTRY/$OWNER/tinyhelm-helper" "$HELPER_VERSION"
}

publish_sentinel() {
  # Sentinel is a separate Rust project; build it from its own repo.
  local tmp; tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' RETURN
  git clone --depth 1 "$SENTINEL_REPO" "$tmp/sentinel"
  build "$tmp/sentinel" "$tmp/sentinel/Dockerfile" \
    "$REGISTRY/$OWNER/sentinel" "$SENTINEL_VERSION"
}

login
case "$TARGET" in
  app) publish_app ;;
  helper) publish_helper ;;
  sentinel) publish_sentinel ;;
  all) publish_app; publish_helper; publish_sentinel ;;
  *) echo "unknown target: $TARGET (app|helper|sentinel|all)" >&2; exit 1 ;;
esac
echo "Done. Make the packages public at https://github.com/orgs/${OWNER}/packages so servers can pull without auth."
