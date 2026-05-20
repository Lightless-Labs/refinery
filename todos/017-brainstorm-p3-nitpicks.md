---
title: "chore: brainstorm verb P3 nitpicks from CodeRabbit reviews"
priority: low
milestone: v0.3
depends_on: 004-verb-brainstorm
---

# Brainstorm P3 Nitpicks

Collected from CodeRabbit review bodies on PR #27.

## Items

1. **Dry-run ignores --output-format json** — all three verbs (converge, synthesize, brainstorm) print text-only dry-run output regardless of `--output-format`. Add JSON dry-run output when requested.

2. **Empty providers early validation** — `brainstorm::run()` could return a clearer error if providers slice is empty, rather than falling through to "All models failed to propose."

3. **ScoreHistory as named struct** — replace `Vec<(String, f64)>` with a struct having `proposal: String` and `mean_score: f64` fields. Clearer, easier to evolve.

## Origin

CodeRabbit PR #27 review nitpick comments (reviews 4039962060 and 4042255885).
