#!/usr/bin/env python3
"""Parity harness: send the same request to Laravel (:8000) and the Rust gateway (:8080) and diff them.

Usage:
    php artisan tinker --execute 'require base_path("scripts/parity/fixtures.php");'   # once
    python3 -I scripts/parity/compare.py [--only SUBSTRING] [--no-writes] [-v]

It compares status codes and JSON bodies for every native route, with every token role
(owner root/read/read:sensitive/write/deploy, a member with grants, a token of another team, no token).
Write routes run as pairs: each side creates its own record, then deletes it, and the shapes are compared
with generated values (uuid, id, timestamps, names) ignored. Exit code 1 when any case differs.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOKENS_FILE = ROOT / ".dev-instances" / "parity-tokens.json"
VOLATILE_KEYS = {"uuid", "id", "created_at", "updated_at", "last_used_at", "name", "key", "description"}
READ_ROLES = ["owner_root", "owner_read", "owner_read_sensitive", "owner_deploy", "member_read_write", "outsider_root"]


@dataclass
class Result:
    status: int
    body: object
    raw: str


@dataclass
class Case:
    label: str
    method: str
    path: str
    token: str | None
    body: dict | None = None


def call(base: str, method: str, path: str, token: str | None, body: dict | None) -> Result:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(base + "/api/v1" + path, data=data, method=method)
    req.add_header("Accept", "application/json")
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=30) as res:
            raw = res.read().decode("utf-8", "replace")
            status = res.status
    except urllib.error.HTTPError as err:
        raw = err.read().decode("utf-8", "replace")
        status = err.code
    try:
        parsed: object = json.loads(raw) if raw.strip() else None
    except json.JSONDecodeError:
        parsed = {"__non_json__": raw[:120]}
    return Result(status, parsed, raw)


def diff(a: object, b: object, ignore: set[str], path: str = "$") -> list[str]:
    """Structural diff (laravel vs rust). Values under keys in `ignore` are compared by type only."""
    out: list[str] = []
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in b:
                out.append(f"{path}.{k}: only in laravel")
            elif k not in a:
                out.append(f"{path}.{k}: only in rust")
            elif k in ignore:
                if type(a[k]) is not type(b[k]) and None not in (a[k], b[k]):
                    out.append(f"{path}.{k}: type {type(a[k]).__name__} vs {type(b[k]).__name__}")
            else:
                out.extend(diff(a[k], b[k], ignore, f"{path}.{k}"))
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f"{path}: length {len(a)} vs {len(b)}")
        for i, (x, y) in enumerate(zip(a, b)):
            out.extend(diff(x, y, ignore, f"{path}[{i}]"))
    elif a != b:
        out.append(f"{path}: {json.dumps(a)[:80]} vs {json.dumps(b)[:80]}")
    return out


def compare(lv: Result, rs: Result, ignore: set[str] | None = None) -> list[str]:
    problems = [] if lv.status == rs.status else [f"status {lv.status} vs {rs.status}"]
    return problems + diff(lv.body, rs.body, ignore or set())


def first(result: Result, key: str = "uuid") -> str:
    body = result.body
    if isinstance(body, list) and body and isinstance(body[0], dict):
        return str(body[0].get(key))
    return "missing"


def discover(laravel: str, token: str) -> dict[str, str]:
    """Real identifiers to put in paths, read from Laravel."""
    get = lambda p: call(laravel, "GET", p, token, None)  # noqa: E731
    ids = {
        "server": first(get("/servers")),
        "project": first(get("/projects")),
        "application": first(get("/applications")),
        "database": first(get("/databases")),
        "service": first(get("/services")),
    }
    return ids


def read_cases(tokens: dict[str, str], ids: dict[str, str]) -> list[Case]:
    paths = [
        "/servers",
        f"/servers/{ids['server']}",
        "/servers/does-not-exist",
        "/projects",
        f"/projects/{ids['project']}/environments",
        "/team",
        "/team/envs",
        "/tags",
        "/applications",
        f"/applications/{ids['application']}",
        f"/applications/{ids['application']}/envs",
        "/services",
        "/databases",
        f"/databases/{ids['database']}",
        f"/databases/{ids['database']}/envs",
    ]
    cases = [Case(f"{role} GET {p}", "GET", p, tokens[role]) for p in paths for role in READ_ROLES]
    cases += [
        Case("no token GET /servers", "GET", "/servers", None),
        Case("bad token GET /servers", "GET", "/servers", "999999|not-a-real-token"),
        Case("owner_read POST /tags", "POST", "/tags", tokens["owner_read"], {"name": "parity-denied"}),
        Case("owner_read POST /projects", "POST", "/projects", tokens["owner_read"], {"name": "parity-denied"}),
        Case("owner_read POST /team/envs", "POST", "/team/envs", tokens["owner_read"], {"key": "P_DENIED", "value": "x"}),
        Case("owner_deploy DELETE /tags/x", "DELETE", "/tags/does-not-exist", tokens["owner_deploy"]),
    ]
    return cases


WRITE_SPECS = [
    ("tags", "/tags", lambda side, stamp: {"name": f"parity-{side}-{stamp}"}, "uuid", "/tags/{id}"),
    ("projects", "/projects", lambda side, stamp: {"name": f"parity-{side}-{stamp}"}, "uuid", "/projects/{id}"),
    ("team envs", "/team/envs", lambda side, stamp: {"key": f"PARITY_{side.upper()}_{stamp}", "value": "v"}, "id", "/team/envs/{id}"),
]


def paired_write_checks(laravel: str, rust: str, token: str) -> list[tuple[str, list[str]]]:
    """Create on each side with distinct values and compare; then delete each side's record and compare."""
    stamp = str(int(time.time()))
    checks: list[tuple[str, list[str]]] = []
    for label, path, body, id_key, delete_path in WRITE_SPECS:
        created = {side: call(base, "POST", path, token, body(side, stamp)) for side, base in (("laravel", laravel), ("rust", rust))}
        checks.append((f"create {label}", compare(created["laravel"], created["rust"], VOLATILE_KEYS)))

        deleted: dict[str, Result] = {}
        for side, base in (("laravel", laravel), ("rust", rust)):
            res = created[side]
            ident = res.body.get(id_key) if isinstance(res.body, dict) else None
            if ident is None:
                checks.append((f"delete {label} ({side})", [f"no {id_key} in create response: {res.raw[:100]}"]))
                continue
            deleted[side] = call(base, "DELETE", delete_path.format(id=ident), token, None)
        if len(deleted) == 2:
            checks.append((f"delete {label}", compare(deleted["laravel"], deleted["rust"], VOLATILE_KEYS)))
    return checks


