---
title: "feat: brainstorm verb — score-only iteration with controversial selection"
type: feat
status: active
date: 2026-03-31
origin: docs/brainstorms/2026-03-30-brainstorm-verb-requirements.md
---

# Brainstorm Verb

## Overview

Add `refinery brainstorm` — a multi-round verb that uses score-only iteration (models see only their own prior answers + average scores) and controversial selection (quality + evaluator disagreement) to produce a panel of diverse, high-quality answers.

## Problem Frame

`converge` finds consensus. `synthesize` merges the best parts. Neither produces **breadth** — a panel of genuinely different perspectives. Brainstorm fills this gap by optimizing for quality AND diversity, returning answers that are interesting precisely because evaluators disagree about them. (see origin: `docs/brainstorms/2026-03-30-brainstorm-verb-requirements.md`)

## Requirements Trace

- R1. Multiple rounds where each model iterates independently on its own prior answers
- R2. Score-only iteration: models receive own prior answers + average scores only — no other models' content, no rationale, no suggestions
- R3. Standard evaluate phase (all models score all answers each round)
- R4. Controversial selection: answers with high quality + high evaluator disagreement rank higher
- R5. Return a panel of `--panel-size` answers
- R6. Panel answers include score distribution metadata (mean, variance, per-evaluator scores)
- R7. `--max-rounds` controls iteration (default higher than converge)
- R8. Evaluation rubric scores on originality and insight, not just correctness

## Scope Boundaries

- Score-only iteration only — no alternative iteration strategies (see origin: scope boundaries)
- Controversial selection only — alternatives deferred to TODO 013
- No embeddings or semantic similarity — diversity measured through evaluator disagreement
- No deduplication — similar but controversial answers both appear in panel
- Does not change `converge` or `synthesize` behavior

## Context & Research

### Relevant Code and Patterns

- **Verb pattern:** `crates/refinery_cli/src/commands/synthesize.rs` — most complex existing verb, best template
- **Shared args/output:** `crates/refinery_cli/src/commands/common.rs` — `SharedArgs`, `JsonOutput`, `OutputFormat`, `build_providers()`, `resolve_prompt()`
- **Command routing:** `crates/refinery_cli/src/main.rs` — `Command` enum, `async_main()` routing
- **Module exports:** `crates/refinery_cli/src/commands/mod.rs` — re-exports verbs
- **Engine:** `crates/refinery_core/src/engine.rs` — `Engine::run()`, `Session`, `model_histories`, `last_mean_scores`
- **Propose phase:** `crates/refinery_core/src/phases/propose.rs` — hardcodes `propose_with_history_prompt()` and `RoundHistory` type; cannot accept `ScoreHistory`
- **Evaluate phase:** `crates/refinery_core/src/phases/evaluate.rs` — hardcodes `evaluate_prompt()` and `EVALUATE_SCHEMA`; cannot accept a custom rubric
- **Close phase:** `crates/refinery_core/src/phases/close.rs` — `compute_mean_scores()` returns per-model means
- **Prompts:** `crates/refinery_core/src/prompts/mod.rs` — `propose_with_history_prompt()`, `round_context()`, `system_prompt()`
- **Types:** `crates/refinery_core/src/types.rs` — `RoundHistory = Vec<(String, Vec<(String, String)>)>`, `Phase` enum, `ConvergenceStatus`
- **Progress:** `crates/refinery_cli/src/progress.rs` — `ProgressDisplay`, callbacks

### Key Architecture Insight

The current `Engine::run()` loop is coupled to the own+reviews iteration strategy: `Session::next_round_with()` builds `model_histories` with full review text and passes it to `propose_with_history_prompt()`. Brainstorm cannot reuse `Engine::run()` directly because:

1. It must strip reviews from history — models get scores only
2. The round context (`round_context()`) includes winner/stability info irrelevant to brainstorm
3. The closing strategy is different — brainstorm always runs to `max_rounds`, no early convergence

**Approach:** Follow synthesize's pattern — build custom propose and evaluate loops in the CLI command, calling `provider.send_message()` directly with brainstorm-specific prompts and schemas. Both `phases::propose::run()` and `phases::evaluate::run()` are hardcoded to converge's prompts/types and cannot be reused. This is exactly what synthesize does for its Phase 3-4 (custom JoinSet-based loops with `SYNTHESIS_EVAL_SCHEMA`).

## Key Technical Decisions

