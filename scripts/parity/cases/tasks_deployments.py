"""Parity cases: scheduled tasks, audit events, deployments, storages and destinations (step 3)."""

from __future__ import annotations

import re
import time

from compare import READ_ROLES, VOLATILE_KEYS, Case, Result, call, compare

APP = "app-real-1"
TASK_VOLATILE = VOLATILE_KEYS | {"uuid"}


def read_cases(tokens: dict[str, str], ids: dict[str, str]) -> list[Case]:
    paths = [
        f"/applications/{APP}/scheduled-tasks",
        "/applications/does-not-exist/scheduled-tasks",
        f"/applications/{APP}/scheduled-tasks/does-not-exist/executions",
        "/deployments",
        "/deployments/does-not-exist",
        f"/deployments/applications/{APP}",
        f"/deployments/applications/{APP}?skip=1&take=1",
        "/deployments/applications/does-not-exist",
        f"/applications/{APP}/storages",
        "/applications/does-not-exist/storages",
        f"/databases/{ids['database']}/storages",
        "/databases/does-not-exist/storages",
        f"/applications/{APP}/destinations",
        "/applications/does-not-exist/destinations",
    ]
    cases = [Case(f"{role} GET {p}", "GET", p, tokens[role]) for p in paths for role in READ_ROLES]
    cases.append(Case("owner_read POST scheduled task", "POST", f"/applications/{APP}/scheduled-tasks", tokens["owner_read"], {"name": "x"}))
    cases.append(Case("owner_write DELETE scheduled task", "DELETE", f"/applications/{APP}/scheduled-tasks/x", tokens["owner_write"]))
    return cases


