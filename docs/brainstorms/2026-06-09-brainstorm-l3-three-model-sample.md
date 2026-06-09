---
date: 2026-06-09
topic: brainstorm-l3-three-model-sample
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
related_todo: 018-brainstorm-divergence-expansion
---

# Brainstorm L3 Three-Model Sample

## Summary

Ran a two-prompt L3 comparison using three Pi-routed models and the merged hidden prompt-reframing expansion:

```text
pi/openai-codex/gpt-5.4:off
pi/zai/glm-5.1:off
pi/kimi-coding/kimi-for-coding:off
```

Compared:

- baseline: `--prompt-variants off`
- L3 expansion: `--prompt-variants per-model`

Common settings:

```text
--max-rounds 2
--panel-size 3
--quality-floor 7.0
--iteration-strategy score-only
--idle-timeout 480
--timeout 1800
--max-concurrent 1
```

Artifact root:

```text
target/brainstorm-benchmark-2026-06-09-l3-three-model-sample/
```

Analyzer outputs:

```text
target/brainstorm-benchmark-2026-06-09-l3-three-model-sample/logs/l3-three-model-sample-analysis.json
target/brainstorm-benchmark-2026-06-09-l3-three-model-sample/logs/l3-three-model-sample-analysis.txt
```

## Prompt Suite

1. Product/strategy — privacy-first personal knowledge assistant.
2. Technical/design — secretless multi-model brainstorm artifact format.

## Run Results

| Prompt | Prompt variants | Status | Eval status | Calls | Elapsed | Provider failures |
|---|---|---|---|---:|---:|---|
| product | `off` | `brainstormed` | `peer_evaluated` | 18 | ~7.1m | none |
| product | `per-model` | `degraded` | `partial` | 75 | ~37.8m | GLM invalid eval score |
| technical | `off` | `brainstormed` | `peer_evaluated` | 18 | ~7.9m | none |
| technical | `per-model` | `degraded` | `partial` | 75 | ~30.3m | Codex SSE header timeout; GLM invalid eval score |

The per-model runs produced complete final-round candidate sets despite degraded evaluation status:

- product per-model: 12 candidates
- technical per-model: 12 candidates

## Production-Selector Metrics

`controversy_floor_7` view:

| Prompt | Prompt variants | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---|---:|---:|---:|---:|---:|
| product | `off` | 8.33 | 7.50 | 0.33 | 0.056 | 0.00 |
| product | `per-model` | 8.17 | 8.00 | 0.83 | 0.102 | 0.00 |
| technical | `off` | 7.33 | 6.50 | 0.33 | 0.056 | 0.00 |
| technical | `per-model` | 8.33 | 8.00 | 0.67 | 0.045 | 0.00 |

Two-prompt averages:

| Prompt variants | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---:|---:|---:|---:|---:|
| `off` | 7.83 | 7.00 | 0.33 | 0.056 | 0.00 |
| `per-model` | 8.25 | 8.00 | 0.75 | 0.073 | 0.00 |

## Observations

- The three-model L3 sample is operationally feasible but expensive: the paired two-prompt sample took ~83 minutes wall-clock with serial Pi calls.
- Prompt reframing improved the two-prompt average quality floor (`7.00` → `8.00`) and disagreement (`0.33` → `0.75`) under `controversy_floor_7`.
- Lexical overlap increased on the product prompt (`0.056` → `0.102`) but decreased on the technical prompt (`0.056` → `0.045`), so there is no simple diversity conclusion from this small sample.
- Both per-model runs degraded from evaluation issues rather than proposal collapse:
  - GLM produced invalid brainstorm evaluation scores in both per-model runs.
  - Codex hit an SSE response header timeout during one technical evaluation.
- The per-model product run initially looked stalled because artifact stdout is only written at process completion; inspecting child processes showed provider calls were still running, and the run completed successfully at the candidate-artifact level.
- `meta_preamble_rate` stayed at `0.0` across all selectors and runs.

## Recommendation

Do not change production defaults based on this sample.

Next benchmark step should be one of:

1. run 2-4 more prompts with the same three-model panel to see whether the quality-floor gain survives degraded evaluation noise, or
2. first harden/triage GLM invalid evaluation scores for expanded brainstorm evaluation prompts, since both per-model runs degraded on that failure mode.

Keep `--max-concurrent 1` for Pi-backed benchmark runs.
