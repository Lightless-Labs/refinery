---
date: 2026-05-23
topic: brainstorm-smoke-baseline
todo: 019-brainstorm-smoke-test-field-report
---

# Brainstorm Smoke Baseline: Codex + GLM + Kimi + MiniMax

## Summary

This run **did establish a valid multi-model v0 brainstorm baseline** using four available providers:

- `codex-cli/gpt-5.4`
- `opencode/zai-coding-plan/glm-5.1`
- `opencode/kimi-for-coding/kimi-k2-thinking`
- `opencode/minimax-coding-plan/MiniMax-M2.5`

The successful baseline runs used `--max-concurrent 1` because a first parallel run exposed an OpenCode SQLite/WAL contention failure when several OpenCode subprocesses started simultaneously.

Both serial baseline prompts completed with:

- `status: "brainstormed"`
- `degraded: false`
- `evaluation_status: "peer_evaluated"`
- `rounds: 2`
- `total_calls: 32`
- `provider_failures: []`

Artifacts and logs are under `target/brainstorm-smoke-2026-05-22/`.

## Model Set

```text
codex-cli,
opencode/zai-coding-plan/glm-5.1,
opencode/kimi-for-coding/kimi-k2-thinking,
opencode/minimax-coding-plan/MiniMax-M2.5
```

Dry-run estimate:

```json
{
  "status": "dry_run",
  "verb": "brainstorm",
  "models": 4,
  "max_rounds": 2,
  "calls_per_round": 16,
  "total_calls": 32,
  "panel_size": 3
}
```

## Commands Run

### Product / strategy prompt

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Generate unconventional but practical product ideas for a privacy-first personal knowledge assistant for software teams. Focus on ideas a small startup could prototype in 6 weeks." \
  --models codex-cli,opencode/zai-coding-plan/glm-5.1,opencode/kimi-for-coding/kimi-k2-thinking,opencode/minimax-coding-plan/MiniMax-M2.5 \
  --max-rounds 2 \
  --panel-size 3 \
  --max-concurrent 1 \
  --timeout 900 \
  --idle-timeout 180 \
  --output-format json \
  --output-dir target/brainstorm-smoke-2026-05-22/product-serial \
  > target/brainstorm-smoke-2026-05-22/logs/product-serial.json \
  2> target/brainstorm-smoke-2026-05-22/logs/product-serial.stderr
```

Result: exit `0`.

Artifact run dir:

```text
target/brainstorm-smoke-2026-05-22/product-serial/20260523-060748_generate-unconventional-but-practical-pr_356f
```

Panel:

| Rank | Model | Mean | Stddev | Controversy | Per-evaluator scores |
|---:|---|---:|---:|---:|---|
| 1 | `opencode/minimax-coding-plan/MiniMax-M2.5` | 7.33 | 0.94 | 6.91 | Codex 8, GLM 6, Kimi 8 |
| 2 | `codex-cli/gpt-5.4` | 8.33 | 0.47 | 3.93 | GLM 9, Kimi 8, MiniMax 8 |
| 3 | `opencode/kimi-for-coding/kimi-k2-thinking` | 8.00 | 0.00 | 0.00 | Codex 8, GLM 8, MiniMax 8 |

Notable panel excerpts / themes:

- MiniMax: theatrical but concrete team rituals such as **Dead Code Funeral** and **Bus Factor Heatmap**.
- Codex: local-development-memory wedge focused on private exhaust such as branches, shell history, merge conflicts, abandoned attempts, and draft questions.
- Kimi: more conceptual reframing around what privacy and knowledge assistance mean, but with visible score-history meta-preamble.

### Technical / artifact-format prompt

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Design a lightweight artifact format for recording multi-model brainstorming runs so later benchmarking can compare divergence, novelty, and feasibility without storing secrets. Include schema shape and implementation trade-offs." \
  --models codex-cli,opencode/zai-coding-plan/glm-5.1,opencode/kimi-for-coding/kimi-k2-thinking,opencode/minimax-coding-plan/MiniMax-M2.5 \
  --max-rounds 2 \
  --panel-size 3 \
  --max-concurrent 1 \
  --timeout 900 \
  --idle-timeout 180 \
  --output-format json \
  --output-dir target/brainstorm-smoke-2026-05-22/technical-serial \
  > target/brainstorm-smoke-2026-05-22/logs/technical-serial.json \
  2> target/brainstorm-smoke-2026-05-22/logs/technical-serial.stderr
```

