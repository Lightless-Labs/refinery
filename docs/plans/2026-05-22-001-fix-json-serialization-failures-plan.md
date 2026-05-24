---
title: "fix: JSON serialization failures in verb output paths"
type: fix
status: completed
date: 2026-05-22
completed: 2026-05-22
todo: 016-silent-json-serialization-failure
---

# JSON Serialization Failure Handling Plan

**Enhanced:** 2026-05-22 (via `/deepen-plan`)
**Reviewed:** 2026-05-23 (via `/coderabbit / review`)
**Completed:** 2026-05-22

## Problem

Some `refinery_cli` JSON output paths used `if let Ok(json) = serde_json::to_string_pretty(...)`, which silently swallowed serialization failures. In those paths a user requesting JSON could receive no output and no explanation.

## Goals

- Replace silent JSON serialization handling with explicit `match`/helper handling.
- Print serialization failures to stderr.
- Return a non-zero exit code for JSON success/output paths that cannot be serialized.
- Verify no silent `if let Ok(json) = serde_json::to_string_pretty(...)` patterns remain in CLI verb commands.

## Non-Goals

- Do not change the public JSON schema for converge, synthesize, or brainstorm.
- Do not alter provider behavior or verb execution semantics.
- Do not add artificial serialization-failure fixtures for otherwise serializable response types.

## Implementation Notes

- `converge` and `brainstorm` already used explicit `match` handling on current `main`.
- Updated remaining `synthesize` paths to use shared local JSON stdout/stderr helpers.
- Error-output serialization failures are now logged even though those paths already return an error exit code.

## Verification

Completed:

- `cargo fmt --all -- --check`
- `cargo test -p refinery_cli`
- `cargo clippy -p refinery_cli --all-targets -- -D warnings`
- `rg "if let Ok\\(json\\) = serde_json::to_string_pretty" crates/refinery_cli/src/commands -n` returned no matches.
