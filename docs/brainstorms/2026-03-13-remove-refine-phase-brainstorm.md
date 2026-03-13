# Remove the Refine Phase from the Consensus Loop

**Date:** 2026-03-13
**Status:** Decided

## What We're Building

Remove the REFINE phase from the consensus loop entirely. The loop becomes **Propose → Evaluate → Close** (was Propose → Evaluate → Refine → Close).

Round N>1 propose prompts are enriched with the model's full history: all its own prior proposals and all reviews it received, organized as per-round pairs. This gives models the same feedback loop the refine phase provided, without a redundant phase or score/answer mismatch.

## Why This Approach

Two problems with the current refine phase:

1. **Redundancy.** Refinement does the same thing as a feedback-aware proposal: take your previous answer + reviews, produce an improved answer. A refine step followed by a propose step is doing the same work twice, burning N extra API calls per round.

2. **Score/answer mismatch.** Scores are computed on proposals, but the "winning answer" returned to the user is a refinement that was never scored by anyone. The convergence check says "this proposal scored 9.2" but the output is a different text entirely.

The fix: fold refinement into proposal. Round N>1 proposals include the model's full trajectory (all prior proposals + reviews per round), so models naturally improve. The winning answer is the actual scored proposal.

## Key Decisions

- **Feedback format:** Each model sees all of its own prior proposals and all reviews it received, organized as per-round pairs (round 1: proposal + reviews, round 2: proposal + reviews, ...). Not just the latest round.
- **No cross-model proposal sharing:** Models don't see other models' proposals. They only see reviews of their own work (which may reference other proposals indirectly).
- **Scope:** Only remove refine. Don't change evaluate or close logic.
- **Cost formula:** Drops from N²+N to N² per round (remove N refine calls).

## Blast Radius

### Delete
- `phases/refine.rs` — entire file
- `RefinementSet` type
- `refine_prompt()` and `sanitize_for_review_tag()` in prompts.rs
- `Phase::Refine` variant
- `ModelRefined` / `ModelRefineFailed` progress events

### Modify
- `engine.rs` — remove refine phase call, set `last_answers` from proposals instead of refinements, store per-round reviews for history injection
- `propose.rs` — accept and render prior proposals + reviews in round N>1
- `prompts.rs` — new `propose_with_history_prompt()` or enriched `propose_prompt()`
- `RoundOutcome` — remove `refinements` field
- `types.rs` — update cost formula comment, remove RefinementSet
- `progress.rs` — remove refine event variants
- `cli/main.rs` — remove refine artifact export and progress rendering arms

### Preserve
- Evaluate phase — unchanged
- Close phase — unchanged
- `sanitize_for_delimiter()`, `wrap_answer()` — still needed for evaluate

## Open Questions

None — scope is clear and constrained.
