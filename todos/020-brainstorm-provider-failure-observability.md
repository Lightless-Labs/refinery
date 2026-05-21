---
title: "fix: surface brainstorm provider failures and degraded-run semantics"
priority: medium
milestone: v0.3
depends_on: 004-verb-brainstorm
created: 2026-05-21
status: completed
completed: 2026-05-21
---

# Surface Brainstorm Provider Failures and Degraded-Run Semantics

## Problem

The 2026-05-21 brainstorm smoke test showed that `refinery brainstorm` can silently degrade from a requested multi-provider run to an effectively single-provider run.

Observed case:

```sh
refinery brainstorm "..." --models claude-code,codex-cli,gemini-cli --max-rounds 2 --panel-size 3
```

Only `codex-cli/gpt-5.4` produced usable proposals. Claude failed due access (`403` / no Claude access) and Gemini failed due quota/capacity. The successful JSON output still returned:

- `status: "brainstormed"`
- one panel entry
- `mean_score: 0.0`
- `stddev: 0.0`
- `controversy_score: 0.0`
- `per_evaluator_scores: []`
- `models_dropped: []`

That output is misleading because controversial selection and peer evaluation were not exercised.

## Desired Behavior

Design the exact policy, then implement it. Candidate requirements:

- Track provider failures per round and phase, including provider/model id and sanitized error message.
- Surface partial failures in text and JSON outputs.
- Populate `models_dropped` or a richer provider-status field when requested providers stop contributing.
- Distinguish "not evaluated" from numeric score `0.0`; avoid implying that missing peer evaluations are real zero scores.
- Consider failing the run when fewer than two providers remain for a multi-model brainstorm, unless an explicit degraded/single-provider mode is requested.
- Preserve successful round artifacts and include partial-run summary on later-round failure where possible.

## Acceptance Criteria

Completed in implementation commit for `docs/plans/2026-05-21-001-fix-brainstorm-provider-failure-observability-plan.md`.

- A multi-model brainstorm where one provider succeeds and others fail does not look like a clean, fully evaluated brainstorm.
- JSON output lets automation distinguish:
  - all requested providers participated,
  - some providers failed/dropped,
  - evaluation was skipped because fewer than two providers produced proposals.
- Text output includes actionable provider failure information without dumping raw secrets or excessive CLI logs.
- Tests cover partial provider failure during proposal and evaluation phases.

Verification completed:

- `cargo test -p refinery_core brainstorm`
- `cargo check -p refinery_cli`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Manual degraded JSON smoke with `claude-code,codex-cli` showing `status: "degraded"`, `evaluated: false`, null score fields, provider failure details, and `provider-failures.json` artifact.

## Origin

`docs/brainstorms/2026-05-21-brainstorm-smoke-test-field-report.md`.
