"""Parity cases for resource configuration updates (step 3) and service part reads.

- PATCH /applications/{uuid} and /databases/{uuid}: Rust answers name/description changes; anything
  else is forwarded to Laravel. Paired writes set a different value on each side, compare the
  responses and the audit rows, and restore the original value.
- GET /services/{uuid}/applications|databases[/{part}]: compared on a throwaway service created and
  deleted through Laravel.
"""

from __future__ import annotations

import time

from compare import READ_ROLES, Case, call, compare

AUDIT_FIELDS = ("event", "source", "action", "level", "actor_type", "resource_type", "resource_uuid")


def read_cases(tokens: dict[str, str], ids: dict[str, str]) -> list[Case]:
    app = f"/applications/{ids['application']}"
    db = f"/databases/{ids['database']}"
    cases: list[Case] = []
    # Denials and lookups for every role (no write happens: invalid names or missing rows).
    for role in READ_ROLES + ["owner_write"]:
        cases.append(Case(f"{role} PATCH {app} short name", "PATCH", app, tokens[role], {"name": "ab"}))
        cases.append(Case(f"{role} PATCH {db} short name", "PATCH", db, tokens[role], {"name": "ab"}))
        cases.append(Case(f"{role} PATCH /applications/missing", "PATCH", "/applications/does-not-exist", tokens[role], {"description": "x"}))
        cases.append(Case(f"{role} PATCH /databases/missing", "PATCH", "/databases/does-not-exist", tokens[role], {"description": "x"}))
    w = tokens["owner_write"]
    for path in (app, db, "/databases/does-not-exist"):
        cases += [
            Case(f"owner_write PATCH {path} null name", "PATCH", path, w, {"name": None}),
            Case(f"owner_write PATCH {path} bad name", "PATCH", path, w, {"name": "bad$name%"}),
            Case(f"owner_write PATCH {path} long description", "PATCH", path, w, {"description": "d" * 300}),
            Case(f"owner_write PATCH {path} bad description", "PATCH", path, w, {"description": "no <> here {}"}),
            Case(f"owner_write PATCH {path} name number", "PATCH", path, w, {"name": 12345}),
            # Delegated to Laravel on the Rust side (field outside name/description): must still match.
            Case(f"owner_write PATCH {path} unknown field", "PATCH", path, w, {"name": "valid name", "not_a_field": 1}),
        ]
    cases.append(Case("no token PATCH app", "PATCH", app, None, {"description": "x"}))
    return cases


def _audit_after(laravel: str, token: str, marker: int, event: str, uuid: str) -> dict | None:
    res = call(laravel, "GET", "/audit-events?per_page=100", token, None)
    rows = res.body.get("data", []) if isinstance(res.body, dict) else []
    for row in rows:
        if row.get("id", 0) > marker and row.get("event") == event and row.get("resource_uuid") == uuid:
            return row
    return None


def _max_audit_id(laravel: str, token: str) -> int:
    res = call(laravel, "GET", "/audit-events?per_page=1", token, None)
    rows = res.body.get("data", []) if isinstance(res.body, dict) else []
    return rows[0]["id"] if rows else 0


def _audit_problems(lv: dict | None, rs: dict | None, label: str) -> list[str]:
    if lv is None and rs is None:
        return []
    if lv is None or rs is None:
        return [f"{label}: audit row only on {'rust' if lv is None else 'laravel'}"]
    problems = [f"{label}: {k} {lv.get(k)!r} vs {rs.get(k)!r}" for k in AUDIT_FIELDS if lv.get(k) != rs.get(k)]
    lm, rm = lv.get("metadata") or {}, rs.get("metadata") or {}
    if sorted(lm) != sorted(rm):
        problems.append(f"{label}: metadata keys {sorted(lm)} vs {sorted(rm)}")
    for k in ("changed_fields", "team_id", "database_type"):
        if lm.get(k) != rm.get(k):
            problems.append(f"{label}: metadata.{k} {lm.get(k)!r} vs {rm.get(k)!r}")
    lc, rc = lv.get("changes"), rs.get("changes")
    if (lc is None) != (rc is None) or (isinstance(lc, dict) and sorted(lc) != sorted(rc or {})):
        problems.append(f"{label}: changes {lc!r} vs {rc!r}")
    return problems


def _patch_both(laravel: str, rust: str, tokens: dict, path: str, uuid: str, events: list[str], body_for) -> tuple[dict, dict]:
    root, w = tokens["owner_root"], tokens["owner_write"]
    results, audits = {}, {}
    for side, base in (("laravel", laravel), ("rust", rust)):
        marker = _max_audit_id(laravel, root)
        results[side] = call(base, "PATCH", path, w, body_for(side))
        time.sleep(0.3)
        audits[side] = {e: _audit_after(laravel, root, marker, e, uuid) for e in events}
    return results, audits


