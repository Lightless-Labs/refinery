---
title: "feat: brainstorm prompt-reframing expansion"
type: feature
status: completed
date: 2026-06-04
todo: 018-brainstorm-divergence-expansion
---

# Brainstorm Prompt-Reframing Expansion Plan

**Enhanced:** 2026-06-04 (session start from handoff)
**Completed:** 2026-06-04

## Context

`brainstorm` v0 preserves divergence through score-only iteration and controversial selection. L2 iteration-strategy benchmarks found that `score-only` remains the best default for useful diversity/non-overlap, while full visibility improves coverage at the cost of overlap. The next L3 benchmark step is upstream divergence: generate strategic prompt reframings before the brainstorm loop and let every model work every frame.

## Goals

- Add a hidden/internal prompt-reframing mode for benchmark runs.
- Keep the production default unchanged: no prompt variants, score-only iteration, quality floor `7.0`.
- Implement the first staged rollout from `todos/018`: original prompt anchor plus one strategic reframing per model.
- Preserve artifact compatibility enough for `benchmark-brainstorm` to analyze final-round proposals/evaluations.
- Expose metadata/dry-run call counts so L3 benchmark cost is explicit.

## Non-Goals

- Do not implement domain collisions in this milestone.
- Do not expose prompt reframing as a public/default UX until benchmark results justify it.
- Do not change panel selection defaults.
- Do not require a human/calibrated judge pass before this benchmark-enabling implementation.

## Design

### CLI

Add hidden `brainstorm --prompt-variants off|per-model`:

- `off` keeps current behavior.
- `per-model` asks each provider for one strategic reframing, then runs each provider over the original prompt plus all generated reframings.

For `n` models and `per-model`, lineages are `n * (1 + n)`. A dry run should report:

```text
variant_calls = n
lineages = n * (1 + n)
calls_per_round = lineages + lineages * (n - 1)
total_calls = variant_calls + max_rounds * calls_per_round
```

### Core

- Add `BrainstormPromptVariantStrategy` to `BrainstormConfig`.
- Generate prompt variants before round 1 using a dedicated JSON schema.
- Treat the original prompt as an anchor; generated variants must preserve the user goal while changing assumptions/success criteria/stakeholders/etc.
- Represent each `(provider, prompt-frame)` as an internal lineage. Variant lineage candidate IDs can suffix the provider model ID so existing `HashMap<ModelId, String>` artifacts still work.
- Peer evaluation remains by provider: a provider does not evaluate its own lineages, but it evaluates other providers' lineages across all frames.
- Single-provider expanded runs skip evaluation as before because no peer evaluator exists.

### Artifacts / Output

- Save prompt-variant metadata in `metadata.json` (`prompt_variant_strategy`, counts).
- Save generated variants in a small JSON artifact when `--output-dir` is set.
- Include `prompt_variant_strategy` in text/JSON/dry-run output.

## Test Plan

- Unit tests for prompt-variant strategy parsing.
- Prompt tests for reframing prompt requirements and schema validity.
- Core tests with echo providers verifying per-model reframing expands call counts/lineages and skips self-evaluation by provider.
- CLI dry-run test if an existing CLI test harness covers command output; otherwise verify manually with dry-run.
- Run targeted checks:
  - `cargo fmt --all -- --check`
  - `cargo test -p refinery_core brainstorm`
  - `cargo test -p refinery_core prompts`
  - `cargo test -p refinery_cli brainstorm`
  - `cargo clippy -p refinery_core --all-targets -- -D warnings`
  - `cargo clippy -p refinery_cli --all-targets -- -D warnings`

## Implementation Notes

Completed 2026-06-04:

- Hidden `brainstorm --prompt-variants off|per-model` CLI flag added.
- Core prompt-variant generation added with original-prompt anchor plus one strategic variant per model.
- Expanded runs represent `(model, prompt frame)` as flat lineage IDs such as `provider/model+variant-1`, preserving the existing `round-N/propose-*.md` and `evaluate-*.json` artifact layout consumed by `benchmark-brainstorm`.
- Peer evaluation is still provider-owned: providers evaluate other providers' lineages but not their own lineages.
- `metadata.json`, CLI text/JSON output, and dry-run output now expose prompt-variant strategy/count information.
- `benchmark-brainstorm` now surfaces prompt-variant metadata in text/JSON output.

## Verification

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test -p refinery_core brainstorm -q`
- `cargo test -p refinery_core prompts -q`
- `cargo test -p refinery_cli brainstorm -q`
- `cargo test --workspace` — clean final summary: all workspace unit/doc tests passed.
- `cargo clippy -p refinery_core --all-targets -- -D warnings`
- `cargo clippy -p refinery_cli --all-targets -- -D warnings`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo run -q -p refinery_cli -- brainstorm "test" --models test/a,test/b --dry-run --prompt-variants per-model --output-format json`
- `cargo run -q -p refinery_cli -- benchmark-brainstorm target/tmp-prompt-variant-benchmark --output-format text`
- `cargo run -q -p refinery_cli -- benchmark-brainstorm target/tmp-prompt-variant-benchmark --output-format json`
