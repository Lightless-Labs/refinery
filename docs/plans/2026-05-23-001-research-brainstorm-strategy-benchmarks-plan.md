---
title: "research: brainstorm strategy benchmarks"
type: research
status: in_progress
date: 2026-05-23
todo: 013-brainstorm-strategy-benchmarks
---

# Brainstorm Strategy Benchmarks Plan

## Context

A valid v0 brainstorm baseline now exists in `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md` using Codex, GLM, Kimi, and MiniMax. That unblocks `todos/013`: define and run benchmarks for iteration, upstream divergence, and selection strategies.

## Problem

`brainstorm` v0 uses score-only iteration plus controversial selection. The smoke baseline shows this works, but it does not answer which strategy produces the best diverse panels across prompts and budgets.

Benchmarking needs a protocol before implementing variants; otherwise strategy comparisons will be anecdotal and budget-heavy.

## Goals

- Define benchmark levels from cheap offline analysis to expensive multi-run strategy comparisons.
- Specify prompt suite, metrics, artifact schema, and decision criteria.
- Run a first offline counterfactual analysis over the existing v0 baseline artifacts.
- Identify the minimal next implementation needed to compare iteration strategies.

## Non-Goals

- Do not implement prompt reframing (`todos/018`) in this milestone.
- Do not implement Open Collider/domain-collision expansion yet.
- Do not rely only on model-judge scores for final conclusions; include human-review hooks.
- Do not require fixing OpenCode concurrency (`todos/022`) before designing the benchmark; use `--max-concurrent 1` where necessary.

## Benchmark Levels

### L0 — Offline selection counterfactuals

Use existing run artifacts. Re-rank final-round candidates by alternative selection rules without making new provider calls.

Strategies:

- `mean`: highest mean score.
- `stddev`: highest evaluator disagreement.
- `controversy`: current `mean * stddev`.
- `quality_x_lexdiv`: greedy quality × lexical diversity heuristic.

### L1 — Repeated v0 baseline

Run v0 across a fixed prompt suite and multiple seeds/orders if/when determinism controls exist. This estimates variance before comparing strategies.

### L2 — Iteration strategy variants

Compare what models see between rounds:

- blind: prompt only each round,
- score-only: current v0,
- own+reviews: own prior answers plus evaluator rationale,
- full visibility: all answers/evals/scores,
- negative-only or cluster-label feedback if L0/L1 show diversity is weak.

### L3 — Upstream divergence expansion

Compare v0 against prompt-reframing expansion from `todos/018`; defer domain collisions until costs and L2 results justify them.

## Metrics

### Panel-level metrics

- `panel_mean_quality`: mean of panel candidates' mean scores.
- `panel_min_quality`: minimum panel candidate mean score.
- `panel_score_disagreement`: mean candidate stddev.
- `lexical_overlap`: average pairwise Jaccard similarity over normalized word sets; lower is more lexically diverse.
- `selector_delta`: which candidates are included/excluded by a selector compared to v0.
- `meta_preamble_rate`: fraction of panel answers that mention score/history/previous round mechanics.

### Optional judged metrics

Use model judges or human review to score the whole panel, not just individual answers:

- useful diversity,
- non-overlap,
- novelty,
- feasibility/actionability,
- panel coherence,
- best-single-answer regret: whether a high-quality answer was excluded for diversity.

## Prompt Suite

Start with 6–8 prompts spanning brainstorm use cases:

1. Product/strategy ideation — privacy-first knowledge assistant.
2. Technical/design — secretless multi-model brainstorm artifact format.
3. Architecture — design a plugin system for local AI tools with sandboxing.
4. Debugging/process — reduce flaky CI failures in a Rust monorepo.
5. Research/science — unconventional experiments for low-cost indoor air quality sensing.
6. Policy/operations — governance model for safely using AI agents in a small company.
7. Creative constraints — design a game mechanic teaching distributed systems concepts.
8. Market wedge — find non-obvious early adopters for privacy-preserving team memory.

## Budget Model

For `n=4`, `rounds=2`, one v0 run costs:

```text
calls_per_round = n + n(n - 1) = 16
total_calls = 32
```

A 6-prompt L1 baseline is 192 calls per repeat. Each additional iteration strategy adds another ~192 calls. Prompt-reframing expansion can multiply lineage count by `n + 1`, so it should not be run before L0/L1/L2 clarify which metrics matter.

Until `todos/022` is fixed, OpenCode-heavy panels should run with `--max-concurrent 1`.

## First Offline Analysis

Completed in `docs/brainstorms/2026-05-23-brainstorm-strategy-benchmark-design.md` using the two valid baseline artifacts.

## Next Implementation Step

Add an artifact-level benchmark/analyzer command or test fixture that can:

1. Load a brainstorm run directory.
2. Read final-round proposals and evaluations.
3. Emit selector counterfactuals and panel metrics as JSON.
4. Later aggregate multiple runs into a benchmark report.

This should be implemented before adding new strategy variants so every strategy can be measured with the same tooling.

## Verification

Documentation/research phase:

- Plan created.
- Baseline field report reviewed.
- Offline counterfactual metrics computed from existing artifacts.
- TODO updated with addendum.

Future implementation phase:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