- **Don't reuse `Engine::run()`, `phases::propose::run()`, or `phases::evaluate::run()`:** All three are hardcoded to converge's iteration strategy (own+reviews), prompt builders, and schemas. Rather than parameterizing shared infrastructure (which would complicate it for converge), brainstorm builds its own propose and evaluate loops calling `provider.send_message()` directly with custom prompts and schemas. This mirrors synthesize's approach for its Phase 3-4. (see origin: R2)
- **Score-only history type:** A new `ScoreHistory = Vec<(String, f64)>` type — list of `(proposal, mean_score)` tuples. Simpler than `RoundHistory` which carries full review text. A new prompt builder `propose_with_score_history_prompt()` formats this for models.
- **Controversial scoring = mean * stddev with two-key sort:** For v0, `controversy_score = mean_score * stddev`. Panel selection sorts by `(controversy_score, mean_score)` descending — mean acts as natural tiebreaker when controversy scores are equal (e.g., when all evaluators agree, stddev=0, controversy=0 for all candidates). Formula is isolated in `scoring.rs` and easy to swap if panels lack diversity.
- **No new ClosingStrategy needed:** Brainstorm always runs to `max_rounds`. No closing check. Selection happens after all rounds complete.
- **Panel selection from final-round answers only:** After the final round, compute controversy scores for each model's last-round answer (N candidates where N = model count). `--panel-size` is implicitly capped at the model count. Earlier-round answers are discarded — a tradeoff: late answers may be less original, but they've been refined through score pressure.
- **Brainstorm-specific evaluation rubric:** New `BRAINSTORM_EVAL_SCHEMA` with dimensions: originality, insight, depth, feasibility, plus rationale and overall score. **Rationale is for panel metadata and auditing only — it is never fed back to proposing models.** Only aggregate scores (mean per dimension) are passed to the next round. Schema places rationale before score (autoregressive anti-manipulation, per `docs/solutions/security-issues/prompt-injection-prevention-multi-model.md`). (see origin: R8)

## Open Questions

### Resolved During Planning

- **Can we reuse Engine::run()?** No — the iteration strategy is coupled. Orchestrate propose/evaluate phases directly from the CLI command, like synthesize does for its custom phases.
- **Do we need a new ClosingStrategy?** No — brainstorm always runs to max_rounds. No early stopping.

### Deferred to Implementation

- If `mean * stddev` doesn't produce good panels, alternative formulas to try: `stddev / (1 + abs(mean - 5))`, `mean + k * stddev`, or filtering evaluator noise (±1 point) before computing stddev.
- Default `--max-rounds` value — likely 5, but depends on how quickly models exhaust variation with score-only feedback.
- Default `--panel-size` — likely 3, but depends on typical number of models and quality distribution.
- Whether the brainstorm evaluation prompt should instruct evaluators to value disagreement explicitly, or let natural disagreement emerge from the originality/insight rubric.

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

```text
brainstorm round loop (orchestrated in commands/brainstorm.rs):

  for round in 1..=max_rounds:
    if round == 1:
      propose(prompt)                          # fresh proposals
    else:
      propose(prompt, score_histories)         # own answers + scores only

    evaluate(proposals, brainstorm_rubric)      # all models score all answers

    score_histories[model] += (proposal, mean_score)

  # after all rounds:
  for each model's final-round answer:
    compute controversy_score = f(mean, stddev of per-evaluator scores)

  rank by controversy_score descending
  return top panel_size answers with metadata
```

## Implementation Units

- [ ] **Unit 1: Score-only prompt builder and types**

  **Goal:** Add `ScoreHistory` type and `propose_with_score_history_prompt()` that presents a model's own prior answers with scores only (no reviews, no other models' content).

  **Requirements:** R1, R2

  **Dependencies:** None

  **Files:**
  - Modify: `crates/refinery_core/src/types.rs`
  - Modify: `crates/refinery_core/src/prompts/mod.rs`
  - Test: `crates/refinery_core/src/prompts/mod.rs` (inline tests)

  **Approach:**
  - Add `pub type ScoreHistory = Vec<(String, f64)>` — list of `(proposal_text, mean_score)` per round
  - Add `propose_with_score_history_prompt(user_prompt, history: &ScoreHistory) -> String` — formats history as `<your_history><round number="1"><your_proposal>...</your_proposal><score>7.2</score></round>...</your_history>` without reviews or round context
  - Add corresponding sanitizers for new XML tags (`<score>`, etc.) per `docs/solutions/security-issues/prompt-injection-prevention-multi-model.md`
  - The system prompt should encourage original thinking and novel perspectives (brainstorm-specific)

  **Patterns to follow:**
  - `propose_with_history_prompt()` in `prompts/mod.rs` — same XML structure but stripping reviews
  - `ScoreHistory` parallels `RoundHistory` but simpler

  **Test scenarios:**
  - Happy path: `propose_with_score_history_prompt` with 3-round history produces prompt containing all proposals and scores but no review text
  - Happy path: empty history falls back to standard propose prompt
  - Edge case: proposal text containing XML-like content is sanitized

  **Verification:** `cargo test --workspace` passes; new prompt builder exists and is tested

