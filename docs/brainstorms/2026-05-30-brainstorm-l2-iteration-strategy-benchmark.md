---
date: 2026-05-30
topic: brainstorm-l2-iteration-strategy-benchmark
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
---

# Brainstorm L2 Iteration Strategy Benchmark

## Summary

Ran the fixed six-prompt brainstorm benchmark suite across the four hidden/internal iteration strategies:

- `blind`
- `score-only`
- `own-reviews`
- `full-visibility`

Final clean comparison uses 24 non-degraded, peer-evaluated runs: 6 prompts × 4 strategies. Each run used 4 models, 2 rounds, and 32 provider calls.

High-level result: **`full-visibility` produced the highest judged quality but also the highest lexical overlap**. **`score-only` produced the lowest lexical overlap**, but not the best judged quality. `own-reviews` landed between them and had the highest disagreement under controversy selection.

The result is not yet enough to change public UX because lexical overlap is only a cheap diversity proxy and full visibility may create semantic convergence not captured by score metrics alone. The next step should be whole-panel human/model-judge review on the saved artifacts.

## Model Panel

Pi-backed model routing was used for all final clean runs:

```text
pi/openai-codex/gpt-5.4:off
pi/zai/glm-5.1:off
pi/kimi-coding/kimi-k2-thinking:off
pi/minimax/MiniMax-M2.7:off
```

Notes:

- `:off` disables Pi model thinking output for benchmark stability and cost/latency control.
- Runs used `--max-concurrent 1` to avoid Pi local config lock contention observed during an initial concurrent attempt.
- `tundish_providers` stdout capture was raised from 1MB to 64MB because Pi JSON event streams can be much larger than the final assistant text.

## Prompt Suite

Same six-prompt suite as the 2026-05-23 benchmark:

1. Product/strategy — privacy-first personal knowledge assistant.
2. Technical/design — secretless multi-model brainstorm artifact format.
3. Architecture — local AI coding tool plugin system with sandboxing.
4. Debugging/process — reduce flaky CI failures in a Rust/Bazel monorepo.
5. Research/science — low-cost indoor air quality experiments.
6. Governance/operations — lightweight governance for AI coding agents.

## Commands

Representative run command:

```sh
cargo run -q -p refinery_cli -- brainstorm "$PROMPT" \
  --models pi/openai-codex/gpt-5.4:off,pi/zai/glm-5.1:off,pi/kimi-coding/kimi-k2-thinking:off,pi/minimax/MiniMax-M2.7:off \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --iteration-strategy "$STRATEGY" \
  --output-dir target/brainstorm-benchmark-2026-05-29-l2-pi-serial/$STRATEGY/$PROMPT_SLUG \
  --output-format json \
  --verbose \
  --idle-timeout 480 \
  --timeout 1800 \
  --max-concurrent 1
```

Analyzer command:

```sh
cargo run -q -p refinery_cli -- benchmark-brainstorm $(cat target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/run-dirs-clean.txt) \
  --output-format json > target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-analysis-clean.json

cargo run -q -p refinery_cli -- benchmark-brainstorm $(cat target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/run-dirs-clean.txt) \
  --output-format text > target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-analysis-clean.txt
```

## Artifact Locations

```text
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/run-dirs-clean.txt
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-analysis-clean.json
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-analysis-clean.txt
```

The clean run list selects the latest successful run for rerun prompts that initially had a single invalid MiniMax evaluation score.

## Aggregate Selector Results

Averages across six prompts per strategy.

| Iteration strategy | Selector | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---|---:|---:|---:|---:|---:|
| `blind` | `mean` | 8.000 | 7.667 | 0.398 | 0.103 | 0.000 |
| `blind` | `stddev` | 7.926 | 7.444 | 0.477 | 0.105 | 0.000 |
| `blind` | `controversy` | 7.926 | 7.444 | 0.477 | 0.105 | 0.000 |
| `blind` | `controversy_floor_7` | 7.926 | 7.444 | 0.477 | 0.105 | 0.000 |
| `blind` | `quality_x_lexdiv` | 8.000 | 7.667 | 0.398 | 0.103 | 0.000 |
| `score-only` | `mean` | 7.981 | 7.667 | 0.386 | 0.100 | 0.000 |
| `score-only` | `stddev` | 7.889 | 7.500 | 0.464 | 0.097 | 0.000 |
| `score-only` | `controversy` | 7.889 | 7.500 | 0.464 | 0.097 | 0.000 |
| `score-only` | `controversy_floor_7` | 7.889 | 7.500 | 0.464 | 0.097 | 0.000 |
| `score-only` | `quality_x_lexdiv` | 7.981 | 7.667 | 0.386 | 0.100 | 0.000 |
| `own-reviews` | `mean` | 8.111 | 7.889 | 0.386 | 0.109 | 0.000 |
| `own-reviews` | `stddev` | 8.019 | 7.667 | 0.517 | 0.112 | 0.000 |
| `own-reviews` | `controversy` | 8.019 | 7.667 | 0.517 | 0.112 | 0.000 |
| `own-reviews` | `controversy_floor_7` | 8.019 | 7.667 | 0.517 | 0.112 | 0.000 |
| `own-reviews` | `quality_x_lexdiv` | 8.093 | 7.833 | 0.360 | 0.103 | 0.000 |
| `full-visibility` | `mean` | 8.222 | 8.000 | 0.405 | 0.134 | 0.000 |
| `full-visibility` | `stddev` | 8.204 | 7.944 | 0.431 | 0.132 | 0.000 |
| `full-visibility` | `controversy` | 8.204 | 7.944 | 0.431 | 0.132 | 0.000 |
| `full-visibility` | `controversy_floor_7` | 8.204 | 7.944 | 0.431 | 0.132 | 0.000 |
| `full-visibility` | `quality_x_lexdiv` | 8.204 | 7.944 | 0.386 | 0.128 | 0.000 |

