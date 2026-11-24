"""Parity cases: security keys, integration tokens, cloud tokens, cloud-init scripts, S3 storages, git apps.

Fixtures are Laravel-made (tinker) and removed at exit. Set PARITY_PHP (php binary, default `php`) and
PARITY_PHP_ROOT (a Laravel checkout with vendor/, default the repository root).
"""

from __future__ import annotations

import atexit
import os
import subprocess
import tempfile
import time
from pathlib import Path

from compare import READ_ROLES, VOLATILE_KEYS, Case, call, compare

ROOT = Path(__file__).resolve().parents[3]
PHP = os.environ.get("PARITY_PHP", "php")
PHP_ROOT = os.environ.get("PARITY_PHP_ROOT", str(ROOT))

FIXTURES = r"""
use App\Models\{CloudInitScript, CloudProviderToken, GiteaApp, GithubApp, GitlabApp, IntegrationToken, PrivateKey, S3Storage, User};
$outsider = User::where('email', 'parity-outsider@tinyhelm.test')->firstOrFail()->teams()->wherePivot('role', 'owner')->firstOrFail()->id;
$key = PrivateKey::query()->where('team_id', 1)->where('name', 'not like', 'parity-%')->firstOrFail();
foreach (['a', 'b'] as $s) {
    CloudProviderToken::create(['team_id' => 1, 'provider' => 'hetzner', 'token' => 'parity-secret', 'name' => "parity-fx-cloud-$s"]);
    IntegrationToken::create(['team_id' => 1, 'provider' => 'infisical', 'name' => "parity-fx-inf-$s", 'token' => 'x', 'capabilities' => ['secrets']]);
    S3Storage::create(['team_id' => 1, 'name' => "parity-fx-s3-$s", 'region' => 'us-east-1', 'key' => 'AKIA', 'secret' => 'shh', 'bucket' => 'b', 'endpoint' => 'https://s3.example.com', 'type' => 's3']);
    GitlabApp::create(['team_id' => 1, 'name' => "parity-fx-gl-$s", 'api_url' => 'https://gitlab.com/api/v4', 'html_url' => 'https://gitlab.com', 'custom_port' => 22, 'custom_user' => 'git', 'app_secret' => 'a', 'webhook_token' => 'w', 'client_secret' => 'c', 'access_token' => 't', 'refresh_token' => 'r', 'expires_at' => 1]);
}
CloudInitScript::create(['team_id' => 1, 'name' => 'parity-fx-bash', 'script' => "#!/bin/bash\necho hi"]);
CloudInitScript::create(['team_id' => 1, 'name' => 'parity-fx-yaml', 'script' => "#cloud-config\npackages:\n  - curl"]);
IntegrationToken::create(['team_id' => 1, 'provider' => 'cloudflare', 'name' => 'parity-fx-cf', 'token' => 'x', 'capabilities' => ['dns'], 'metadata' => ['automatic_dns' => false, 'zones_synced_at' => '2026-01-01T00:00:00Z']]);
S3Storage::create(['team_id' => 1, 'name' => 'parity-fx-sftp', 'region' => 'us-east-1', 'key' => 'u', 'secret' => '', 'endpoint' => 'sftp://h:22', 'type' => 'sftp', 'private_key_id' => $key->id]);
GithubApp::create(['team_id' => 1, 'name' => 'parity-fx-gh', 'api_url' => 'https://api.github.com', 'html_url' => 'https://github.com', 'custom_user' => 'git', 'custom_port' => 22, 'app_id' => 1, 'installation_id' => 2, 'client_id' => 'c', 'client_secret' => 's', 'webhook_secret' => 'w', 'private_key_id' => $key->id, 'webhook_events' => ['push']]);
GithubApp::create(['team_id' => $outsider, 'name' => 'parity-fx-gh-system', 'api_url' => 'https://api.github.com', 'html_url' => 'https://github.com', 'custom_user' => 'git', 'custom_port' => 22, 'app_id' => 3, 'installation_id' => 4, 'client_id' => 'c2', 'client_secret' => 's2', 'webhook_secret' => 'w2', 'is_system_wide' => true]);
GiteaApp::create(['team_id' => 1, 'name' => 'parity-fx-gt', 'api_url' => 'https://gitea.example.com/api/v1', 'html_url' => 'https://gitea.example.com', 'custom_port' => 22, 'custom_user' => 'git', 'client_id' => 'c', 'client_secret' => 's', 'access_token' => 't', 'refresh_token' => 'r', 'webhook_secret' => 'w']);
GiteaApp::create(['team_id' => 1, 'name' => 'parity-fx-gt-off', 'api_url' => 'https://gitea2.example.com/api/v1', 'html_url' => 'https://gitea2.example.com', 'custom_port' => 22, 'custom_user' => 'git']);
"""

