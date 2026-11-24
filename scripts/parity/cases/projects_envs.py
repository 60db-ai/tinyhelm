"""Parity cases: project detail and environments, project/environment/server shared variables,
tag update, resource tags (applications, databases) and GET /resources.

Write scenarios run completely on Laravel, then completely on Rust (each side with its own names), and
the responses are compared step by step, so neither side sees the other's records. Every record a
scenario creates is deleted at its end.
"""

from __future__ import annotations

import time

from compare import READ_ROLES, VOLATILE_KEYS, Case, Result, call, compare

# Values that differ per side by construction (generated or side-specific names/keys/ids).
SCENARIO_VOLATILE = VOLATILE_KEYS | {"env_id", "environment_uuid"}


def read_cases(tokens: dict[str, str], ids: dict[str, str]) -> list[Case]:
    p, s, a, d, svc = ids["project"], ids["server"], ids["application"], ids["database"], ids["service"]
    paths = [
        f"/projects/{p}",
        "/projects/does-not-exist",
        f"/projects/{p}/production",
        f"/projects/{p}/no-such-environment",
        f"/projects/{p}/envs",
        f"/projects/{p}/environments/production/envs",
        f"/projects/{p}/environments/no-such-environment/envs",
        "/projects/does-not-exist/envs",
        f"/servers/{s}/envs",
        "/servers/does-not-exist/envs",
        "/resources",
        f"/applications/{a}/tags",
        f"/databases/{d}/tags",
        f"/services/{svc}/tags",
        "/applications/does-not-exist/tags",
        "/databases/does-not-exist/tags",
    ]
    cases = [Case(f"{role} GET {path}", "GET", path, tokens[role]) for path in paths for role in READ_ROLES]
    read = tokens["owner_read"]
    deploy = tokens["owner_deploy"]
    cases += [
        Case("owner_read POST environments", "POST", f"/projects/{p}/environments", read, {"name": "parity-denied"}),
        Case("owner_read POST project envs", "POST", f"/projects/{p}/envs", read, {"key": "P_DENIED", "value": "x"}),
        Case("owner_read POST server envs", "POST", f"/servers/{s}/envs", read, {"key": "P_DENIED", "value": "x"}),
        Case("owner_read PATCH tag", "PATCH", "/tags/does-not-exist", read, {"name": "parity-denied"}),
        Case("owner_read POST app tags", "POST", f"/applications/{a}/tags", read, {"tag_name": "parity-denied"}),
        Case("owner_deploy DELETE app tag", "DELETE", f"/applications/{a}/tags/does-not-exist", deploy),
        Case("owner_deploy DELETE environment", "DELETE", f"/projects/{p}/environments/nope", deploy),
    ]
    return cases


def _id(res: Result, key: str) -> str:
    return str(res.body.get(key)) if isinstance(res.body, dict) else "missing"


