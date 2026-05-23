---
date: 2026-05-23
topic: brainstorm-strategy-benchmark-design
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
---

# Brainstorm Strategy Benchmark Design

## Executive Summary

The v0 brainstorm baseline is now strong enough to benchmark against. The benchmark should proceed in levels:

1. **L0 offline selector counterfactuals** over existing artifacts — cheap, immediate, no provider calls.
2. **L1 repeated v0 baseline** across a prompt suite — estimates natural variance.
3. **L2 iteration variants** — blind, score-only, own+reviews, full visibility.
4. **L3 upstream divergence expansion** — prompt reframing first; domain collisions only after budget is explicit.

The first L0 analysis over the two valid baseline runs shows that current controversial selection already behaves differently from mean-only selection and often improves lexical diversity by selecting MiniMax's divisive answers. However, it can also select lower-mean answers first, so panel-level quality controls should be measured before changing the selector.

## Inputs Used

Baseline report:

- `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md`

Artifact run dirs:

- Product: `target/brainstorm-smoke-2026-05-22/product-serial/20260523-060748_generate-unconventional-but-practical-pr_356f`
- Technical: `target/brainstorm-smoke-2026-05-22/technical-serial/20260523-062159_design-a-lightweight-artifact-format-for_ccb6`

## L0 Offline Selector Counterfactuals

L0 requires only final-round proposals and evaluation scores. It can compare selection strategies without rerunning providers.

### Candidate selectors

| Selector | Definition | Why test it |
|---|---|---|
| `mean` | Sort by mean evaluator score, descending | Quality baseline; likely less diverse |
| `stddev` | Sort by evaluator score standard deviation, descending | Pure disagreement baseline |
| `controversy` | Sort by `mean * stddev`, descending | Current v0 selector |
| `quality_x_lexdiv` | Greedy: start from highest mean, then choose high-quality low-overlap candidates | Cheap semantic-dedup approximation |

### Cheap diversity metric

Use average pairwise Jaccard similarity over normalized word sets:

```text
lexical_overlap(panel) = mean(|tokens(a) ∩ tokens(b)| / |tokens(a) ∪ tokens(b)|)
```

Lower is more lexically different. This is intentionally weak: it catches overlap and repetition but not conceptual equivalence. It is useful as a smoke metric, not a final judgment.

## L0 Results on Existing Baseline

### Product prompt

| Selector | Avg lexical overlap | Selected models |
|---|---:|---|
| `mean` | 0.122 | Codex, Kimi, GLM |
| `stddev` | 0.070 | MiniMax, Codex, Kimi |
| `controversy` | 0.070 | MiniMax, Codex, Kimi |
| `quality_x_lexdiv` | 0.070 | Codex, Kimi, MiniMax |

Interpretation:

- Mean-only would have excluded MiniMax and included GLM.
- Controversy included MiniMax because evaluator disagreement was high (`mean=7.33`, `stddev=0.94`).
- Lexical overlap improved from `0.122` to `0.070` when MiniMax entered the panel.
- `quality_x_lexdiv` selected the same set as controversy but ordered Codex first.

### Technical prompt

| Selector | Avg lexical overlap | Selected models |
|---|---:|---|
| `mean` | 0.109 | Codex, GLM, Kimi |
| `stddev` | 0.106 | MiniMax, Codex, Kimi |
| `controversy` | 0.106 | MiniMax, Codex, Kimi |
| `quality_x_lexdiv` | 0.109 | Codex, GLM, Kimi |

Interpretation:

- Controversy again selected MiniMax over GLM because MiniMax had high evaluator disagreement (`mean=6.67`, `stddev=1.25`).
- The cheap lexical metric barely changed (`0.109` vs `0.106`), so the argument for MiniMax is not lexical diversity alone.
- This is the first warning that final benchmark decisions need whole-panel human or model-judge assessment, not only Jaccard overlap.

## Benchmark Protocol

### Prompt suite

Start with 6 prompts, then expand to 8 after cost review:

1. Product/strategy ideation: privacy-first knowledge assistant.
2. Technical/design: secretless multi-model brainstorm artifact format.
3. Architecture: plugin system for local AI tools with sandboxing.
4. Debugging/process: reduce flaky CI failures in a Rust monorepo.
5. Research/science: low-cost indoor air quality sensing experiments.
6. Policy/operations: governance model for AI agents in a small company.

Optional prompts:

7. Creative/game design: teach distributed systems through game mechanics.
8. Market wedge: early adopters for privacy-preserving team memory.

### Fixed model panel

Use the currently available four-model panel:

```text
codex-cli/gpt-5.4
opencode/zai-coding-plan/glm-5.1
opencode/kimi-for-coding/kimi-k2-thinking
opencode/minimax-coding-plan/MiniMax-M2.5
```

