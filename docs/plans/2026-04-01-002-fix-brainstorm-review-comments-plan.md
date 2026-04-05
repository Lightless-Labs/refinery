---
title: "fix: address PR #27 review comments"
type: fix
status: active
date: 2026-04-01
---

# PR #27 Review Comment Triage

## Already Fixed (reply needed)

1. panel_size=0 validation (Copilot 3018221588) — fixed in 35efb18
2. First-round prompt inconsistency (Copilot 3018221608) — fixed in 35efb18

## P1 Fixes

3. Dry-run with 0 models: `n * (n - 1)` underflows on usize. Add empty-models check before estimation.
4. Empty scores default to 0.0: single-model or all-evals-fail produces misleading 0.0 mean in ScoreHistory. Use `None`-like sentinel or skip score recording.

## P2/P3 Defer

5. --output-dir unused in brainstorm — defer, create TODO.