## Production-Selector View

For the current production-like selector, `controversy_floor_7`:

| Iteration strategy | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---:|---:|---:|---:|---:|
| `blind` | 7.926 | 7.444 | 0.477 | 0.105 | 0.000 |
| `score-only` | 7.889 | 7.500 | 0.464 | 0.097 | 0.000 |
| `own-reviews` | 8.019 | 7.667 | 0.517 | 0.112 | 0.000 |
| `full-visibility` | 8.204 | 7.944 | 0.431 | 0.132 | 0.000 |

## Findings

### 1. Full visibility wins on score quality but loses on lexical diversity

`full-visibility` had the highest aggregate mean quality and minimum quality for every selector. Under `controversy_floor_7`, it averaged `8.204` mean quality and `7.944` minimum quality.

However, its lexical overlap was also highest (`0.132` under `controversy_floor_7`). This supports the expected conformity risk: seeing all prior answers may help models refine toward evaluator-preferred answers, but it may also narrow the panel.

### 2. Score-only remains the strongest cheap-diversity baseline

`score-only` had the lowest lexical overlap under controversy-based selectors (`0.097`), but it also had the lowest aggregate quality in this run (`7.889` mean quality under `controversy_floor_7`). It still looks like the safest default if the product promise emphasizes independent divergent ideation.

### 3. Own-reviews is a plausible middle ground

`own-reviews` improved quality versus `score-only` and `blind`, and it had the highest panel disagreement under controversy selection (`0.517`). Lexical overlap was higher than `score-only` but lower than `full-visibility`. This may be the best candidate to inspect manually because it gives each model actionable critique without exposing all competing answers.

### 4. Blind generation is not clearly better than score-only

`blind` was competitive but did not dominate. It had slightly better quality than `score-only` under controversy selection in this run, but worse lexical overlap. Because blind iteration ignores useful score pressure, it is not an obvious replacement.

### 5. Meta-preamble polish held across all variants

Every aggregate row reported `meta_preamble_rate: 0.000`. The 2026-05-25 prompt polish appears robust across the L2 iteration strategies.

### 6. The quality floor did not alter aggregate selector sets here

In this Pi-backed run, `controversy` and `controversy_floor_7` produced identical aggregate metrics for every strategy. The candidate pool was generally above the floor; this differs from the 2026-05-23 OpenCode-heavy baseline where raw controversy selected several low-quality, high-disagreement answers.

## Operational Notes

An initial Pi-backed concurrent run with `--max-concurrent 4` produced two classes of failures:

1. Pi local config lock contention, surfaced as transient provider credential/config errors.
2. `ResponseTooLarge` failures from Pi JSON event streams exceeding the previous 1MB stdout cap.

The clean benchmark used `--max-concurrent 1` and a 64MB bounded stdout capture. A future provider improvement should stream-parse Pi JSON events instead of retaining the whole event stream.

## Blind Panel Review Pack

Added `refinery review-brainstorm-panels` to generate a blind review packet from brainstorm artifact directories. The command hides iteration strategies and model IDs in the reviewer-facing output, while writing a separate JSON answer key for later analysis.

Generated the L2 review pack for `score-only`, `own-reviews`, and `full-visibility` panels selected by `controversy_floor_7`:

```text
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-pack.md
target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-key.json
```

The Markdown packet asks reviewers to score each panel on useful diversity, non-overlap, novelty, actionability, coverage, overall panel value, and best-answer regret.

A first-pass qualitative review is documented in `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`. It found `score-only` strongest on useful diversity/non-overlap and `full-visibility` strongest on actionability/coverage, with no justification to change the production default yet.

## Recommendations

1. Use the blind review pack to compare `score-only`, `own-reviews`, and `full-visibility` without exposing strategy labels.
2. Do not promote `full-visibility` to the public default yet despite higher scores; first check semantic convergence in the panel review.
3. Keep `score-only` as the production default for now because it preserves the strongest measured lexical diversity and matches the original brainstorm design goal.
4. Complete `todos/026-stream-parse-pi-json-events.md` to stream-parse Pi JSON mode, avoiding large transport buffers while preserving current event extraction.
5. For L3 prompt-reframing work, use `score-only` as the default baseline and include `own-reviews` as the most interesting L2 challenger if budget allows.
