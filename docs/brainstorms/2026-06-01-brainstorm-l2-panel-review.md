---
date: 2026-06-01
topic: brainstorm-l2-panel-review
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
review_artifacts:
  pack: target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-pack.md
  key: target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-key.json
---

# Brainstorm L2 Panel Review

## Summary

Performed a first-pass qualitative review using the blind L2 brainstorm panel review pack generated from the six-prompt Pi-backed benchmark. The review compared the `score-only`, `own-reviews`, and `full-visibility` iteration strategies using panels selected by `controversy_floor_7`.

This is **not a replacement for a human panel or calibrated model-judge study**. It is a lightweight agent review to check whether the automated metrics from `docs/brainstorms/2026-05-30-brainstorm-l2-iteration-strategy-benchmark.md` are directionally plausible before changing defaults.

Main result:

- `score-only` still looked strongest on useful diversity and non-overlap.
- `full-visibility` looked strongest on actionability and coverage.
- `own-reviews` did not dominate overall, but produced the best debugging/process panel.
- No reviewed evidence justifies changing the production default away from `score-only` yet.

## Review Method

Input artifacts:

```text
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-pack.md
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-key.json
```

The review pack hides model IDs and iteration strategies in the reviewer-facing output. I used the reviewer-facing panel labels for scoring and then applied the answer key for strategy-level aggregation. Treat this as a first-pass agent review rather than a rigorously blinded human study.

Scoring dimensions used the pack's 1-5 rubric:

- useful diversity,
- non-overlap,
- novelty,
- actionability,
- coverage,
- overall panel value,
- best-answer regret / omissions.

## Prompt-Level Scores

| Prompt | Blind panel | Strategy | Useful diversity | Non-overlap | Novelty | Actionability | Coverage | Overall | Best-answer regret / omissions |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| architecture | A | `own-reviews` | 4 | 3 | 5 | 2 | 4 | 3 | Yes — highly imaginative, but too much abstract trust ecology and not enough implementable baseline architecture. |
| architecture | B | `full-visibility` | 4 | 4 | 5 | 3 | 5 | 4 | No major omission; good breadth across intent VM, codebase-as-oracle, and prompt-influence risk. |
| architecture | C | `score-only` | 4 | 4 | 4 | 4 | 5 | 4 | No major omission; best balance of strong sandbox model plus practical capability checkpoints. |
| debugging | A | `full-visibility` | 3 | 3 | 4 | 4 | 4 | 4 | Minor — duplicated determinism-contract ideas across answers. |
| debugging | B | `score-only` | 4 | 4 | 4 | 4 | 4 | 4 | No major omission; good technical/process spread, but one answer was less complete. |
| debugging | C | `own-reviews` | 4 | 4 | 4 | 5 | 5 | 5 | No major omission; strongest actionable panel across test ABI, failure capsules, isolation, and time-travel replay. |
| governance | A | `own-reviews` | 3 | 3 | 4 | 4 | 5 | 4 | No major omission, but stewardship/reversibility themes overlap. |
| governance | B | `score-only` | 4 | 4 | 4 | 3 | 3 | 3 | Yes — diverse mechanisms, but some are underdeveloped or gimmicky compared with concrete governance lifecycle models. |
| governance | C | `full-visibility` | 4 | 4 | 4 | 4 | 5 | 4 | No major omission; best balanced governance surface across runtime, intent, and risk-budget controls. |
| product | A | `own-reviews` | 5 | 5 | 4 | 3 | 4 | 4 | No major omission; broad set of wedges, though some are farther from a focused PKM startup wedge. |
| product | B | `score-only` | 5 | 5 | 5 | 3 | 4 | 4 | No major omission; very high novelty, but practicality varies across intellectual immune system, executor mode, and ZK recall. |
| product | C | `full-visibility` | 4 | 4 | 4 | 4 | 4 | 4 | Minor — coherent and practical, but over-indexes crisis/caregiving/threat scenarios. |
| research | A | `own-reviews` | 5 | 5 | 5 | 3 | 5 | 4 | No major omission; extremely novel, but includes some fragile biology/physics experiments. |
| research | B | `full-visibility` | 4 | 4 | 4 | 4 | 5 | 4 | No major omission; strong practical coverage, with some overlap around acoustic/resonance methods. |
| research | C | `score-only` | 5 | 5 | 5 | 4 | 5 | 5 | No major omission; best mix of perturbation experiments, soap-film sensing, and bio-integrative approaches. |
| technical | A | `full-visibility` | 3 | 3 | 4 | 4 | 4 | 4 | No major omission; strongest schema/format panel, though answers share the same secretless-shape motif. |
| technical | B | `score-only` | 3 | 3 | 3 | 2 | 2 | 3 | Yes — several entries are too short or schema-stub-like to be directly useful. |
| technical | C | `own-reviews` | 2 | 2 | 3 | 3 | 3 | 3 | Yes — one detailed schema is useful, but the panel is not diverse enough and includes very short answers. |

