# TinyHelm → Rust migration

Tooling: [rust-anything](https://github.com/erkamyaman/rust-anything) 3.4.0 (dev dependency; registered as a Claude Code MCP server + skill).

## Status
Whole-app plan: `PLAN.md` (`rust-anything analyze --whole-app --lang php`).
- 2389 source files, ~309k lines, 277 units: 118 ready · 14 caution · 145 blocked.
- 145 blocked mainly because `laravel/framework` has no Rust equivalent (see PLAN.md "Decisions", D10).

## Per-unit loop
`analyze → bridge → record → port src/port.rs → build → diff → shadow → gate → enable`

## Open architectural decision (PLAN.md D1)
Server app: port every entry point behind an axum front-door (`rust-anything app front-door`)? This is a re-architecture of the Laravel app, not a 1:1 port. Pending user sign-off before bulk porting.

## Suggested first proof-of-concept unit
`app/Enums/` (pure logic: Role, AccessLevel, AccessScope) — real value, no framework deps.