def _scenario(base: str, side: str, tokens: dict[str, str], ids: dict[str, str], stamp: str) -> list[tuple[str, Result]]:
    """One side's full write scenario; returns (step label, response) in order."""
    w = tokens["owner_write"]
    root = tokens["owner_root"]
    p, s, a, d = ids["project"], ids["server"], ids["application"], ids["database"]
    steps: list[tuple[str, Result]] = []

    def step(label: str, method: str, path: str, body: dict | None = None, token: str = w) -> Result:
        res = call(base, method, path, token, body)
        steps.append((label, res))
        return res

    # --- environments ---
    env_name = f"parity-{side}-{stamp}"
    step("env create: invalid name", "POST", f"/projects/{p}/environments", {"name": "x"})
    step("env create: extra field", "POST", f"/projects/{p}/environments", {"name": env_name, "color": "red"})
    step("env create: missing project", "POST", "/projects/does-not-exist/environments", {"name": env_name})
    step("env create", "POST", f"/projects/{p}/environments", {"name": env_name})
    step("env create: duplicate", "POST", f"/projects/{p}/environments", {"name": env_name})
    step("env details (empty, by name)", "GET", f"/projects/{p}/{env_name}", token=root)
    step("env update: description", "PATCH", f"/projects/{p}/environments/{env_name}", {"description": "Parity env"})
    step("env update: same values", "PATCH", f"/projects/{p}/environments/{env_name}", {"description": "Parity env"})
    step("env update: duplicate name", "PATCH", f"/projects/{p}/environments/{env_name}", {"name": "production"})
    step("env update: bad field", "PATCH", f"/projects/{p}/environments/{env_name}", {"nope": 1})

    # --- environment shared variables (inside the new environment) ---
    env_base = f"/projects/{p}/environments/{env_name}/envs"
    created = step("environment env create", "POST", env_base, {"key": f"PARITY_E_{side.upper()}", "value": "one"})
    eid = _id(created, "id")
    step("environment env list", "GET", env_base, token=root)
    step("environment env update", "PATCH", f"{env_base}/{eid}", {"value": "two", "comment": "c"})
    step("environment env update: empty body", "PATCH", f"{env_base}/{eid}", {})
    step("environment env delete", "DELETE", f"{env_base}/{eid}")
    step("environment env delete: gone", "DELETE", f"{env_base}/{eid}")

    step("env delete", "DELETE", f"/projects/{p}/environments/{env_name}")
    step("env delete: gone", "DELETE", f"/projects/{p}/environments/{env_name}")

    # --- project and server shared variables ---
    for scope, base_path in (("project", f"/projects/{p}/envs"), ("server", f"/servers/{s}/envs")):
        key = f"PARITY_{scope[0].upper()}_{side.upper()}_{stamp}"
        step(f"{scope} env create: invalid key", "POST", base_path, {"key": "1bad", "value": "x"})
        res = step(f"{scope} env create", "POST", base_path, {"key": key, "value": "v1", "is_literal": True})
        env_id = _id(res, "id")
        step(f"{scope} env create: duplicate", "POST", base_path, {"key": key, "value": "v2"})
        step(f"{scope} env update", "PATCH", f"{base_path}/{env_id}", {"value": "v2", "is_multiline": True}, token=root)
        step(f"{scope} env update: not found", "PATCH", f"{base_path}/999999999", {"value": "v"})
        step(f"{scope} env delete", "DELETE", f"{base_path}/{env_id}")

    # --- tag update ---
    t1 = step("tag create", "POST", "/tags", {"name": f"parity-t-{side}-{stamp}"})
    t1_uuid = _id(t1, "uuid")
    step("tag create (second)", "POST", "/tags", {"name": f"parity-u-{side}-{stamp}"})
    step("tag update: rename", "PATCH", f"/tags/{t1_uuid}", {"name": f"parity-r-{side}-{stamp}"})
    step("tag update: same name", "PATCH", f"/tags/{t1_uuid}", {"name": f"parity-r-{side}-{stamp}"})
    step("tag update: conflict", "PATCH", f"/tags/{t1_uuid}", {"name": f"parity-u-{side}-{stamp}"})
    step("tag update: too short", "PATCH", f"/tags/{t1_uuid}", {"name": "<b>a</b>"})
    step("tag update: missing", "PATCH", "/tags/does-not-exist", {"name": "parity-missing"})

    # --- resource tags ---
    for kind, uuid in (("applications", a), ("databases", d)):
        tag_base = f"/{kind}/{uuid}/tags"
        step(f"{kind} tags: both fields", "POST", tag_base, {"tag_name": "aa", "tag_names": ["bb"]})
        step(f"{kind} tags: empty", "POST", tag_base, {})
        step(f"{kind} tags: extra + bad type", "POST", tag_base, {"tag_names": "nope", "x": 1})
        step(f"{kind} tags: too short", "POST", tag_base, {"tag_names": ["ok-tag", "<i>z</i>"]})
        attached = step(
            f"{kind} tags: attach",
            "POST",
            tag_base,
            {"tag_names": [f"parity-r-{side}-{stamp}", f"parity-n-{side}-{stamp}", f"PARITY-N-{side}-{stamp}"]},
        )
        step(f"{kind} tags: list", "GET", tag_base, token=root)
        attached_uuids = [t.get("uuid") for t in attached.body] if isinstance(attached.body, list) else []
        for tag_uuid in attached_uuids:
            step(f"{kind} tags: detach", "DELETE", f"{tag_base}/{tag_uuid}")
        step(f"{kind} tags: detach again", "DELETE", f"{tag_base}/{t1_uuid}")

    # --- cleanup: every tag this scenario created ---
    listed = call(base, "GET", "/tags", root, None).body
    for tag in listed if isinstance(listed, list) else []:
        if isinstance(tag, dict) and str(tag.get("name", "")).endswith(f"-{side}-{stamp}"):
            call(base, "DELETE", f"/tags/{tag['uuid']}", w, None)
    return steps


def write_checks(laravel: str, rust: str, tokens: dict[str, str], ids: dict[str, str]) -> list[tuple[str, list[str]]]:
    stamp = str(int(time.time()))
    lv = _scenario(laravel, "laravel", tokens, ids, stamp)
    rs = _scenario(rust, "rust", tokens, ids, stamp)
    checks = [(label, compare(l_res, r_res, SCENARIO_VOLATILE)) for (label, l_res), (_, r_res) in zip(lv, rs)]
    if len(lv) != len(rs):
        checks.append(("scenario length", [f"{len(lv)} vs {len(rs)} steps"]))
    return checks