def _paired_patch(laravel: str, rust: str, tokens: dict, path: str, uuid: str, field: str, events: list[str]) -> list[tuple[str, list[str]]]:
    root = tokens["owner_root"]
    original = call(laravel, "GET", path, root, None).body
    before = original.get(field) if isinstance(original, dict) else None
    stamp = str(int(time.time()))
    checks: list[tuple[str, list[str]]] = []

    results, audits = _patch_both(laravel, rust, tokens, path, uuid, events, lambda side: {field: f"Parity {side} {stamp}"})
    checks.append((f"PATCH {path} {field}", compare(results["laravel"], results["rust"])))
    for e in events:
        checks.append((f"PATCH {path} {field} audit {e}", _audit_problems(audits["laravel"][e], audits["rust"][e], e)))

    # The same value twice: nothing changes (Laravel skips the save; databases still record the controller event).
    call(laravel, "PATCH", path, tokens["owner_write"], {field: "Parity same value"})
    results, audits = _patch_both(laravel, rust, tokens, path, uuid, events, lambda side: {field: "Parity same value"})
    checks.append((f"PATCH {path} {field} unchanged", compare(results["laravel"], results["rust"])))
    for e in events:
        checks.append((f"PATCH {path} {field} unchanged audit {e}", _audit_problems(audits["laravel"][e], audits["rust"][e], e)))

    restore = call(laravel, "PATCH", path, root, {field: before})
    if restore.status != 200:
        checks.append((f"restore {path} {field}", [f"restore failed: {restore.status} {restore.raw[:120]}"]))
    return checks


def _throwaway_service(laravel: str, root: str) -> str | None:
    servers = call(laravel, "GET", "/servers", root, None).body
    projects = call(laravel, "GET", "/projects", root, None).body
    if not (isinstance(servers, list) and servers and isinstance(projects, list) and projects):
        return None
    res = call(laravel, "POST", "/services", root, {
        "type": "bookstack",
        "name": "parity-service",
        "server_uuid": servers[0]["uuid"],
        # The server may have several destinations; Laravel then requires one explicitly.
        **({"destination_uuid": d[0]["uuid"]} if isinstance(d := call(laravel, "GET", f"/servers/{servers[0]['uuid']}/destinations", root, None).body, list) and d else {}),
        "project_uuid": projects[0]["uuid"],
        "environment_name": "production",
        "instant_deploy": False,
    })
    return res.body.get("uuid") if isinstance(res.body, dict) else None


def write_checks(laravel: str, rust: str, tokens: dict[str, str], ids: dict[str, str]) -> list[tuple[str, list[str]]]:
    checks: list[tuple[str, list[str]]] = []
    app, db = ids["application"], ids["database"]
    checks += _paired_patch(laravel, rust, tokens, f"/applications/{app}", app, "description", ["api.application.updated"])
    checks += _paired_patch(laravel, rust, tokens, f"/applications/{app}", app, "name", ["api.application.updated"])
    db_events = ["api.standalone_postgresql.updated", "api.database.updated"]
    checks += _paired_patch(laravel, rust, tokens, f"/databases/{db}", db, "description", db_events)
    checks += _paired_patch(laravel, rust, tokens, f"/databases/{db}", db, "name", db_events)

    root = tokens["owner_root"]
    service = _throwaway_service(laravel, root)
    if service is None:
        checks.append(("service parts: throwaway service", ["could not create a service through Laravel"]))
        return checks
    try:
        apps = call(laravel, "GET", f"/services/{service}/applications", root, None).body or []
        dbs = call(laravel, "GET", f"/services/{service}/databases", root, None).body or []
        paths = [
            f"/services/{service}/applications",
            f"/services/{service}/databases",
            f"/services/{service}/applications/does-not-exist",
            f"/services/{service}/databases/does-not-exist",
            "/services/does-not-exist/applications",
            "/services/does-not-exist/databases",
        ]
        paths += [f"/services/{service}/applications/{a['uuid']}" for a in apps if isinstance(a, dict)]
        paths += [f"/services/{service}/databases/{d['uuid']}" for d in dbs if isinstance(d, dict)]
        for path in paths:
            for role in READ_ROLES:
                lv = call(laravel, "GET", path, tokens[role], None)
                rs = call(rust, "GET", path, tokens[role], None)
                checks.append((f"{role} GET {path.replace(service, '<svc>')}", compare(lv, rs)))
    finally:
        call(laravel, "DELETE", f"/services/{service}", root, None)
    return checks
