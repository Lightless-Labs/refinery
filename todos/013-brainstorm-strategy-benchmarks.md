---
title: "research: benchmark iteration and selection strategies for brainstorm verb"
priority: low
milestone: v0.4
depends_on: 004-verb-brainstorm
status: in_progress
updated: 2026-05-31
---

# Benchmark: Brainstorm Iteration and Selection Strategies

**Plan:** `docs/plans/2026-05-23-001-research-brainstorm-strategy-benchmarks-plan.md`

**Phase 1 deliverable:** `docs/brainstorms/2026-05-23-brainstorm-strategy-benchmark-design.md`

**Phase 2 deliverable:** `refinery benchmark-brainstorm` artifact analyzer

**Phase 3 deliverable:** `docs/brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md`

## Goal

After brainstorm v0 ships (score-only iteration + controversial selection), benchmark alternative strategies on both axes to find what actually produces the best diverse panels.

## Iteration Strategies to Benchmark

What models see between rounds:

1. **Score-only** (v0 baseline) — prompt + own prior answers + scores only
2. **Own+reviews** (converge/synthesize today) — prompt + own prior answers + evaluations (scores + rationale + suggestions)
3. **Full visibility** — everything: all models' answers, all evaluations, all scores. Risk: conformity.
4. **Cluster labels** — prompt + topic summaries of what exists ("3 answers about cost, 2 about culture — go elsewhere"). Risk: shallow diversity.
5. **Negative-only** — prompt + list of taken topics ("these topics are taken: ..."). Risk: over-constraining.
6. **Blind** — prompt only, no context from prior rounds. Pure independent generation. Baseline for comparison.
7. **Similarity-based** — measure proximity between answers (TF-IDF, Jaccard, embeddings?) and use that signal somehow. Explore: as iteration feedback? As selection input? As post-processing dedup?

## Upstream Divergence Strategies to Benchmark

What lineages exist before score-only iteration begins:

1. **No expansion** (v0 baseline) — each model works the original prompt only.
2. **Prompt reframing expansion** — each model generates one strategically different version of the initial prompt; all models work on the original plus every variation. For `n` models this yields `n(n + 1)` lineages.
3. **Domain collision expansion** — generate structurally distant domains and run isolated brief × domain collisions, inspired by Open Collider / bisociation.
4. **Prompt reframing × domain collision** — combine both expansions: `n models × (1 original + p prompt variations) × d domains`. Expensive; requires budget controls.

Prompt variations should not be paraphrases. They should alter assumptions, success criteria, time horizon, stakeholder, causal model, constraints, abstraction level, risk appetite, or second-order-effect framing.

## Selection Strategies to Benchmark

1. **Controversial** (v0 baseline) — high quality + high evaluator disagreement
2. **Score variance** — keep answers with high standard deviation across evaluators
3. **Semantic deduplication** — cluster answers, keep one representative per cluster
4. **Model-as-judge diversity** — ask a model to assess pairwise diversity
5. **Combined** — controversy + diversity as a composite score

## Benchmark Design

Initial design completed 2026-05-23. Use a staged protocol:

1. **L0 offline selector counterfactuals** over existing artifacts.
2. **L1 repeated v0 baseline** across a fixed prompt suite.
3. **L2 iteration strategy variants** (blind, score-only, own+reviews, full visibility).
4. **L3 upstream divergence expansion** (prompt reframing first; domain collisions only after budget review).

Panel quality should combine:

- automated quality/score metrics,
- lexical/semantic diversity diagnostics,
- degradation/provider-failure rates,
- meta-preamble/noise rates,
- whole-panel human or calibrated model-judge review for useful diversity, actionability, novelty, and regret.

First offline counterfactual on the valid 2026-05-23 baseline found that controversy selection differs meaningfully from mean-only selection and often includes MiniMax's high-disagreement answers.

The artifact analyzer is now implemented as `refinery benchmark-brainstorm` and has been used across a 6-prompt v0 baseline suite.

Key benchmark result: raw controversy improves lexical diversity but can select low-quality high-disagreement answers. `controversy_floor_7` improved the quality floor while preserving some diversity benefit. Follow-ups created:

- `todos/023-brainstorm-quality-floor-selection.md` (completed 2026-05-24)
- `todos/024-brainstorm-suppress-score-history-meta-preambles.md` (completed 2026-05-25)

## Remaining Work

Immediate quality follow-ups are now complete:

- `todos/023-brainstorm-quality-floor-selection.md` added/configured production quality-floor selection.
- `todos/024-brainstorm-suppress-score-history-meta-preambles.md` reduced measured score-history meta-preambles to `0.0` on two validation prompts.

Benchmark-only iteration variants are now implemented behind hidden/internal CLI config:

- `blind` — prompt-only every round.
- `score-only` — production default, own prior answers plus aggregate scores.
- `own-reviews` — own prior answers plus received peer scores and rationales.
- `full-visibility` — all prior answers plus peer scores and rationales.

The default production behavior remains score-only. Brainstorm outputs and artifact `metadata.json` now expose `iteration_strategy`, and `refinery benchmark-brainstorm` reads that metadata for grouping.

Implementation PR #40 passed GitHub Actions, Buildkite, CodeRabbit, Gemini, and Codex review after follow-up fixes for nested model-spec validation, Clippy sort linting, string score parsing, Pi credential environment passthrough, and Windows `USERPROFILE` preservation.

Next concrete step: run the fixed six-prompt suite for each variant using Pi-backed model routing where possible. If OpenCode-backed models are included from local config, serialize them with `--max-concurrent 1` until `todos/022` is fixed. Compare selectors (`mean`, `controversy`, `controversy_floor_7`, `quality_x_lexdiv`) and whole-panel metrics before promoting any variant to public UX.

## References

- Brainstorm verb: `todos/004-verb-brainstorm.md`
- Divergence expansion: `todos/018-brainstorm-divergence-expansion.md`
- Open Collider: `https://github.com/CL-ML/open-collider`
