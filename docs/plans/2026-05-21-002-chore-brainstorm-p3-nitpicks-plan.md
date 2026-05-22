---
title: "chore: brainstorm P3 nitpicks"
type: chore
status: completed
date: 2026-05-21
completed: 2026-05-21
todo: 017-brainstorm-p3-nitpicks
---

# Brainstorm P3 Nitpicks Plan

**Reviewed:** 2026-05-22 (via `/coderabbit / review`)
**Completed:** 2026-05-21

## Problem

CodeRabbit P3 nitpicks remain after the brainstorm verb merge:

1. Dry-run ignores `--output-format json`.
2. `brainstorm::run()` has no explicit empty-provider validation.
3. `ScoreHistory` is a tuple alias, making prompt code harder to read/evolve.

## Goals

- Emit structured JSON for dry-run mode across converge, synthesize, and brainstorm when `--output-format json` is requested.
- Return a clear brainstorm core error when no providers are passed.
- Replace `ScoreHistory = Vec<(String, f64)>` with named `ScoreHistoryEntry { proposal, mean_score }` entries.

## Non-Goals

- Do not alter non-dry-run JSON schemas except as required by type changes.
- Do not change provider behavior or brainstorm selection strategy.
- Do not implement TOON output.

## Implementation Notes

- Add a shared `DryRunOutput` JSON type/helper in CLI common code if practical.
- Keep text dry-run output unchanged for compatibility.
- Update prompt tests to use named `ScoreHistoryEntry` values.
- Add a core test for empty provider validation.

## Verification

Completed:

- `cargo fmt --all -- --check`
- `cargo test -p refinery_core brainstorm`
- `cargo check -p refinery_cli`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Manual dry-run JSON checks for `converge`, `synthesize`, and `brainstorm`.
