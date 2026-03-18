---
date: 2026-03-18
topic: synthesize-verb
---

# Synthesize Verb

## Problem Frame

`refinery converge` returns the single best answer. But sometimes the user wants a *synthesis* — a response that integrates the strongest insights from multiple models into one cohesive piece. No single model may have produced the ideal answer, but the best parts of several answers, combined well, could be better than any individual one.

## Requirements

- R1. `refinery synthesize` runs N converge rounds first (default 2, set with `--converge-rounds`) to raise the quality bar across all models
- R2. After converge rounds, collect all answers with mean score ≥ `--synthesis-threshold` (default: same as `--threshold`)
- R3. All models receive only the qualifying answers plus the original prompt, and generate a synthesis
- R4. All models evaluate each other's syntheses using a synthesis-specific rubric (integration, coherence, completeness, prompt fidelity)
- R5. Return the best-scoring synthesis as the winner (or all with scores if no consensus, like converge)
- R6. If no answers qualify after converge rounds (none above synthesis threshold), return a "no qualifying answers" status without running the synthesis phase
- R7. The synthesis prompt must include the original user prompt so models synthesize *for* the right question/format
- R8. Synthesis evaluation uses a different rubric than converge evaluation:
  - **Integration** — weaves together insights from multiple answers, not just copies the best one
  - **Coherence** — reads as a unified piece, not a patchwork
  - **Completeness** — preserves key insights from each qualifying answer
  - **Prompt fidelity** — answers what was asked, in the format requested

## Success Criteria

- Running `refinery synthesize "prompt" --models a,b,c` produces a synthesis that demonstrably combines insights from multiple models' answers
- The synthesis phase adds meaningful value over just returning converge's winner
- A user who specifies a format in their prompt gets a synthesis that respects that format

## Scope Boundaries

- No new closing strategy — synthesis phase runs once (propose syntheses → evaluate → pick best), not iteratively
- No semantic similarity detection — qualifying answers are filtered by score only, not by content overlap
- Synthesis evaluation does not need stability rounds — single round of synthesis evaluation is sufficient
- Does not change `converge` behavior — synthesize is a separate verb that builds on top

## Key Decisions

- **All models synthesize, not just qualifiers:** Every model gets a chance to synthesize, but only qualifying *answers* are provided as input (decision C from brainstorm)
- **Converge rounds as quality filter:** Default 2 rounds to raise the bar before synthesis, configurable
- **Different eval rubric for synthesis:** Integration + coherence + completeness + prompt fidelity, not the converge rubric (accuracy + correctness)
- **Single synthesis round:** No iterative synthesis — one propose + one evaluate is enough

## Outstanding Questions

### Deferred to Planning

- [Affects R3][Technical] How to structure the synthesis prompt — should qualifying answers be presented anonymously (like converge evaluations) or attributed?
- [Affects R4][Technical] What JSON schema to use for synthesis evaluation — extend EVALUATE_SCHEMA or create SYNTHESIS_EVAL_SCHEMA?
- [Affects R5][Technical] Should the synthesis phase use the same convergence/tiebreaking logic as converge, or simpler "highest score wins"?
- [Affects R1][Technical] Can we reuse `Engine::run()` for the converge phase and then add synthesis as a post-processing step, or does the engine need a new method?

## Next Steps

→ `/ce:plan` for structured implementation planning
