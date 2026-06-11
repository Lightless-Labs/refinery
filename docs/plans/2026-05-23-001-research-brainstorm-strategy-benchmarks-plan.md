---
title: "research: brainstorm strategy benchmarks"
type: research
status: in_progress
date: 2026-05-23
todo: 013-brainstorm-strategy-benchmarks
---

# Brainstorm Strategy Benchmarks Plan

**Enhanced:** 2026-05-23 (via `/deepen-plan`)
**Reviewed:** 2026-05-31 (via `/coderabbit / review`)
**Completed:** TBD
**Addendum:** 2026-05-30 — L2 iteration strategy suite completed with Pi-backed model routing; see `docs/brainstorms/2026-05-30-brainstorm-l2-iteration-strategy-benchmark.md`.
**Addendum:** 2026-05-30 — Added blind panel review pack generator (`refinery review-brainstorm-panels`) and generated the first L2 review packet.
**Addendum:** 2026-06-01 — Completed first-pass qualitative L2 panel review; see `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`.
**Addendum:** 2026-06-05 — Ran an updated-model L3 smoke with `pi/kimi-coding/kimi-for-coding:off` and `pi/minimax/MiniMax-M3:off`; see `docs/brainstorms/2026-06-05-brainstorm-l3-updated-model-smoke.md`.
**Addendum:** 2026-06-09 — Ran a two-prompt three-model L3 comparison (`off` vs `per-model`) with Codex, GLM, and Kimi-for-coding; see `docs/brainstorms/2026-06-09-brainstorm-l3-three-model-sample.md`.
**Addendum:** 2026-06-11 — Completed a verified brainstorm evaluation parser hardening pass for GLM-style invalid-score failures: score parsing now accepts scaled score text, nested score objects, `overall_score`, and a missing-overall fallback to the four required dimension scores. Targeted parser/brainstorm tests and `refinery_core` clippy passed.

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

## Implementation Progress

### Completed 2026-05-23

Added `refinery benchmark-brainstorm` as the artifact-level analyzer. It can:

1. Load one or more brainstorm run directories.
2. Read final-round proposals and evaluations.
3. Emit selector counterfactuals and panel metrics as JSON or text.
4. Compare `mean`, `stddev`, `controversy`, `controversy_floor_7`, and `quality_x_lexdiv` selectors.

This gives future strategy variants a shared measurement path.

### Completed 2026-05-26

Added benchmark-only brainstorm iteration variants behind a hidden CLI flag:

- `blind` — prompt-only every round.
- `score-only` — production default, own prior answers plus aggregate scores.
- `own-reviews` — own prior answers plus received peer scores and rationales.
- `full-visibility` — all prior answers plus peer scores and rationales.

The production default remains `score-only`. Brainstorm JSON/text output and dry-run output now expose `iteration_strategy`. Artifact runs now write `metadata.json` with `iteration_strategy`, and `refinery benchmark-brainstorm` reads that metadata so benchmark outputs can group runs by iteration strategy. Evaluation artifacts now include `rationale` while remaining backwards compatible with the analyzer's score loading.

### Completed 2026-05-30

Ran the fixed six-prompt benchmark suite across all four L2 iteration variants with Pi-backed model routing:

- `blind`
- `score-only`
- `own-reviews`
- `full-visibility`

Final clean comparison uses 24 non-degraded, peer-evaluated runs (6 prompts × 4 strategies). The current production-like `controversy_floor_7` selector showed:

| Iteration strategy | Mean quality | Min quality | Disagreement | Lexical overlap |
|---|---:|---:|---:|---:|
| `blind` | 7.926 | 7.444 | 0.477 | 0.105 |
| `score-only` | 7.889 | 7.500 | 0.464 | 0.097 |
| `own-reviews` | 8.019 | 7.667 | 0.517 | 0.112 |
| `full-visibility` | 8.204 | 7.944 | 0.431 | 0.132 |

`full-visibility` had the highest score quality but also the highest lexical overlap, matching the expected conformity risk. `score-only` preserved the lowest lexical overlap and remains the recommended default until whole-panel diversity review says otherwise. `own-reviews` is the strongest middle-ground challenger.

Operationally, the clean Pi-backed suite needed `--max-concurrent 1` to avoid local config lock contention and a larger bounded stdout capture because Pi JSON event streams can exceed 1MB while streaming normal benchmark-sized answers.

### Completed 2026-06-01

Completed a first-pass qualitative panel review using the generated blind L2 review pack. The review compared `score-only`, `own-reviews`, and `full-visibility` panels on useful diversity, non-overlap, novelty, actionability, coverage, overall panel value, and best-answer regret.

Result: `score-only` remained strongest on useful diversity/non-overlap; `full-visibility` was strongest on actionability/coverage; `own-reviews` did not dominate overall but produced the strongest debugging/process panel. The review does not justify changing the production default away from `score-only` yet. Treat `score-only` as the L3 baseline, include `own-reviews` as an optional challenger if budget allows, and defer public exposure of iteration strategy variants until stronger human/calibrated judge evidence exists.

## Next Implementation Step

Continue `todos/013` with either a human/calibrated model-judge pass over `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`, 2-4 more L3 prompts with the Codex/GLM/Kimi-for-coding panel, or hardening/triage for GLM invalid evaluation scores on expanded prompt-reframing runs. The 2026-06-05 updated-model smoke showed Kimi-for-coding and MiniMax M3 are available through Pi, but MiniMax M3 can dominate runtime and timed out on one expanded product-prompt lineage; do not launch a full 4-model × 6-prompt L3 suite until latency/output budget controls are explicit. The 2026-06-09 three-model sample showed promising quality-floor/disagreement gains for `per-model`, but both expanded runs degraded due to evaluator failures. Do not change the production default based on the first-pass L2 review or small L3 samples alone.

## Verification

Completed:

- Plan created.
- Baseline field report reviewed.
- Offline counterfactual metrics computed from existing artifacts.
- Artifact analyzer implemented as `refinery benchmark-brainstorm`.
- Analyzer run against the two valid 2026-05-23 baseline artifacts and the later six-prompt suite.
- Benchmark-only iteration variants implemented behind hidden CLI config.
- L2 six-prompt suite run across `blind`, `score-only`, `own-reviews`, and `full-visibility`; analyzer outputs saved under `target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/`.
- Blind panel review pack generator added as `refinery review-brainstorm-panels`; L2 review pack and answer key generated under the same logs directory.
- First-pass L2 panel review documented in `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`.
- Updated-model L3 smoke documented in `docs/brainstorms/2026-06-05-brainstorm-l3-updated-model-smoke.md`.
- Three-model L3 sample documented in `docs/brainstorms/2026-06-09-brainstorm-l3-three-model-sample.md`.
- `cargo fmt --all -- --check`
- `cargo test -p refinery_core brainstorm`
- `cargo test -p refinery_cli`
- `cargo clippy -p refinery_core --all-targets -- -D warnings`
- `cargo clippy -p refinery_cli --all-targets -- -D warnings`
