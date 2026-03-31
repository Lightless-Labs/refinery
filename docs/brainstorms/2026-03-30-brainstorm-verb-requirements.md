---
date: 2026-03-30
topic: brainstorm-verb
---

# Brainstorm Verb

## Problem Frame

`refinery converge` finds consensus. `refinery synthesize` merges the best parts into one answer. But sometimes the user wants **breadth** — a panel of high-quality, genuinely different perspectives on a question. No single answer or synthesis is the goal; the goal is exploring the solution space and returning the most interesting diversity it contains.

## Requirements

- R1. `refinery brainstorm` runs multiple rounds where each model iterates independently on its own prior answers
- R2. Models receive only their own prior answers and each answer's average score between rounds — no other models' content, no rationale, no suggestions (score-only iteration)
- R3. Evaluation uses the standard evaluate phase (all models score all answers each round)
- R4. Selection uses a "controversial" algorithm: answers that score high overall BUT have high evaluator disagreement rank higher than answers with uniform mid-range scores
- R5. Return a panel of `--panel-size` answers selected for quality + disagreement
- R6. Each answer in the output panel includes score distribution metadata (mean, variance, per-evaluator scores)
- R7. `--max-rounds` controls total iteration rounds (default higher than converge — this is exploration, not convergence)
- R8. The evaluation prompt scores on originality and insight, not just correctness — different rubric from converge

## Success Criteria

- Running `refinery brainstorm "prompt" --models a,b,c` returns a panel of answers that are meaningfully different from each other
- The panel contains answers that wouldn't survive converge (high disagreement would kill them in consensus-seeking)
- A subject-matter expert reviewing the panel finds perspectives they hadn't considered

## Scope Boundaries

- v0 uses score-only iteration strategy only — alternative iteration strategies (own+reviews, full visibility, cluster labels, negative-only, blind, similarity-based) are deferred to benchmarking (TODO 013)
- v0 uses controversial selection only — alternative selection strategies deferred to benchmarking (TODO 013)
- No embeddings or semantic similarity in v0 — diversity is measured indirectly through evaluator disagreement
- No deduplication in v0 — if two answers happen to be similar but both controversial, both appear in the panel
- Does not change `converge` or `synthesize` behavior

## Key Decisions

- **Score-only iteration:** Models never see other models' work. Each lineage evolves independently. This avoids conformity pressure — the main risk in brainstorming.
- **Controversial = quality + disagreement:** An answer half the evaluators love and half dislike is more interesting than one everyone rates 7. This is the core insight from Reddit's Controversial algorithm.
- **Panel output, not single winner:** The verb returns N answers, not one. This is structurally different from converge/synthesize.
- **Standard evaluate phase for scoring:** Reuse the existing evaluate infrastructure. The selection strategy (controversial) operates on the scores after evaluation, not during.

## Outstanding Questions

### Deferred to Planning

- [Affects R4][Technical] Concrete formula for controversy scoring — Reddit's `upvotes/(upvotes+downvotes)` needs adaptation since we have continuous scores (1-10), not binary up/down. Score variance (standard deviation) is the simplest proxy. Are there better formulas?
- [Affects R5][Technical] How to select the final panel — top N by controversy score? Or cluster by controversy score and pick representatives? Simple top-N is the v0 default.
- [Affects R8][Technical] What dimensions should the brainstorm evaluation rubric score on? Candidates: originality, insight, depth, provocativeness, feasibility. Needs to differ from converge's accuracy/correctness focus.
- [Affects R2][Technical] Prompt structure for score-only iteration — how to present a model's own prior answers + scores without directing improvement. "Here are your previous attempts and how they were received" vs more neutral framing.
- [Affects R7][Needs research] What's the right default for `--max-rounds`? Brainstorming needs more rounds than converge (exploration vs convergence), but too many rounds may cause models to exhaust their variation. Likely 5-10.
- [Affects R5][Needs research] What's the right default for `--panel-size`? Too small (2) isn't a panel. Too large (10) dilutes quality. Likely 3-5.
- [Affects R1][Technical] Can we reuse `Engine::run()` for the round loop, or does score-only iteration require a different engine mode? The key difference: models don't see round context (other answers + evaluations), only their own history.

## Next Steps

→ `/ce:plan` for structured implementation planning