CLEANUP = r"""
foreach (['cloud_provider_tokens', 'cloud_init_scripts', 'integration_tokens', 's3_storages', 'github_apps', 'gitlab_apps', 'gitea_apps', 'private_keys'] as $t) {
    Illuminate\Support\Facades\DB::table($t)->where('name', 'like', 'parity-%')->delete();
}
"""

_ready = False


def tinker(code: str) -> None:
    subprocess.run(
        [PHP, "artisan", "tinker", "--execute", code],
        cwd=PHP_ROOT,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        check=True,
        timeout=120,
    )


def ensure_fixtures() -> None:
    global _ready
    if _ready:
        return
    tinker(CLEANUP)
    tinker(FIXTURES)
    atexit.register(tinker, CLEANUP)
    _ready = True


def uuid_of(laravel: str, token: str, path: str, name: str, key: str = "uuid") -> str:
    res = call(laravel, "GET", path, token, None)
    for row in res.body if isinstance(res.body, list) else []:
        if isinstance(row, dict) and row.get("name") == name:
            return str(row.get(key))
    return "missing"


LISTS = [
    "/security/keys",
    "/security/integration-tokens",
    "/cloud-tokens",
    "/cloud-init-scripts",
    "/s3-storages",
    "/github-apps",
    "/gitlab-apps",
    "/gitea-apps",
]


def read_cases(tokens: dict[str, str], ids: dict[str, str]) -> list[Case]:
    ensure_fixtures()
    laravel = os.environ.get("PARITY_LARAVEL", "http://127.0.0.1:8000")
    root = tokens["owner_root"]
    keys = call(laravel, "GET", "/security/keys", root, None).body
    key_uuid = keys[0]["uuid"] if isinstance(keys, list) and keys else "missing"
    paths = LISTS + [
        f"/security/keys/{key_uuid}",
        f"/security/integration-tokens/{uuid_of(laravel, root, '/security/integration-tokens', 'parity-fx-cf')}",
        f"/security/integration-tokens/{uuid_of(laravel, root, '/security/integration-tokens', 'parity-fx-inf-a')}",
        f"/cloud-tokens/{uuid_of(laravel, root, '/cloud-tokens', 'parity-fx-cloud-a')}",
        f"/cloud-init-scripts/{uuid_of(laravel, root, '/cloud-init-scripts', 'parity-fx-bash')}",
        f"/s3-storages/{uuid_of(laravel, root, '/s3-storages', 'parity-fx-s3-a')}",
        f"/s3-storages/{uuid_of(laravel, root, '/s3-storages', 'parity-fx-sftp')}",
        "/security/keys/nope",
        "/security/integration-tokens/nope",
        "/cloud-tokens/nope",
        "/cloud-init-scripts/nope",
        "/s3-storages/nope",
    ]
    cases = [Case(f"{role} GET {p}", "GET", p, tokens[role]) for p in paths for role in READ_ROLES]
    for path, body in [
        ("/security/keys", {"name": "parity-denied", "private_key": "x"}),
        ("/cloud-init-scripts", {"name": "parity-denied", "script": "#!/bin/sh"}),
    ]:
        cases.append(Case(f"owner_read POST {path}", "POST", path, tokens["owner_read"], body))
    cases.append(Case("owner_write DELETE /cloud-tokens/nope", "DELETE", "/cloud-tokens/nope", tokens["owner_write"]))
    cases.append(Case("owner_root DELETE /gitlab-apps/999999", "DELETE", "/gitlab-apps/999999", root))
    return cases


def new_key(directory: str, name: str) -> str:
    path = os.path.join(directory, name)
    subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "parity", "-f", path], check=True)
    return Path(path).read_text()


def pair(laravel: str, rust: str, method: str, paths: tuple, token: str, bodies: tuple, label: str, out: list):
    """Send one request per side (own path/body each) and record the comparison."""
    lv = call(laravel, method, paths[0], token, bodies[0])
    rs = call(rust, method, paths[1], token, bodies[1])
    out.append((label, compare(lv, rs, VOLATILE_KEYS)))
    return lv, rs