- [ ] **Unit 2: Brainstorm evaluation rubric**

  **Goal:** Add brainstorm-specific evaluation schema and prompt that scores on originality/insight rather than correctness.

  **Requirements:** R8

  **Dependencies:** None (can parallel Unit 1)

  **Files:**
  - Create: `crates/refinery_core/src/prompts/brainstorm.rs`
  - Modify: `crates/refinery_core/src/prompts/mod.rs` (add `pub mod brainstorm;`)
  - Test: `crates/refinery_core/src/prompts/brainstorm.rs` (inline tests)

  **Approach:**
  - `BRAINSTORM_EVAL_SCHEMA` — JSON schema with: originality (1-10), insight (1-10), depth (1-10), feasibility (1-10), rationale (string), score (1-10)
  - `brainstorm_evaluate_prompt()` — instructs evaluators to value novelty, surprising connections, and depth of thinking over conventional correctness
  - Follow synthesize's pattern: `SYNTHESIS_EVAL_SCHEMA` + `synthesize_evaluate_prompt()`

  **Patterns to follow:**
  - `crates/refinery_core/src/prompts/synthesize.rs` — schema constants + prompt builder functions

  **Test scenarios:**
  - Happy path: `brainstorm_evaluate_prompt` produces prompt mentioning originality and insight dimensions
  - Happy path: `BRAINSTORM_EVAL_SCHEMA` is valid JSON schema with required fields

  **Verification:** `cargo test --workspace` passes; schema and prompt exist and are tested

- [ ] **Unit 3: Controversy scoring**

  **Goal:** Implement controversy score computation from per-evaluator scores.

  **Requirements:** R4, R6

  **Dependencies:** None (can parallel Units 1-2)

  **Files:**
  - Create: `crates/refinery_core/src/scoring.rs` (or add to existing close.rs)
  - Modify: `crates/refinery_core/src/lib.rs` (export module)
  - Test: `crates/refinery_core/src/scoring.rs` (inline tests)

  **Approach:**
  - `controversy_score(scores: &[f64]) -> f64` — computes a composite of mean and standard deviation
  - `PanelCandidate { model_id, answer, mean_score, stddev, controversy_score, per_evaluator_scores }` — struct for panel selection
  - `select_panel(candidates: &[PanelCandidate], panel_size: usize) -> Vec<PanelCandidate>` — sort by `(controversy_score, mean_score)` descending, take top N

  **Patterns to follow:**
  - `compute_mean_scores()` in `crates/refinery_core/src/phases/close.rs` — similar score aggregation

  **Test scenarios:**
  - Happy path: answer with mean=7.0 and scores [3, 5, 9, 10] (high stddev) ranks higher than mean=7.0 and scores [6, 7, 7, 8] (low stddev)
  - Happy path: `select_panel` with panel_size=3 returns top 3 by controversy score
  - Edge case: all answers have identical scores — controversy scores are all 0, panel picks by mean
  - Edge case: single evaluator — stddev is 0, falls back to mean score
  - Edge case: panel_size larger than candidates — returns all candidates

  **Verification:** `cargo test --workspace` passes; controversy scoring ranks high-variance answers above uniform ones

- [ ] **Unit 4: Types and status updates**

  **Goal:** Add `Phase::Brainstorm` variant and `ConvergenceStatus::Brainstormed` for brainstorm-specific progress and output.

  **Requirements:** R5, R6

  **Dependencies:** None (can parallel Units 1-3)

  **Files:**
  - Modify: `crates/refinery_core/src/types.rs`
  - Test: `crates/refinery_core/src/types.rs` (inline tests)

  **Approach:**
  - Add `Phase::Brainstorm` to the Phase enum
  - Add `ConvergenceStatus::Brainstormed` for successful brainstorm completion
  - Ensure serde serialization produces `"brainstormed"`

  **Patterns to follow:**
  - `Phase::Synthesize` and `ConvergenceStatus::Synthesized` — exact same pattern

  **Test scenarios:**
  - Happy path: `ConvergenceStatus::Brainstormed` serializes to `"brainstormed"`
  - Happy path: `Phase::Brainstorm` display renders correctly

  **Verification:** `cargo test --workspace` passes; new variants exist and serialize correctly