## Strategy-Level Aggregate

Averages across six prompts.

| Strategy | Useful diversity | Non-overlap | Novelty | Actionability | Coverage | Overall |
|---|---:|---:|---:|---:|---:|---:|
| `score-only` | 4.17 | 4.17 | 4.17 | 3.33 | 3.83 | 3.83 |
| `own-reviews` | 3.83 | 3.67 | 4.17 | 3.33 | 4.33 | 3.83 |
| `full-visibility` | 3.67 | 3.67 | 4.17 | 3.83 | 4.50 | 4.00 |

Prompt wins by overall qualitative panel value:

| Prompt | Best panel(s) | Notes |
|---|---|---|
| architecture | `score-only`, `full-visibility` | `score-only` had the best practicality/diversity balance; `full-visibility` had slightly broader threat coverage. |
| debugging | `own-reviews` | The most actionable panel in the review. |
| governance | `full-visibility`, `own-reviews` | `full-visibility` was cleaner; `own-reviews` was also viable. |
| product | all roughly tied | `score-only` and `own-reviews` were more divergent; `full-visibility` was more coherent/practical. |
| research | `score-only` | Strongest combination of novelty, non-overlap, and testability. |
| technical | `full-visibility` | Most complete artifact-format panel; both other strategies had thin schema stubs. |

## Findings

### 1. Full visibility's higher score quality was not just evaluator bias

The automated benchmark found `full-visibility` had the highest mean/min quality and highest lexical overlap. The qualitative review mostly agrees: full visibility often produced panels with better coverage, clearer integration, and more immediately usable proposals.

This was especially visible in governance and technical artifact-format prompts, where seeing the whole prior round seems to help models fill obvious gaps and converge on complete answer shapes.

### 2. Score-only still best matches the brainstorm diversity promise

`score-only` had the highest reviewed useful-diversity and non-overlap averages. It produced the strongest research panel and a strong architecture panel. Its weaker results were not because it lacked imagination; rather, some panels contained underdeveloped or overly short answers.

For a verb whose public promise is divergent ideation rather than best single answer synthesis, `score-only` remains the safest default.

### 3. Own-reviews is not an obvious default, but it is a useful challenger

`own-reviews` produced the best debugging/process panel and generally good coverage. However, it also produced the weakest architecture actionability and a weak technical panel. The review does not support promoting it over `score-only` or `full-visibility` globally.

It may still be worth keeping as a hidden benchmark variant or eventual advanced option because it can improve practical refinement without fully exposing peer answers.

### 4. Lexical overlap underestimates some semantic convergence risks

`full-visibility` did not collapse into identical answers, but the review found repeated framing motifs in several panels: secretless artifact schemas, runtime/governance controls, and determinism contracts. These are not always harmful; sometimes they are the obvious correct abstractions. But they confirm that automated lexical overlap should be treated as a weak proxy, not a final diversity measure.

### 5. Best-answer regret was concentrated in thin or over-abstract panels

Major regret cases were mostly:

- panels with beautiful but hard-to-implement abstractions,
- schema stubs too short to act on,
- panels where all answers leaned into the same framing.

This suggests a future selection metric should penalize answer underdevelopment and panel redundancy, not just low individual mean scores.

## Recommendation

Keep the production default as `score-only`.

Do not promote `full-visibility` as the default yet. It is a credible option for quality/coverage-oriented brainstorming, but the public `brainstorm` verb should continue prioritizing independent divergence until a larger human/model-judge panel confirms users prefer the higher-coverage, higher-overlap trade-off.

For the next benchmark phase:

1. Treat `score-only` as the L3 baseline.
2. Include `own-reviews` as the most interesting challenger only if budget allows.
3. Start L3 with prompt-reframing expansion from `todos/018`; continue deferring domain-collision/Open Collider variants until budget constraints are explicit.
4. Consider a future selector or reranker that adds a panel-level redundancy penalty and answer-completeness floor.
