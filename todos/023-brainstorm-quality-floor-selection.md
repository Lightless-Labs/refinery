---
title: "feat: add quality floor to brainstorm controversial selection"
priority: medium
milestone: v0.4
created: 2026-05-23
depends_on: 013-brainstorm-strategy-benchmarks
---

# Add Quality Floor to Brainstorm Selection

## Problem

The six-prompt benchmark found that raw controversy (`mean * stddev`) is too willing to trade answer quality for evaluator disagreement.

Most obvious failure: in the debugging prompt, raw controversy selected a MiniMax answer first with `mean=5.67`, `stddev=1.25`, `controversy=7.07`. That reduced lexical overlap but created an unacceptable panel quality floor.

Report: `docs/brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md`.

## Candidate Fix

Add a panel-selection quality floor, initially benchmarked as `controversy_floor_7`:

1. Prefer candidates with `mean_score >= 7.0`.
2. Rank qualifying candidates by current controversy order (`controversy_score`, then `mean_score`).
3. If fewer than `panel_size` candidates qualify, backfill from below-floor candidates by controversy order.

## Acceptance Criteria

- Brainstorm panel selection can enforce or configure a minimum mean score.
- Existing raw controversy behavior remains testable for benchmark comparisons.
- JSON/text output makes the selected strategy clear if user-facing behavior changes.
- Unit tests cover high-disagreement low-quality candidates being excluded when enough qualifying candidates exist.

## Origin

Follow-up from `todos/013` six-prompt benchmark.