def write_checks(laravel: str, rust: str, tokens: dict[str, str], ids: dict[str, str]) -> list[tuple[str, list[str]]]:
    ensure_fixtures()
    out: list[tuple[str, list[str]]] = []
    w, root, stamp = tokens["owner_write"], tokens["owner_root"], str(int(time.time()))
    keys_path = ("/security/keys", "/security/keys")

    with tempfile.TemporaryDirectory() as tmp:
        key_a, key_b = new_key(tmp, "a"), new_key(tmp, "b")
        pair(laravel, rust, "POST", keys_path, w, ({"name": "pa"},) * 2, "keys create missing key", out)
        pair(laravel, rust, "POST", keys_path, w, ({"private_key": "nope"},) * 2, "keys create invalid", out)
        lv, rs = pair(laravel, rust, "POST", keys_path, w,
                      ({"name": f"parity-l-{stamp}", "private_key": key_a}, {"name": f"parity-r-{stamp}", "private_key": key_b}),
                      "keys create", out)
        lu, ru = (lv.body or {}).get("uuid"), (rs.body or {}).get("uuid")
        pair(laravel, rust, "POST", keys_path, w,
             ({"name": "parity-dup", "private_key": key_a}, {"name": "parity-dup", "private_key": key_b}), "keys create duplicate", out)
        one = (f"/security/keys/{lu}", f"/security/keys/{ru}")
        derived = VOLATILE_KEYS | {"fingerprint", "private_key", "public_key"}
        lv_show, rs_show = call(laravel, "GET", one[0], root, None), call(rust, "GET", one[1], root, None)
        out.append(("keys show created", compare(lv_show, rs_show, derived)))
        pair(laravel, rust, "PATCH", one, w, ({"description": "new"},) * 2, "keys update description", out)
        pair(laravel, rust, "PATCH", one, w, ({"private_key": key_b}, {"private_key": key_a}), "keys update to duplicate", out)
        pair(laravel, rust, "PATCH", one, w, ({"bogus": 1},) * 2, "keys update extra field", out)
        pair(laravel, rust, "DELETE", one, w, (None, None), "keys delete", out)

        # The same key through each side in turn: fingerprint and public key must be byte-identical.
        key_c = new_key(tmp, "c")
        shown = []
        for base in (laravel, rust):
            made = call(base, "POST", "/security/keys", w, {"name": f"parity-same-{stamp}", "private_key": key_c})
            ident = (made.body or {}).get("uuid")
            shown.append(call(base, "GET", f"/security/keys/{ident}", root, None))
            call(base, "DELETE", f"/security/keys/{ident}", w, None)
        out.append(("keys same key derivation", compare(shown[0], shown[1], {"uuid", "id", "created_at", "updated_at"})))

    init_path = ("/cloud-init-scripts", "/cloud-init-scripts")
    lv, rs = pair(laravel, rust, "POST", init_path, w,
                  ({"name": f"parity-l-{stamp}", "script": "#!/bin/sh\necho l"}, {"name": f"parity-r-{stamp}", "script": "#!/bin/sh\necho r"}),
                  "cloud-init create", out)
    one = (f"/cloud-init-scripts/{(lv.body or {}).get('uuid')}", f"/cloud-init-scripts/{(rs.body or {}).get('uuid')}")
    pair(laravel, rust, "PATCH", one, w, ({},) * 2, "cloud-init update empty", out)
    pair(laravel, rust, "PATCH", one, w, ({"script": "#!/bin/sh\necho 2"},) * 2, "cloud-init update script", out)
    pair(laravel, rust, "PATCH", one, root, ({"name": "x" * 300},) * 2, "cloud-init update too long", out)
    pair(laravel, rust, "DELETE", one, w, (None, None), "cloud-init delete", out)
    pair(laravel, rust, "POST", init_path, w, ({"script": 5, "x": 1},) * 2, "cloud-init create invalid", out)

    def fixture_pair(path: str, suffix: str, key: str = "uuid") -> tuple[str, str]:
        return (f"{path}/{uuid_of(laravel, root, path, f'parity-fx-{suffix}-a', key)}",
                f"{path}/{uuid_of(laravel, root, path, f'parity-fx-{suffix}-b', key)}")

    tokens_pair = fixture_pair("/cloud-tokens", "cloud")
    pair(laravel, rust, "PATCH", tokens_pair, w, ({"name": ""},) * 2, "cloud token rename empty", out)
    pair(laravel, rust, "PATCH", tokens_pair, w, ({"name": "parity-renamed"},) * 2, "cloud token rename", out)
    pair(laravel, rust, "DELETE", tokens_pair, w, (None, None), "cloud token delete", out)
    for path, suffix, key, label in [
        ("/security/integration-tokens", "inf", "uuid", "integration token delete"),
        ("/s3-storages", "s3", "uuid", "s3 storage delete"),
        ("/gitlab-apps", "gl", "id", "gitlab app delete"),
    ]:
        pair(laravel, rust, "DELETE", fixture_pair(path, suffix, key), w, (None, None), label, out)
    return out
