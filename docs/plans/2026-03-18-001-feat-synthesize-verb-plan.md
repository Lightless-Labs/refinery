---
title: "feat: synthesize verb"
type: feat
status: active
date: 2026-03-18
origin: docs/brainstorms/2026-03-18-synthesize-verb-requirements.md
---

# Synthesize Verb

## Overview

Add `refinery synthesize` — a two-phase verb that runs converge rounds to raise quality, then has all models synthesize the best answers into a unified response.

(see origin: `docs/brainstorms/2026-03-18-synthesize-verb-requirements.md`)

## Problem Statement

`converge` returns a single model's best answer. But the best possible response often combines insights from multiple models. `synthesize` extracts quality answers via converge rounds, then has every model produce a synthesis from that curated input, evaluated on integration/coherence/completeness/fidelity.

## Proposed Solution

### Phase 1: Converge rounds (existing engine)

Reuse `Engine::run()` with `max_rounds` set to `--converge-rounds` (default 2). This produces `ConsensusOutcome` with scored `all_answers`.

### Phase 2: Synthesis round (new)

1. Filter `all_answers` by `--synthesis-threshold` (default: same as `--threshold`)
2. Build a synthesis prompt: original user prompt + all qualifying answers
3. All models produce a synthesis (via `phases::propose::run()` with the synthesis prompt)
4. All models evaluate each other's syntheses (via `phases::evaluate::run()` with synthesis rubric)
5. Pick the best synthesis (highest score, simple — no stability rounds needed)

### Why this approach

- Reuses the existing engine for phase 1 (no changes to `Engine` or `Session`)
- Phase 2 is orchestrated in `run_synthesize()` in the CLI, calling phase functions directly
- No new closing strategy needed — synthesis is a single propose+evaluate cycle
- Prompts are the only truly new code

## Implementation Phases

### Phase 1: CLI scaffolding + args

**File: `crates/refinery_cli/src/main.rs`**

- Add `Synthesize(SynthesizeArgs)` to `Command` enum
- Create `SynthesizeArgs`:
  ```rust
  struct SynthesizeArgs {
      #[command(flatten)]
      shared: SharedArgs,
      #[arg(short, long, default_value = "8.0")]
      threshold: f64,
      #[arg(long, default_value = "2")]
      converge_rounds: u32,
      #[arg(long)]
      synthesis_threshold: Option<f64>, // defaults to threshold if not set
      #[arg(short = 's', long, default_value = "2")]
      stability_rounds: u32,
  }
  ```
- Add `run_synthesize()` async function
- Route in `async_main()`: `Command::Synthesize(args) => run_synthesize(args).await`

### Phase 2: Synthesis prompts

**File: `crates/refinery_core/src/prompts/synthesize.rs`**

- Add `SYNTHESIS_SCHEMA`:
  ```json
  {"type":"object","properties":{"synthesis":{"type":"string"}},"required":["synthesis"],"additionalProperties":false}
  ```
- Add `SYNTHESIS_EVAL_SCHEMA`:
  ```json
  {"type":"object","properties":{
    "integration":{"type":"integer"},
    "coherence":{"type":"integer"},
    "completeness":{"type":"integer"},
    "fidelity":{"type":"integer"},
    "rationale":{"type":"string"},
    "score":{"type":"integer"}
  },"required":["integration","coherence","completeness","fidelity","rationale","score"],"additionalProperties":false}
  ```
- Add `synthesize_prompt(user_prompt, qualifying_answers, round_ctx)` — presents all qualifying answers anonymously and asks the model to produce a unified synthesis
- Add `synthesize_evaluate_prompt(user_prompt, synthesis, label, nonce, round_ctx)` — evaluates on integration, coherence, completeness, fidelity

### Phase 3: Synthesis orchestration

**File: `crates/refinery_cli/src/commands/synthesize.rs` — `run()`**

