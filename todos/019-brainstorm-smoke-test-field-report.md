---
title: "test: run brainstorm smoke tests and capture field report"
priority: medium
milestone: v0.3
depends_on: 004-verb-brainstorm
created: 2026-05-21
status: completed
completed: 2026-05-21
---

# Brainstorm Smoke Tests and Field Report

## Goal

Run `refinery brainstorm` on real prompts now that PR #28 is merged, then document whether the v0 design actually produces useful divergent panels.

This should happen before implementing prompt reframing or Open Collider-style domain collisions, so future divergence work has a baseline.

## Suggested Test Matrix

Use at least two prompts:

1. A product/strategy ideation prompt where originality matters.
2. A technical/design prompt where feasibility and depth matter.

For each prompt, run:

```sh
refinery brainstorm "..." --models claude-code,codex-cli,gemini-cli --max-rounds 2 --panel-size 3
```

If provider credentials or local CLIs are unavailable, record the blocker explicitly instead of forcing implementation changes.

## What to Observe

- Does score-only iteration preserve visibly different answer lineages?
- Does controversial selection pick interesting answers, or just noisy ones?
- Are panel entries meaningfully non-overlapping?
- Does the originality/insight/depth/feasibility rubric produce sensible scores?
- Are progress output, JSON output, and artifact output understandable?
- Are there obvious UX papercuts before strategy benchmarking?

## Deliverable

Completed in `docs/brainstorms/2026-05-21-brainstorm-smoke-test-field-report.md`.

The first run generated follow-up `todos/020-brainstorm-provider-failure-observability.md` because provider failures can make a requested multi-provider brainstorm look like a successful single-provider run.

A later valid four-model baseline was completed in `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md` using Codex, GLM, Kimi, and MiniMax with `--max-concurrent 1`.

Original deliverable requirements:

- commands run
- models used
- short excerpts or summaries of panel outputs
- what worked
- what failed or felt weak
- concrete TODOs generated from the run

## Follow-ups This Should Inform

- `todos/013-brainstorm-strategy-benchmarks.md`
- `todos/017-brainstorm-p3-nitpicks.md`
- `todos/018-brainstorm-divergence-expansion.md`