def report(label: str, problems: list[str], verbose: bool) -> int:
    if not problems:
        if verbose:
            print(f"  ok    {label}")
        return 0
    print(f"  DIFF  {label}")
    limit = 20 if verbose else 4
    for p in problems[:limit]:
        print(f"          {p}")
    if len(problems) > limit:
        print(f"          ... {len(problems) - limit} more")
    return 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--laravel", default=os.environ.get("PARITY_LARAVEL", "http://127.0.0.1:8000"))
    ap.add_argument("--rust", default=os.environ.get("PARITY_RUST", "http://127.0.0.1:8080"))
    ap.add_argument("--only", default="", help="run only cases whose label contains this text")
    ap.add_argument("--no-writes", action="store_true", help="skip the paired create/delete checks")
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args()

    tokens = json.loads(TOKENS_FILE.read_text())
    ids = discover(args.laravel, tokens["owner_root"])

    failures = total = 0
    for case in read_cases(tokens, ids):
        if args.only not in case.label:
            continue
        total += 1
        lv = call(args.laravel, case.method, case.path, case.token, case.body)
        rs = call(args.rust, case.method, case.path, case.token, case.body)
        failures += report(case.label, compare(lv, rs), args.verbose)

    if not args.no_writes and (not args.only or "write" in args.only):
        for label, problems in paired_write_checks(args.laravel, args.rust, tokens["owner_write"]):
            total += 1
            failures += report(f"owner_write {label}", problems, args.verbose)

    print(f"\n{total - failures}/{total} cases match")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