Until OpenCode concurrency is fixed, run provider-heavy benchmarks with:

```sh
--max-concurrent 1
```

### Artifact requirements

Every benchmark run should save:

- command line and environment summary,
- prompt ID and strategy ID,
- model set,
- all round proposals,
- all evaluations,
- provider failures/degradation status,
- final panel,
- counterfactual selector outputs,
- panel-level metrics.

Canonical storage should remain JSON/JSONL. TOON can be evaluated later as a prompt-facing export format (`todos/021`).

## Metrics

### Required automated metrics

| Metric | Definition | Direction |
|---|---|---|
| `panel_mean_quality` | Mean of selected candidates' mean scores | Higher better |
| `panel_min_quality` | Minimum selected mean score | Higher better |
| `panel_disagreement` | Mean selected candidate stddev | Contextual |
| `lexical_overlap` | Average pairwise token Jaccard | Lower usually better |
| `selector_delta` | Set difference versus v0 controversy selector | Diagnostic |
| `degraded_rate` | Fraction of runs with provider failures | Lower better |
| `meta_preamble_rate` | Fraction mentioning score/history mechanics | Lower better |

### Required judged metrics

Automated metrics are insufficient for brainstorm panels. Add a whole-panel rubric:

1. **Useful diversity:** Are panel entries meaningfully different in approach, not just wording?
2. **Quality floor:** Is every panel entry worth a user reading?
3. **Novelty:** Does the panel surface non-obvious options?
4. **Actionability:** Can at least one entry be acted on now?
5. **Complementarity:** Do entries cover different trade-off surfaces?
6. **Regret:** Did the selector exclude an obviously better answer?

This can initially be human-scored on a 1–5 scale. A model judge can be added for scale, but should be calibrated against human review.

## Strategy Comparison Order

### Phase A — Selection-only benchmarks

No provider calls beyond existing v0 runs. Implement artifact analyzer and compare selectors across the prompt suite.

Decision to make:

- Keep `mean * stddev`?
- Add a quality floor before controversy ranking?
- Add semantic/lexical dedup as a second-stage constraint?

### Phase B — Iteration strategy benchmarks

Implement minimal variants behind internal config or a hidden benchmark command:

1. `blind` — prompt only each round.
2. `score-only` — current v0.
3. `own+reviews` — likely higher quality but lower divergence.
4. `full-visibility` — expected conformity baseline.

Measure whether score-only actually outperforms blind and own+reviews on useful diversity.

### Phase C — Upstream divergence benchmarks

Only after Phases A/B:

- Implement prompt-reframing from `todos/018` with strict budget caps.
- Compare against v0 on the same prompt suite.
- Defer domain collision until prompt reframing has clear evidence or a specific use case.

## Budget

For `n=4`, `rounds=2`:

```text
calls_per_round = n + n(n - 1) = 16
total_calls_per_prompt = 32
```

Costs by phase:

| Phase | Prompts | Strategies | Calls |
|---|---:|---:|---:|
| L1 v0 baseline | 6 | 1 | 192 |
| L2 iteration comparison | 6 | 4 | 768 |
| L3 prompt reframing | 6 | TBD | likely 5×+ v0 |

Because OpenCode currently needs serial execution, elapsed wall-clock is a real constraint. Fixing `todos/022` would make larger benchmarks much easier.

## Decisions / Recommendations

1. **Implement the artifact analyzer before adding new brainstorm strategies.** It will make every later run measurable.
2. **Keep v0 controversy selector for now.** L0 shows it changes panels in useful ways and improves product-prompt lexical diversity.
3. **Add a quality-floor experiment.** The technical run selected MiniMax first with `mean=6.67`; this may be acceptable for controversy but needs a guardrail in user-facing contexts.
4. **Treat lexical overlap as diagnostic only.** It did not capture the conceptual value of the technical panel well.
5. **Add whole-panel judging.** The final goal is panel usefulness, not individual answer score.
6. **Suppress score-history meta-preambles before polished demos.** They are measurable as a benchmark artifact quality issue.

## Implemented Analyzer

`refinery benchmark-brainstorm` now accepts one or more brainstorm run directories and emits selector counterfactuals plus panel metrics:

```sh
refinery benchmark-brainstorm path/to/run-dir --output-format json
```

The initial implementation compares:

- `mean`
- `stddev`
- `controversy`
- `quality_x_lexdiv`

It reports:

- selected panel members,
- `panel_mean_quality`,
- `panel_min_quality`,
- `panel_disagreement`,
- `lexical_overlap`,
- `meta_preamble_rate`.

## Next Concrete Step

Use the analyzer across the 6-prompt v0 baseline suite, then add minimal benchmark-only iteration variants (`blind`, `score-only`, `own+reviews`, `full-visibility`) so the same analyzer can compare their final panels.