def _strip_host(value: object) -> object:
    """Paginator URLs embed the request host (Laravel :8000, Rust :8114): compare path + query only."""
    if isinstance(value, dict):
        return {k: _strip_host(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_strip_host(v) for v in value]
    if isinstance(value, str):
        return re.sub(r"^https?://[^/]+", "", value)
    return value


def _stable(laravel: str, rust: str, method: str, path: str, token: str, body: dict | None = None) -> list[str]:
    """Compare under concurrent writes (other forks add audit rows): on a mismatch, read Laravel again and
    retry when the two Laravel reads disagree with each other."""
    problems: list[str] = []
    for _ in range(3):
        lv = call(laravel, method, path, token, body)
        rs = call(rust, method, path, token, body)
        lv.body, rs.body = _strip_host(lv.body), _strip_host(rs.body)
        problems = compare(lv, rs)
        if not problems:
            return []
        again = call(laravel, method, path, token, body)
        again.body = _strip_host(again.body)
        if not compare(lv, again):
            return problems  # Laravel is stable, so the difference is real
    return problems


def _message_only(laravel: str, rust: str, path: str, token: str) -> list[str]:
    """Laravel's debug-mode exception 404s add exception/file/line/trace; production returns only `message`."""
    lv = call(laravel, "GET", path, token, None)
    rs = call(rust, "GET", path, token, None)
    if isinstance(lv.body, dict) and "exception" in lv.body:
        lv.body = {"message": lv.body.get("message")}
    return compare(lv, rs)


def _task_checks(laravel: str, rust: str, tokens: dict[str, str]) -> list[tuple[str, list[str]]]:
    checks: list[tuple[str, list[str]]] = []
    base = f"/applications/{APP}/scheduled-tasks"
    write, root = tokens["owner_write"], tokens["owner_root"]
    stamp = str(int(time.time()))

    def both(label: str, method: str, path: str, token: str, body: dict | None) -> None:
        checks.append((label, compare(call(laravel, method, path, token, body), call(rust, method, path, token, body), TASK_VOLATILE)))

    # Validation and permission failures (no rows are created).
    both("task create: missing fields", "POST", base, write, {})
    both("task create: invalid cron", "POST", base, write, {"name": "p", "command": "echo", "frequency": "61 * * * *", "enabled": False})
    both("task create: bad timeout + extra", "POST", base, write, {"name": "p", "command": "echo", "frequency": "@daily", "timeout": 0, "nope": 1, "enabled": False})
    both("task create: null timeout", "POST", base, write, {"name": "p", "command": "echo", "frequency": "@daily", "timeout": None, "enabled": False})
    both("task create: enabled without deploy", "POST", base, write, {"name": "p", "command": "echo", "frequency": "@daily"})

    created: dict[str, dict[str, str]] = {"laravel": {}, "rust": {}}
    for variant, token, body in (
        ("disabled", write, {"command": "echo hi", "frequency": "*/5 * * * *", "enabled": False, "timeout": "60", "container": "web"}),
        ("enabled", root, {"command": "echo hi", "frequency": "@daily", "use_shell": False}),
    ):
        res: dict[str, Result] = {}
        for side, url in (("laravel", laravel), ("rust", rust)):
            res[side] = call(url, "POST", base, token, {"name": f"parity-{side}-{variant}-{stamp}", **body})
            if isinstance(res[side].body, dict) and res[side].body.get("uuid"):
                created[side][variant] = res[side].body["uuid"]
        checks.append((f"task create {variant}", compare(res["laravel"], res["rust"], TASK_VOLATILE)))

    # Updates (each side on its own task), a no-op update, an empty body, executions.
    for label, token, body in (
        ("rename", write, {"name": "<b>parity</b>-renamed"}),
        ("no-op", write, {"name": "parity-renamed"}),
        ("empty", write, {}),
        ("timeout", write, {"timeout": 120, "container": None}),
        ("enable without deploy", write, {"enabled": True}),
        ("enable with root", root, {"enabled": True}),
    ):
        res = {side: call(url, "PATCH", f"{base}/{created[side].get('disabled', 'missing')}", token, body) for side, url in (("laravel", laravel), ("rust", rust))}
        checks.append((f"task update {label}", compare(res["laravel"], res["rust"], TASK_VOLATILE)))
    res = {side: call(url, "GET", f"{base}/{created[side].get('disabled', 'missing')}/executions", write, None) for side, url in (("laravel", laravel), ("rust", rust))}
    checks.append(("task executions", compare(res["laravel"], res["rust"])))

    # Listing with the new tasks present, then delete everything this run created.
    listing = [call(url, "GET", base, tokens["owner_read"], None) for url in (laravel, rust)]
    checks.append(("task list with fixtures", compare(listing[0], listing[1], TASK_VOLATILE)))
    for variant in ("disabled", "enabled"):
        res = {side: call(url, "DELETE", f"{base}/{created[side].get(variant, 'missing')}", write, None) for side, url in (("laravel", laravel), ("rust", rust))}
        checks.append((f"task delete {variant}", compare(res["laravel"], res["rust"])))
    return checks


def write_checks(laravel: str, rust: str, tokens: dict[str, str], ids: dict[str, str]) -> list[tuple[str, list[str]]]:
    checks = _task_checks(laravel, rust, tokens)

    # Sensitive columns differ by role; members are delegated to Laravel by the guard (no need to compare).
    for role in ("owner_root", "owner_read", "outsider_root"):
        for query in ("", "?per_page=3&page=2", "?search=worker-test", "?source=api&action=updated&per_page=5",
                      "?search=no-such-event-xyz"):
            checks.append((f"{role} audit-events{query}", _stable(laravel, rust, "GET", f"/audit-events{query}", tokens[role])))
    for query in ("?per_page=0", "?page=abc&source=nope", "?per_page=&search[]=x", "?per_page=101&page=0"):
        checks.append((f"owner_read audit-events{query}", _stable(laravel, rust, "GET", f"/audit-events{query}", tokens["owner_read"])))

    destination_paths = ["/destinations", "/destinations/does-not-exist", f"/servers/{ids['server']}/destinations",
                         "/servers/does-not-exist/destinations"]
    first = call(laravel, "GET", "/destinations", tokens["owner_root"], None).body
    if isinstance(first, list) and first:
        destination_paths += [f"/destinations/{d['uuid']}" for d in first if isinstance(d, dict)]
    for role in READ_ROLES:
        for path in destination_paths:
            checks.append((f"{role} GET {path}", _message_only(laravel, rust, path, tokens[role])))

    # Deployments by uuid, from the team list (fixtures or real data).
    # dep-local-1 etc. are hand-made rows whose application_id is a uuid: Laravel fails with a 500 there.
    for dep in ("parity-dep-running", "parity-dep-done", "does-not-exist"):
        for role in READ_ROLES:
            checks.append((f"{role} GET /deployments/{dep}", _message_only(laravel, rust, f"/deployments/{dep}", tokens[role])))
    # Not compared: `?skip=` (empty) makes Laravel pass null to an int parameter (500); Rust uses the default.
    for query in ("?skip=-1", "?take=0&skip=abc", "?skip=2&take=5"):
        path = f"/deployments/applications/{APP}{query}"
        checks.append((f"owner_read GET {path}", compare(call(laravel, "GET", path, tokens["owner_read"], None), call(rust, "GET", path, tokens["owner_read"], None))))
    return checks