- [ ] **Unit 5: CLI command — `refinery brainstorm`**

  **Goal:** Implement the brainstorm CLI command with score-only round loop, brainstorm evaluation, controversial panel selection, and both text/JSON output.

  **Requirements:** R1-R8

  **Dependencies:** Units 1-4

  **Files:**
  - Create: `crates/refinery_cli/src/commands/brainstorm.rs`
  - Modify: `crates/refinery_cli/src/commands/mod.rs` (add `pub mod brainstorm;`)
  - Modify: `crates/refinery_cli/src/main.rs` (add `Brainstorm` variant to `Command` enum)

  **Approach:**
  - `BrainstormArgs` with `shared: SharedArgs`, `--max-rounds` (default 5), `--panel-size` (default 3), `--threshold` (minimum mean score for panel inclusion)
  - Validate args before I/O (panel_size >= 1, max_rounds >= 1)
  - Dry-run: estimate calls (`N * max_rounds` propose + `N*(N-1) * max_rounds` evaluate)
  - Round loop orchestrated directly (not via Engine::run() or phases::*::run()):
    1. Build providers and resolve prompt
    2. For each round: build propose messages using `propose_with_score_history_prompt()`, dispatch via JoinSet + semaphore + `provider.send_message()`. Then build evaluate messages using `brainstorm_evaluate_prompt()` + `BRAINSTORM_EVAL_SCHEMA`, dispatch similarly.
    3. After each round: extract per-evaluator scores from eval response JSON (follow synthesize's inline parsing pattern — parse `score` field directly, apply float coercion `as_u64().or_else(as_f64().round())`), update `ScoreHistory` per model
    4. After all rounds: compute controversy scores for final-round answers, select panel
  - Output: panel of answers with score metadata (mean, stddev, controversy score, per-evaluator scores)
  - JSON output: new `BrainstormJsonOutput` with `status`, `panel: Vec<PanelAnswer>`, `metadata`
  - Text output: numbered panel with scores
  - Artifact saving if `--output-dir` provided

  **Patterns to follow:**
  - `crates/refinery_cli/src/commands/synthesize.rs` — custom phase orchestration, error handling, output formatting
  - `crates/refinery_cli/src/commands/converge.rs` — simpler verb structure for reference

  **Test scenarios:**
  - Happy path: `--dry-run` with 3 models estimates correct call count
  - Happy path: `--dry-run` with invalid args returns exit code 4
  - Edge case: `--panel-size` larger than model count — returns all models' answers
  - Edge case: single model — returns that model's answer as sole panel member (no evaluation, short-circuit)
  - Error path: all models fail in propose — emits error in both text and JSON formats
  - Error path: `--output-format json` on late failures emits `ErrorResponse`
  - Integration: score-only history is correctly built round-over-round (proposals accumulate, scores update, no review text leaks)

  **Verification:** `cargo fmt --all --check && cargo clippy --workspace -- -D warnings && cargo test --workspace` all pass; `refinery brainstorm --help` shows correct flags; `refinery brainstorm --dry-run` produces estimates

## System-Wide Impact

- **Interaction graph:** New `Command::Brainstorm` variant in main.rs routes to `commands::brainstorm::run()`. Does NOT reuse `phases::propose::run()` or `phases::evaluate::run()` — builds its own JoinSet-based loops with custom prompts/schemas (same pattern as synthesize). No changes to existing phases. Adds `brainstorm` module to prompts. New `<your_history>` and `<score>` XML tags require corresponding sanitizers.
- **Error propagation:** Follows synthesize's pattern — provider errors during propose/evaluate are counted and reported. Late failures emit `ErrorResponse` for JSON output.
- **State lifecycle risks:** None — brainstorm is stateless between invocations. Round state is local to the command.
- **API surface parity:** New subcommand only — does not change converge or synthesize behavior.
- **Integration coverage:** The round loop orchestration (calling propose + evaluate phases directly with custom history) is the key integration seam. Unit tests in refinery_core cover scoring; the CLI command integration is verified via dry-run and manual testing.
- **Unchanged invariants:** `Engine::run()`, `Session`, `ClosingStrategy`, `phases::propose::run()`, `phases::evaluate::run()`, converge, and synthesize are all untouched.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Controversy score formula may not produce genuinely diverse panels | Start with mean * stddev, tune based on real runs. The formula is isolated in `scoring.rs` and easy to change. |
| LLMs may converge despite score-only feedback (trained to improve) | Score-only iteration removes the strongest convergence signal (reviews). If models still converge, the brainstorm evaluation rubric (valuing originality) provides counter-pressure. |
| Brainstorm evaluation rubric may not meaningfully differ from converge's | Test with real prompts early. The rubric dimensions (originality, insight, depth, feasibility) are explicitly different from converge's (accuracy, correctness). |

## Sources & References

- **Origin document:** [docs/brainstorms/2026-03-30-brainstorm-verb-requirements.md](docs/brainstorms/2026-03-30-brainstorm-verb-requirements.md) — key decisions: score-only iteration, controversial selection, panel output
- **Verb design brainstorm:** `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
- **TODO:** `todos/004-verb-brainstorm.md`
- **Deferred benchmarks:** `todos/013-brainstorm-strategy-benchmarks.md`
- **Synthesize verb (template):** `crates/refinery_cli/src/commands/synthesize.rs`
