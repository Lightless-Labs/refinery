---
date: 2026-05-25
topic: brainstorm-meta-preamble-prompt-polish
todo: 024-brainstorm-suppress-score-history-meta-preambles
plan: 2026-05-25-001-fix-brainstorm-score-history-meta-preambles-plan
---

# Brainstorm Meta-Preamble Prompt Polish Check

## Summary

After adding prompt instructions to use score history internally without mentioning scores, previous rounds, feedback, benchmark mechanics, or selection mechanics, I reran two known benchmark prompts.

Both runs completed cleanly:

- `status: "brainstormed"`
- `degraded: false`
- `evaluation_status: "peer_evaluated"`
- `selection_strategy: "controversy_floor_7"`
- `rounds: 2`
- four-model panel, serial execution via `--max-concurrent 1`

`refinery benchmark-brainstorm` reported `meta_preamble_rate: 0.0` for all selectors on both runs. This improves on the prior six-prompt baseline average of `0.333`.

## Model Panel

```text
codex-cli/gpt-5.4
opencode/zai-coding-plan/glm-5.1
opencode/kimi-for-coding/kimi-k2-thinking
opencode/minimax-coding-plan/MiniMax-M2.5
```

## Run Directories

```text
target/brainstorm-meta-preamble-2026-05-25/product/20260525-121143_generate-unconventional-but-practical-pr_7088
target/brainstorm-meta-preamble-2026-05-25/technical/20260525-122739_design-a-lightweight-artifact-format-for_270f
```

Analyzer outputs:

```text
target/brainstorm-meta-preamble-2026-05-25/logs/analysis.json
target/brainstorm-meta-preamble-2026-05-25/logs/analysis.txt
```

## Commands

### Product prompt

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Generate unconventional but practical product ideas for a privacy-first personal knowledge assistant for software teams. Focus on ideas a small startup could prototype in 6 weeks." \
  --models codex-cli,opencode/zai-coding-plan/glm-5.1,opencode/kimi-for-coding/kimi-k2-thinking,opencode/minimax-coding-plan/MiniMax-M2.5 \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --max-concurrent 1 \
  --timeout 900 \
  --idle-timeout 480 \
  --output-format json \
  --output-dir target/brainstorm-meta-preamble-2026-05-25/product
```

### Technical prompt

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Design a lightweight artifact format for recording multi-model brainstorming runs so later benchmarking can compare divergence, novelty, and feasibility without storing secrets. Include schema shape and implementation trade-offs." \
  --models codex-cli,opencode/zai-coding-plan/glm-5.1,opencode/kimi-for-coding/kimi-k2-thinking,opencode/minimax-coding-plan/MiniMax-M2.5 \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --max-concurrent 1 \
  --timeout 900 \
  --idle-timeout 480 \
  --output-format json \
  --output-dir target/brainstorm-meta-preamble-2026-05-25/technical
```

### Analyzer

```sh
cargo run -q -p refinery_cli -- benchmark-brainstorm \
  target/brainstorm-meta-preamble-2026-05-25/product/20260525-121143_generate-unconventional-but-practical-pr_7088 \
  target/brainstorm-meta-preamble-2026-05-25/technical/20260525-122739_design-a-lightweight-artifact-format-for_270f \
  --output-format json
```

## Results

| Prompt | Selector | Meta preamble rate | Panel mean quality | Panel min quality |
|---|---|---:|---:|---:|
| Product | `mean` | 0.0 | 8.22 | 8.00 |
| Product | `stddev` | 0.0 | 8.11 | 7.67 |
| Product | `controversy` | 0.0 | 8.11 | 7.67 |
| Product | `controversy_floor_7` | 0.0 | 8.11 | 7.67 |
| Product | `quality_x_lexdiv` | 0.0 | 8.22 | 8.00 |
| Technical | `mean` | 0.0 | 7.89 | 7.67 |
| Technical | `stddev` | 0.0 | 7.89 | 7.67 |
| Technical | `controversy` | 0.0 | 7.89 | 7.67 |
| Technical | `controversy_floor_7` | 0.0 | 7.89 | 7.67 |
| Technical | `quality_x_lexdiv` | 0.0 | 7.89 | 7.67 |

## Notes

- The analyzer's meta-preamble detector found no explicit score-history phrases in selected panels.
- One selected answer used wording like "Builds on ...", which is lineage-oriented but not an explicit score/round/benchmark preamble. If this style is still considered too process-like, create a narrower follow-up for forbidding references to the model's own prior concepts. For now, the acceptance criterion focused on score-history meta-commentary is satisfied.

## Conclusion

Prompt-only mitigation appears sufficient for the measured issue. `todos/024` can be marked complete.
