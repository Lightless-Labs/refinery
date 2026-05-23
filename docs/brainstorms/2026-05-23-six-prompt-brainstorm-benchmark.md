---
date: 2026-05-23
topic: six-prompt-brainstorm-benchmark
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
---

# Six-Prompt Brainstorm Benchmark

## Summary

Ran the first six-prompt v0 benchmark suite and analyzed the saved artifacts with `refinery benchmark-brainstorm`.

All six final runs completed as valid peer-evaluated four-model brainstorms:

- `status: "brainstormed"`
- `degraded: false`
- `evaluation_status: "peer_evaluated"`
- `rounds: 2`
- `total_calls: 32` per prompt

The research prompt initially degraded because Codex and Kimi hit idle timeouts at `--idle-timeout 180`; rerunning with `--idle-timeout 480` completed cleanly.

## Model Panel

```text
codex-cli/gpt-5.4
opencode/zai-coding-plan/glm-5.1
opencode/kimi-for-coding/kimi-k2-thinking
opencode/minimax-coding-plan/MiniMax-M2.5
```

All successful benchmark runs used `--max-concurrent 1` because multiple OpenCode-backed subprocesses can hit SQLite/WAL startup failures (see `todos/022-opencode-concurrency-sqlite-wal.md`).

## Prompt Suite

The suite combined the two existing valid baseline prompts with four new prompts:

1. Product/strategy — privacy-first personal knowledge assistant.
2. Technical/design — secretless multi-model brainstorm artifact format.
3. Architecture — local AI coding tool plugin system with sandboxing.
4. Debugging/process — reduce flaky CI failures in a Rust/Bazel monorepo.
5. Research/science — low-cost indoor air quality experiments.
6. Governance/operations — lightweight governance for AI coding agents.

## Run Directories

```text
target/brainstorm-smoke-2026-05-22/product-serial/20260523-060748_generate-unconventional-but-practical-pr_356f
target/brainstorm-smoke-2026-05-22/technical-serial/20260523-062159_design-a-lightweight-artifact-format-for_ccb6
target/brainstorm-benchmark-2026-05-23/architecture/20260523-201812_design-a-plugin-system-for-local-ai-codi_5be1
target/brainstorm-benchmark-2026-05-23/debugging/20260523-203631_generate-unconventional-but-practical-wa_9324
target/brainstorm-benchmark-2026-05-23/research-retry/20260523-213237_propose-unconventional-low-cost-experime_11e7
target/brainstorm-benchmark-2026-05-23/governance/20260523-211827_design-a-lightweight-governance-model-fo_8c6c
```

Analyzer outputs:

```text
target/brainstorm-benchmark-2026-05-23/logs/six-prompt-analysis.json
target/brainstorm-benchmark-2026-05-23/logs/six-prompt-analysis.txt
```

## Analyzer Command

```sh
cargo run -q -p refinery_cli -- benchmark-brainstorm \
  target/brainstorm-smoke-2026-05-22/product-serial/20260523-060748_generate-unconventional-but-practical-pr_356f \
  target/brainstorm-smoke-2026-05-22/technical-serial/20260523-062159_design-a-lightweight-artifact-format-for_ccb6 \
  target/brainstorm-benchmark-2026-05-23/architecture/20260523-201812_design-a-plugin-system-for-local-ai-codi_5be1 \
  target/brainstorm-benchmark-2026-05-23/debugging/20260523-203631_generate-unconventional-but-practical-wa_9324 \
  target/brainstorm-benchmark-2026-05-23/research-retry/20260523-213237_propose-unconventional-low-cost-experime_11e7 \
  target/brainstorm-benchmark-2026-05-23/governance/20260523-211827_design-a-lightweight-governance-model-fo_8c6c \
  --output-format json
```

## Aggregate Selector Results

Averages across six prompts:

