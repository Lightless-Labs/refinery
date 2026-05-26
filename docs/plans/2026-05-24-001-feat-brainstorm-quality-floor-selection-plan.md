---
date: 2026-05-24
topic: brainstorm-quality-floor-selection
todo: 023-brainstorm-quality-floor-selection
status: completed
completed: 2026-05-24
---

# Plan: Brainstorm Quality-Floor Selection

**Reviewed:** 2026-05-26 (via `/codex-review / local-pi`)

**Completed:** 2026-05-24

## Context

The six-prompt brainstorm benchmark showed that raw controversy (`mean_score * stddev`) improves diversity but can rank low-quality divisive answers too highly. The immediate candidate fix is a production selector that prefers answers with `mean_score >= 7.0`, while keeping raw controversy available for benchmark comparisons.

## Goals

- Add configurable quality-floor panel selection for brainstorm results.
- Keep raw controversy behavior available and covered by tests.
- Make the active selection strategy visible in CLI text and JSON output.
- Preserve existing artifact/analyzer counterfactual support for raw controversy and `controversy_floor_7`.

## Implementation Steps

1. Extend core scoring with a quality-floor selection helper:
   - rank qualifying candidates by existing controversy order;
   - if too few candidates qualify, backfill below-floor candidates by existing controversy order;
   - keep `select_panel` as the raw controversy selector for benchmarks/tests.
2. Extend `BrainstormConfig`/`BrainstormResult` to carry the optional quality floor and selected strategy label.
3. Update brainstorm CLI:
   - add `--quality-floor` with a default of `7.0`;
   - validate it is finite and within `0..=10`;
   - pass `None` for `0.0` to preserve raw controversy when requested;
   - emit the selection strategy in text and JSON output.
4. Add unit tests for the quality-floor exclusion/backfill behavior and CLI validation where practical.
5. Update docs/TODO/handoff after verification.

## Verification

Run the smallest relevant gates:

```sh
cargo fmt --all -- --check
cargo test -p refinery_core scoring
cargo test -p refinery_cli
cargo clippy -p refinery_core --all-targets -- -D warnings
cargo clippy -p refinery_cli --all-targets -- -D warnings
```
