---
title: "chore: brainstorm verb P3 nitpicks from CodeRabbit reviews"
priority: low
milestone: v0.3
depends_on: 004-verb-brainstorm
updated: 2026-05-21
status: completed
completed: 2026-05-21
---

# Brainstorm P3 Nitpicks

Collected from CodeRabbit review bodies on PR #27.

## Items

Completed in implementation plan `docs/plans/2026-05-21-002-chore-brainstorm-p3-nitpicks-plan.md`.

1. **Dry-run ignores --output-format json** — all three verbs (converge, synthesize, brainstorm) now emit structured JSON dry-run output when requested.

2. **Empty providers early validation** — `brainstorm::run()` now returns a clearer error if providers slice is empty, rather than falling through to "All models failed to propose."

3. **ScoreHistory as named struct** — replaced tuple entries with `ScoreHistoryEntry { proposal, mean_score }`.

## Origin

CodeRabbit PR #27 review nitpick comments (reviews 4039962060 and 4042255885).