| Selector | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---:|---:|---:|---:|---:|
| `mean` | 8.093 | 7.778 | 0.288 | 0.130 | 0.333 |
| `stddev` | 7.685 | 6.833 | 0.551 | 0.100 | 0.333 |
| `controversy` | 7.685 | 6.833 | 0.551 | 0.100 | 0.333 |
| `controversy_floor_7` | 7.963 | 7.500 | 0.386 | 0.118 | 0.333 |
| `quality_x_lexdiv` | 8.056 | 7.667 | 0.340 | 0.121 | 0.333 |

## Per-Prompt Highlights

### Product

- `controversy` selected MiniMax, Codex, Kimi.
- Mean quality dropped from `8.11` to `7.89` versus mean-only.
- Lexical overlap improved from `0.124` to `0.070`.

### Technical

- `controversy` selected MiniMax first despite `mean=6.67` because evaluator disagreement was high.
- `controversy_floor_7` excluded MiniMax and matched the mean/lexdiv set.
- This is evidence that controversy needs a quality floor for user-facing defaults.

### Architecture

- Mean-only selected Codex, GLM, Kimi.
- Controversy pulled in MiniMax at `mean=6.67`, reducing lexical overlap but lowering quality floor.
- `controversy_floor_7` again matched the higher-quality set.

### Debugging

- Strongest quality-floor warning.
- `controversy` selected MiniMax first with `mean=5.67`, `stddev=1.25`, `controversy=7.07`.
- This produced low lexical overlap (`0.065`) but an unacceptable panel quality floor (`5.67`).
- `controversy_floor_7` recovered the mean-only set with `min_quality=8.0`.

### Research

- Valid retry completed with no provider failures.
- `controversy` and `controversy_floor_7` both selected GLM, MiniMax, Codex.
- Here the floor did not alter the set because all selected candidates met `mean >= 7`.

### Governance

- `controversy` and `controversy_floor_7` selected GLM, MiniMax, Codex.
- Mean-only and `quality_x_lexdiv` preserved slightly higher quality and lower meta-preamble rate.

## Findings

### 1. Raw controversy is too willing to trade quality for disagreement

Across six prompts, `controversy` reduced lexical overlap from `0.130` to `0.100`, but it also reduced average panel minimum quality from `7.778` to `6.833`. The debugging prompt showed the failure mode clearly: high disagreement can rank a `5.67` mean answer first.

### 2. A quality floor is the best immediate selector improvement

`controversy_floor_7` preserved much of the diversity benefit while raising average minimum quality to `7.500`. It is a better candidate default than raw controversy if the product promise is "diverse but still worth reading." 

### 3. `mean` remains a strong baseline

Mean-only had the best average panel quality and quality floor. It should remain the primary baseline in future benchmarks, especially for tasks where user time is scarce.

### 4. `quality_x_lexdiv` is a reasonable cheap heuristic, but not clearly better yet

The greedy lexical-diversity selector often matched mean-only. Its aggregate metrics were close to mean-only: slightly lower quality, slightly better lexical overlap. It may become more useful on larger candidate pools or prompt-reframing lineages.

### 5. Meta-preambles are common enough to track

All selectors averaged `0.333` meta-preamble rate. Models frequently mention score history or previous rounds in final panel answers. This should be addressed by prompt polish before public demos or human evaluation.

## Recommendations

1. Add a production quality floor to brainstorm panel selection, or at least expose it as an option for benchmarking.
2. Keep raw controversy as an experimental selector, not the only default.
3. Before implementing prompt reframing, run a smaller code change to suppress score-history meta-preambles in final-round answers.
4. Use `benchmark-brainstorm` as the standard measurement path for all future strategy variants.
5. For L2 iteration strategy benchmarks, compare at least: `mean`, `controversy`, `controversy_floor_7`, and `quality_x_lexdiv` on the same artifacts.

## Follow-Up TODOs

- Created `todos/023-brainstorm-quality-floor-selection.md` for production/benchmark quality-floor selection.
- Created `todos/024-brainstorm-suppress-score-history-meta-preambles.md` for prompt polish around score-history meta-commentary.
- Continue `todos/013` with L2 iteration strategy variants after deciding whether to change the default selector.