```
1. Parse args, build providers (same as converge)
2. Run converge phase:
   - Build EngineConfig with converge_rounds as max_rounds
   - Run engine.run(&prompt)
   - Collect all_answers from outcome
3. Filter qualifying answers:
   - synthesis_threshold = args.synthesis_threshold.unwrap_or(args.threshold)
   - qualifying = all_answers.filter(|a| a.mean_score >= synthesis_threshold)
   - If qualifying.is_empty() → return NoQualifyingAnswers status
4. Build synthesis prompt with original prompt + qualifying answers
5. Run synthesis propose:
   - Call phases::propose::run() with synthesis prompt + SYNTHESIS_SCHEMA
   - Each model produces a synthesis
6. Run synthesis evaluate:
   - Call phases::evaluate::run() with SYNTHESIS_EVAL_SCHEMA
   - Each model scores each other's synthesis
7. Pick winner:
   - Compute mean scores from synthesis evaluations
   - Best score wins (no stability rounds)
8. Return result with synthesis as the answer
```

### Phase 4: Output + progress

- Reuse `JsonOutput` / text output format — winner is the best synthesizer, answer is the synthesis
- Add `Phase::Synthesize` variant for progress events
- Progress display shows synthesis phase after converge rounds

### Phase 5: New status variant

**File: `crates/refinery_core/src/types.rs`**

- Add `ConvergenceStatus::NoQualifyingAnswers` for when no answers meet the synthesis threshold after converge
- Add `ConvergenceStatus::Synthesized` for successful synthesis

## Acceptance Criteria

- [ ] `refinery synthesize "prompt" --models a,b,c` runs converge rounds then synthesis
- [ ] `--converge-rounds N` controls how many converge rounds run first (default 2)
- [ ] `--synthesis-threshold` filters which answers go into synthesis (default: same as `--threshold`)
- [ ] All models synthesize, but only qualifying answers are provided as input (R3 from origin)
- [ ] Synthesis evaluation uses integration/coherence/completeness/fidelity rubric (R8 from origin)
- [ ] Original user prompt is included in synthesis prompt (R7 from origin)
- [ ] If no answers qualify, returns cleanly without running synthesis (R6 from origin)
- [ ] JSON output matches converge format (winner + all_answers)
- [ ] Progress display shows both converge and synthesis phases
- [ ] `--dry-run` shows estimated total calls (converge calls + synthesis calls)

## Technical Considerations

- `phases::propose::run()` and `phases::evaluate::run()` are public functions — can be called directly without going through `Engine::run()`
- The synthesis prompt should present qualifying answers anonymously (shuffled labels A, B, C) to avoid model-name bias
- Synthesis evaluation needs its own `parse_synthesis_evaluation()` function or reuse `parse_evaluation()` if the schema is compatible
- The `Phase` enum needs a `Synthesize` variant for progress events

## Dependencies

- Requires the `converge` subcommand structure (PR #24 — merged)
- Requires `--stability-rounds` flag (PR #25 — in merge queue)
- No external dependencies

## Files Changed

| File | Change |
|---|---|
| `crates/refinery_cli/src/main.rs` | Thin routing: `Cli` + `Command` + `main()` |
| `crates/refinery_cli/src/commands/synthesize.rs` | `SynthesizeArgs` + `run()` |
| `crates/refinery_cli/src/commands/converge.rs` | `ConvergeArgs` + `run()` (extracted from main.rs) |
| `crates/refinery_cli/src/commands/common.rs` | `SharedArgs`, output types, shared helpers |
| `crates/refinery_core/src/prompts/synthesize.rs` | Synthesis prompts + schemas |
| `crates/refinery_core/src/types.rs` | Add `Synthesized`, `NoQualifyingAnswers` to `ConvergenceStatus` |
| `crates/refinery_core/src/types.rs` | Add `Phase::Synthesize` |
| `crates/refinery_core/src/phases/evaluate.rs` | Possibly extend `parse_evaluation()` for synthesis rubric |
| `README.md` | Document the `synthesize` verb |

## Sources & References

- **Origin document:** [docs/brainstorms/2026-03-18-synthesize-verb-requirements.md](docs/brainstorms/2026-03-18-synthesize-verb-requirements.md) — key decisions: all models synthesize from curated input (C), synthesis-specific eval rubric (integration/coherence/completeness/fidelity), single synthesis round
- TODO: `todos/003-verb-synthesize.md`
- Converge implementation: `crates/refinery_cli/src/main.rs:195-464`
- Phase functions: `crates/refinery_core/src/phases/propose.rs`, `evaluate.rs`
- Prompts: `crates/refinery_core/src/prompts.rs`
