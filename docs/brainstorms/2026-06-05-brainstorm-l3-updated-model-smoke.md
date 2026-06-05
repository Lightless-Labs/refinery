---
date: 2026-06-05
topic: brainstorm-l3-updated-model-smoke
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
related_todo: 018-brainstorm-divergence-expansion
---

# Brainstorm L3 Updated-Model Smoke

## Summary

After Pi exposed newer benchmark candidates (`pi/kimi-coding/kimi-for-coding` / Kimi K2.6 for coding, and `pi/minimax/MiniMax-M3`), ran a small L3 smoke to check whether prompt-reframing expansion still works with the updated models.

This is **not** a full L3 benchmark. It is a budget/operability check on the product prompt with two updated models.

Models:

```text
pi/kimi-coding/kimi-for-coding:off
pi/minimax/MiniMax-M3:off
```

Artifact root:

```text
target/brainstorm-benchmark-2026-06-05-l3-updated-models-two-model/
```

## Commands

Baseline (`prompt-variants off`):

```sh
cargo run -q -p refinery_cli -- brainstorm "$PROMPT" \
  --models pi/kimi-coding/kimi-for-coding:off,pi/minimax/MiniMax-M3:off \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --iteration-strategy score-only \
  --prompt-variants off \
  --output-format json \
  --verbose \
  --idle-timeout 240 \
  --timeout 900 \
  --max-concurrent 1
```

Prompt-reframing run used the same arguments with `--prompt-variants per-model`.

Analyzer:

```sh
cargo run -q -p refinery_cli -- benchmark-brainstorm $(cat target/brainstorm-benchmark-2026-06-05-l3-updated-models-two-model/logs/run-dirs.txt) --output-format text
```

## Results

### Baseline: `prompt-variants off`

Run:

```text
target/brainstorm-benchmark-2026-06-05-l3-updated-models-two-model/off/product/20260605-221050_generate-unconventional-but-practical-pr_8dd1
```

Result:

- `status: brainstormed`
- `degraded: false`
- `evaluation_status: peer_evaluated`
- `prompt_variant_count: 1`
- `lineage_count: 2`
- `total_calls: 8`
- elapsed: ~223s

`controversy_floor_7` metrics:

| Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---:|---:|---:|---:|---:|
| 7.50 | 7.00 | 0.00 | 0.073 | 0.00 |

### L3 smoke: `prompt-variants per-model`

Run:

```text
target/brainstorm-benchmark-2026-06-05-l3-updated-models-two-model/per-model/product/20260605-221451_generate-unconventional-but-practical-pr_0bfc
```

Result:

- `status: degraded`
- `evaluation_status: peer_evaluated`
- `prompt_variant_count: 3`
- `lineage_count: 6`
- final candidates: 5
- `total_calls: 25` of expected 26
- elapsed: ~2386s (~39.8m), dominated by one timeout
- provider failure: round 2 MiniMax proposal for `+variant-2` timed out after 900s

Generated variants were meaningfully different:

- Kimi reframed toward adversarial environments, surveillance capitalism, infrastructure collapse, and privacy as a metabolic/degradation process.
- MiniMax reframed toward subpoena/adversarial legal scrutiny, reconstruction, and defensibility of a knowledge base.

`controversy_floor_7` metrics:

| Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---:|---:|---:|---:|---:|
| 8.33 | 8.00 | 0.00 | 0.080 | 0.00 |

`quality_x_lexdiv` selected one MiniMax original-lineage answer and reduced lexical overlap to `0.065` with the same quality mean/min.

## Observations

- The updated Kimi and MiniMax models are reachable through Pi and work for single-model brainstorm calls.
- Two-model prompt reframing produced a larger and apparently stronger candidate pool on this product prompt, but the run is too small and degraded to support a default change.
- With only two providers, each candidate has one evaluator, so `stddev`/controversy metrics collapse to `0.00`; two-model runs are useful for operability smoke only, not disagreement-based selector conclusions.
- MiniMax M3 can be very verbose/slow under expanded prompt frames. The degraded run's timeout came from a MiniMax proposal on the legal-scrutiny variant, not from JSON parsing or artifact analysis.
- A four-model updated-model sample was started with Codex + GLM + Kimi-for-coding + MiniMax M3, but it was stopped after ~14 minutes while still in the first baseline run. Partial artifacts showed round-1 progress; this was an operational budget/time decision, not a confirmed correctness failure.

## Recommendation

Do not launch a full 4-model × 6-prompt L3 suite with MiniMax M3 without first adding tighter benchmark controls (for example a smaller prompt sample, shorter answer budget if Pi supports it, or model-specific timeout/output expectations).

For the next L3 comparison, prefer either:

1. a three-model run that excludes MiniMax M3 if latency dominates, or
2. a 2-3 prompt updated-model sample with explicit acceptance of long MiniMax timeouts/degradation.

Keep production defaults unchanged: `score-only`, `prompt-variants off`, `controversy_floor_7`.
