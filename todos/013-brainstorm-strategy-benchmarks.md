---
title: "research: benchmark iteration and selection strategies for brainstorm verb"
priority: low
milestone: v0.4
depends_on: 004-verb-brainstorm
status: in_progress
updated: 2026-06-11
---

# Benchmark: Brainstorm Iteration and Selection Strategies

**Plan:** `docs/plans/2026-05-23-001-research-brainstorm-strategy-benchmarks-plan.md`

**Phase 1 deliverable:** `docs/brainstorms/2026-05-23-brainstorm-strategy-benchmark-design.md`

**Phase 2 deliverable:** `refinery benchmark-brainstorm` artifact analyzer

**Phase 3 deliverable:** `docs/brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md`

**Phase 4 deliverable:** `docs/brainstorms/2026-05-30-brainstorm-l2-iteration-strategy-benchmark.md`

**Phase 5 deliverable:** `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`

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

The fixed six-prompt suite has now been run for all four L2 variants with Pi-backed model routing. Clean result: 24 non-degraded, peer-evaluated runs. Aggregate `controversy_floor_7` view: `full-visibility` scored highest on mean/min quality but had highest lexical overlap; `score-only` had the lowest lexical overlap but lower judged quality; `own-reviews` is the most interesting middle-ground challenger.

A first-pass qualitative review over the generated blind panel review pack is complete. Result: `score-only` still looked strongest on useful diversity and non-overlap; `full-visibility` looked strongest on actionability and coverage; `own-reviews` did not dominate globally but produced the strongest debugging/process panel. Keep production default as `score-only` until stronger human/calibrated model-judge evidence says otherwise.

Latest L3 three-model sample is documented in `docs/brainstorms/2026-06-09-brainstorm-l3-three-model-sample.md`. It compared `--prompt-variants off` vs `per-model` on product and technical prompts with Codex, GLM, and Kimi-for-coding. Per-model improved two-prompt `controversy_floor_7` average quality floor (`7.00` → `8.00`) and disagreement (`0.33` → `0.75`), but both per-model runs degraded due to evaluation issues (GLM invalid eval scores; one Codex SSE header timeout), so it cannot support default changes.

A verified parser hardening pass for recoverable GLM-style invalid evaluation scores completed on 2026-06-11. Brainstorm evaluation parsing now accepts scaled score text, nested score objects, `overall_score`, and a missing-overall fallback to the four required dimension scores while rejecting incomplete dimension sets. Verified with targeted parser tests, `cargo test -p refinery_core brainstorm -q`, and `cargo clippy -p refinery_core --all-targets -- -D warnings`.

A live post-hardening L3 validation on 2026-06-12 is documented in `docs/brainstorms/2026-06-12-brainstorm-l3-parser-validation.md`. It still degraded: baseline had one GLM invalid eval score, while expanded prompt-reframing had Kimi overload/rate-limit failures plus one Kimi invalid eval score. This does not prove a remaining score-shape parser gap; the next code step is evidence capture to distinguish malformed/empty/provider output from genuinely unhandled score JSON.

Next concrete step: rerun a small L3 validation and inspect bounded `response_preview` fields for any invalid structured-response parse failures, or run a human/calibrated model-judge pass over the L2/L3 panel findings. For L3, use `score-only` as the baseline, treat `own-reviews` as optional, and avoid launching a full 4-model × 6-prompt suite with MiniMax M3 until latency/output budget controls are explicit.

## References

- Brainstorm verb: `todos/004-verb-brainstorm.md`
- Divergence expansion: `todos/018-brainstorm-divergence-expansion.md`
- Open Collider: `https://github.com/CL-ML/open-collider`
