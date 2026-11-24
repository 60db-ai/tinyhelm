"""Parity cases: notifications, teams and instance endpoints (NotificationsController, TeamController,
OtherController). Singleton writes are flipped and restored on each side, so no state is left behind."""

from __future__ import annotations

import json
import os
import subprocess
import urllib.error
import urllib.request

from compare import READ_ROLES, VOLATILE_KEYS, Case, call, compare, diff

CHANNELS = ["email", "discord", "slack", "telegram", "pushover", "webhook"]
LARAVEL = os.environ.get("PARITY_LARAVEL", "http://127.0.0.1:8000")
# Generated timestamps differ between the two writes; everything else must match exactly.
WRITE_IGNORE = VOLATILE_KEYS - {"name", "key", "description", "id"}


def _team_id(tokens: dict[str, str]) -> int:
    body = call(LARAVEL, "GET", "/team", tokens["owner_root"], None).body
    return int(body["id"]) if isinstance(body, dict) else 1


def read_cases(tokens: dict[str, str], ids: dict[str, str]) -> list[Case]:
    team = _team_id(tokens)
    paths = [f"/notifications/{c}" for c in CHANNELS] + [
        "/teams",
        "/teams/current",
        f"/teams/{team}",
        "/teams/999999",
        f"/teams/{team}/members",
        "/teams/999999/members",
        "/team/members",
        "/teams/current/members",
        "/version",
        "/realtime/config",
        "/health",
    ]
    cases = [Case(f"{role} GET {p}", "GET", p, tokens[role]) for p in paths for role in READ_ROLES]
    write = tokens["owner_write"]
    cases += [
        Case("no token GET /notifications/email", "GET", "/notifications/email", None),
        Case("owner_read PATCH /notifications/discord", "PATCH", "/notifications/discord", tokens["owner_read"], {"discord_enabled": True}),
        Case("owner_read PATCH /team", "PATCH", "/team", tokens["owner_read"], {"scan_images_on_new_servers": True}),
        Case("owner_read GET /disable", "GET", "/disable", tokens["owner_read"]),
        Case("owner_write GET /disable", "GET", "/disable", write),
        Case("owner_read POST /mcp/enable", "POST", "/mcp/enable", tokens["owner_read"]),
        Case("owner_write POST /mcp/enable (not root team)", "POST", "/mcp/enable", write),
        Case("owner_write POST /mcp/disable (not root team)", "POST", "/mcp/disable", write),
        Case("owner_write POST /disable (not root team)", "POST", "/disable", write),
        # Validation failures change nothing.
        Case("owner_write PATCH email invalid", "PATCH", "/notifications/email", write,
             {"smtp_port": 0, "smtp_encryption": "ssl", "smtp_enabled": "maybe", "bogus": 1, "smtp_timeout": "abc"}),
        Case("owner_write PATCH email bad ehlo", "PATCH", "/notifications/email", write, {"smtp_ehlo_domain": "bad host!"}),
        Case("owner_write PATCH email port too big", "PATCH", "/notifications/email", write, {"smtp_port": 70000}),
        Case("owner_write PATCH telegram wrong types", "PATCH", "/notifications/telegram", write,
             {"telegram_token": 123, "telegram_enabled": None}),
        Case("owner_write PATCH pushover empty json", "PATCH", "/notifications/pushover", write, {}),
        Case("owner_write PATCH team extra field", "PATCH", "/team", write, {"other": 1}),
        Case("owner_write PATCH team bad bool", "PATCH", "/team", write, {"scan_images_on_new_servers": "yes"}),
    ]
    return cases


def _last_audit() -> dict:
    sql = (
        "select row_to_json(a) from (select event, source, action, level, actor_type, resource_type, resource_uuid, "
        "resource_name, description, metadata, changes is not null as has_changes from audit_events order by id desc limit 1) a"
    )
    env = {**os.environ, "PGPASSWORD": os.environ.get("PARITY_PGPASSWORD", "postgres")}
    out = subprocess.run(
        ["psql", "-h", "127.0.0.1", "-U", "postgres", "-d", "coolify_app", "-tAc", sql],
        capture_output=True, text=True, env=env, check=False,
    ).stdout.strip()
    return json.loads(out) if out else {}


def _flip_and_restore(laravel: str, rust: str, token: str, path: str, field: str, original: object, label: str):
    results = {}
    for side, base in (("laravel", laravel), ("rust", rust)):
        changed = call(base, "PATCH", path, token, {field: not original})
        audit = _last_audit()
        restored = call(base, "PATCH", path, token, {field: original})
        results[side] = (changed, audit, restored)
    (lc, la, lr), (rc, ra, rr) = results["laravel"], results["rust"]
    return [
        (f"{label} change", compare(lc, rc, WRITE_IGNORE)),
        (f"{label} audit row", diff(la, ra, set())),
        (f"{label} restore", compare(lr, rr, WRITE_IGNORE)),
    ]


def _health(base: str) -> tuple[int, str]:
    try:
        with urllib.request.urlopen(base + "/api/health", timeout=10) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def write_checks(laravel: str, rust: str, tokens: dict[str, str], ids: dict[str, str]) -> list[tuple[str, list[str]]]:
    write = tokens["owner_write"]
    root = tokens["owner_root"]
    checks: list[tuple[str, list[str]]] = []

    lh, rh = _health(laravel), _health(rust)
    checks.append(("GET /api/health", [] if lh == rh else [f"{lh} vs {rh}"]))

    for ch in CHANNELS:
        field = f"backup_success_{ch}_notifications"
        current = call(laravel, "GET", f"/notifications/{ch}", root, None).body
        original = bool(current.get(field)) if isinstance(current, dict) else False
        checks += _flip_and_restore(laravel, rust, write, f"/notifications/{ch}", field, original, f"PATCH {ch} {field}")

    # An encrypted secret written by one side must read back through the other.
    secret = "telegram_chat_id"
    current = call(laravel, "GET", "/notifications/telegram", root, None).body
    original_secret = current.get(secret) if isinstance(current, dict) else None
    for writer, reader, label in ((rust, laravel, "rust->laravel"), (laravel, rust, "laravel->rust")):
        call(writer, "PATCH", "/notifications/telegram", root, {secret: "parity-chat-42"})
        seen = call(reader, "GET", "/notifications/telegram", root, None).body
        value = seen.get(secret) if isinstance(seen, dict) else seen
        checks.append((f"encrypted telegram secret {label}", [] if value == "parity-chat-42" else [f"read back {value!r}"]))
    call(laravel, "PATCH", "/notifications/telegram", root, {secret: original_secret})

    team = call(laravel, "GET", "/team", root, None).body
    original_team = bool(team.get("scan_images_on_new_servers")) if isinstance(team, dict) else False
    checks += _flip_and_restore(laravel, rust, write, "/team", "scan_images_on_new_servers", original_team, "PATCH /team scan_images_on_new_servers")
    return checks
