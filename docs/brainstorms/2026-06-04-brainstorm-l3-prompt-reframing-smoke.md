---
date: 2026-06-04
topic: brainstorm-l3-prompt-reframing-smoke
todo: 018-brainstorm-divergence-expansion
plan: 2026-06-04-001-feat-brainstorm-prompt-reframing-expansion-plan
---

# Brainstorm L3 Prompt-Reframing Smoke

## Summary

Ran smoke validation for hidden/internal prompt-reframing expansion. Schematic command shape:

```sh
refinery brainstorm "$PROMPT" \
  --models pi/openai-codex/gpt-5.4:off,pi/zai/glm-5.1:off,pi/kimi-coding/kimi-k2-thinking:off \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --prompt-variants per-model \
  --output-format json \
  --idle-timeout 480 \
  --timeout 1800 \
  --max-concurrent 1
```

This is **not** the full L3 benchmark suite. It validates that prompt reframing can produce analyzable artifacts and exposed then verified the Pi JSON streaming fix in `todos/026`.

## Cost Shape

For `n` models, `per-model` generates one variant per model plus the original prompt:

```text
prompt_variants = n + 1
lineages = n * (n + 1)
variant_calls = n
calls_per_round = lineages + lineages * (n - 1)
total_calls = variant_calls + max_rounds * calls_per_round
```

For 3 models and 2 rounds, dry-run reported:

```json
{
  "models": 3,
  "max_rounds": 2,
  "calls_per_round": 36,
  "total_calls": 75,
  "prompt_variant_strategy": "per-model"
}
```

For 4 models and 2 rounds, dry-run reported `total_calls: 164`, so full six-prompt L3 with four models would be expensive and should be budgeted deliberately.

## Smoke Runs

### Two-model product smoke

Artifacts:

```text
target/brainstorm-benchmark-2026-06-04-l3-smoke/product-two-model/20260604-215623_generate-unconventional-but-practical-pr_e07a
```

Result:

- `status: brainstormed`
- `degraded: false`
- `evaluation_status: peer_evaluated`
- `prompt_variant_strategy: per-model`
- `prompt_variant_count: 3`
- `lineage_count: 6`
- `total_calls: 26`

This confirmed the artifact layout stayed compatible with `benchmark-brainstorm`.

### Three-model product smoke before Pi streaming fix

Artifacts:

```text
target/brainstorm-benchmark-2026-06-04-l3-smoke/product-three-model/20260604-221056_generate-unconventional-but-practical-pr_3a26
```

Result was usable but degraded:

- `status: degraded`
- `evaluation_status: partial`
- `prompt_variant_count: 4`
- `lineage_count: 12`
- `total_calls: 71` out of expected 75

Failure modes in `provider-failures.json`:

- Codex proposals exceeded the tactical 64MB Pi stdout cap twice:
  - `64003882 bytes (max: 64000000)`
  - `64012342 bytes (max: 64000000)`
- GLM returned invalid brainstorm evaluation scores for several Kimi variant targets.

This run re-exposed `todos/026`: Pi JSON event streams can be much larger than final assistant text.

### Three-model product smoke after Pi streaming fix

Artifacts:

```text
target/brainstorm-benchmark-2026-06-04-l3-smoke-streaming/product-three-model/20260604-224820_generate-unconventional-but-practical-pr_a9e8
```

Result:

- `status: brainstormed`
- `degraded: false`
- `evaluation_status: peer_evaluated`
- `prompt_variant_strategy: per-model`
- `prompt_variant_count: 4`
- `lineage_count: 12`
- `total_calls: 75`
- no `provider-failures.json`
- stderr log was empty (`0` bytes)

Analyzer output:

| Selector | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---:|---:|---:|---:|---:|
| `mean` | 8.83 | 8.50 | 0.17 | 0.132 | 0.00 |
| `stddev` | 6.67 | 4.00 | 0.67 | 0.052 | 0.00 |
| `controversy` | 6.67 | 4.00 | 0.67 | 0.052 | 0.00 |
| `controversy_floor_7` | 7.83 | 7.50 | 0.50 | 0.103 | 0.00 |
| `quality_x_lexdiv` | 8.67 | 8.00 | 0.00 | 0.081 | 0.00 |

## Early Observations

- Prompt reframing materially expands the candidate pool: 3 models yielded 12 final-round candidates.
- The generated variants were meaningfully different, not mere paraphrases:
  - Codex reframed toward engineering managers/security leads and coordination cost.
  - GLM reframed toward expensive-to-replace knowledge types and ambient micro-experiments.
  - Kimi reframed toward deliberate short-term friction / anti-productivity.
- Raw `controversy` again selected a low-quality high-disagreement answer (`min_quality: 4.00`), reinforcing the need to keep `controversy_floor_7` as the production-like selector.
- The clean 3-model run's `controversy_floor_7` panel had lower lexical overlap (`0.103`) than mean-only (`0.132`) with acceptable minimum quality (`7.50`).

## Recommendation

Do not launch a full 4-model × 6-prompt L3 suite without budget approval: it is ~984 provider calls at 2 rounds (`6 * 164`). A smaller next step is a 3-model six-prompt L3 comparison against `score-only` baseline (~450 calls for prompt-reframing alone) or a 2-3 prompt sample before scaling.
