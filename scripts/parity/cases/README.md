# Parity case files

One file per ported domain (`<domain>.py`), loaded by `compare.py`. Import helpers with
`from compare import Case, call, compare, READ_ROLES, VOLATILE_KEYS` (compare.py is on `sys.path` as the
running script's directory). Define either or both:

- `read_cases(tokens, ids) -> list[Case]` — GET and denied-write cases, run against both servers.
- `write_checks(laravel, rust, tokens, ids) -> list[tuple[label, problems]]` — paired writes: each side
  creates its own record with distinct values, compare, then delete each side's record and compare.
  Leave no rows behind.