Result: exit `0`.

Artifact run dir:

```text
target/brainstorm-smoke-2026-05-22/technical-serial/20260523-062159_design-a-lightweight-artifact-format-for_ccb6
```

Panel:

| Rank | Model | Mean | Stddev | Controversy | Per-evaluator scores |
|---:|---|---:|---:|---:|---|
| 1 | `opencode/minimax-coding-plan/MiniMax-M2.5` | 6.67 | 1.25 | 8.31 | Codex 7, GLM 5, Kimi 8 |
| 2 | `codex-cli/gpt-5.4` | 8.33 | 0.47 | 3.93 | GLM 8, Kimi 9, MiniMax 8 |
| 3 | `opencode/kimi-for-coding/kimi-k2-thinking` | 7.33 | 0.47 | 3.46 | Codex 7, GLM 7, MiniMax 8 |

Notable panel excerpts / themes:

- MiniMax: **Three-Layer Artifact Format** with identity, structure, and signals layers; selected first because evaluator disagreement was highest.
- Codex: `BRAID/2` as append-only JSONL representing a secretless idea graph rather than a transcript; highest mean but lower controversy.
- Kimi: **Resonance Cavity** metaphor for storing interference patterns between models; conceptually distinctive but more abstract.

## Observations

### Score-only iteration

The v0 mechanism worked: round-2 answers visibly evolved without seeing peer text. Models reacted to their own prior answer and score, producing distinct lineages rather than converging on one shared answer.

One UX side effect: several final answers include meta-preambles such as "Based on my Round 1 score..." or "My previous proposal got...". This is faithful to the score-only prompt, but it is noisy in user-facing panel output. Future prompt polish should ask models to produce final answers without mentioning score history unless relevant.

### Controversial selection

Controversial selection was exercised and behaved as designed. In both prompts, MiniMax ranked first despite a lower mean score than Codex because evaluator disagreement was much higher:

- Product: MiniMax `mean=7.33`, `stddev=0.94`, `controversy=6.91`; Codex `mean=8.33`, `stddev=0.47`, `controversy=3.93`.
- Technical: MiniMax `mean=6.67`, `stddev=1.25`, `controversy=8.31`; Codex `mean=8.33`, `stddev=0.47`, `controversy=3.93`.

This produced more varied panels than highest-mean-only selection would have produced.

### Panel overlap / diversity

The panels were meaningfully non-overlapping:

- Product: ritual/team-behavior ideas, private local-dev-memory assistant, and conceptual privacy reframing.
- Technical: layered schema, append-only lineage graph, and metaphor-driven resonance/interference format.

This is good enough to justify using v0 as the baseline before implementing prompt-reframing or Open Collider-style divergence expansion.

### Rubric behavior

Scores looked plausible and discriminative. The most interesting behavior was not the mean scores but the disagreement distribution. GLM was harsher on MiniMax in the technical run (`5`) while Kimi was favorable (`8`), which is exactly the kind of disagreement the current selection formula surfaces.

### Provider / execution UX

A parallel run without `--max-concurrent 1` returned `status: "degraded"` with OpenCode failures:

```text
Failed to run the query 'PRAGMA journal_mode = WAL'
```

The degraded-run observability added in TODO 020 worked: JSON marked the run degraded, listed provider failures, and wrote `provider-failures.json`. However, the practical workaround for OpenCode-backed panels is currently serial execution.

### Output UX

- JSON success shape was usable.
- Artifact output was complete and easy to inspect.
- Non-TTY stderr for the successful serial runs contained no raw ANSI escape codes.
- The final panel can include score-history meta-preambles from models, which should be polished later.

## Conclusions

This is the first valid v0 brainstorm baseline observed in this repo.

The v0 strategy is worth keeping as the baseline:

- score-only iteration preserved independent lineages,
- controversial selection surfaced answers that a mean-only selector would not rank first,
- output artifacts are sufficient for later benchmark fixture work.

The next strategy work should now be `todos/013` benchmark design and/or `todos/018` prompt-reframing experiments, using this run as the baseline reference.

## Follow-ups

- Created `todos/022-opencode-concurrency-sqlite-wal.md` for OpenCode concurrent subprocess WAL failures.
- Consider prompt polish to suppress score-history meta-preambles in final brainstorm answers before broader demos.
- `todos/013` is now unblocked by a valid baseline.
- `todos/018` can now receive a dedicated implementation plan if prompt reframing is still desired.
