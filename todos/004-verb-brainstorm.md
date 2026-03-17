---
title: "feat: brainstorm verb — optimize for quality and diversity"
priority: medium
milestone: v0.3
depends_on: 002-cli-subcommand-converge
---

# Verb: `refinery brainstorm`

## Behavior

Multi-round process optimizing for quality AND variety. Instead of converging on a single winner, breed diversity through rounds and return a panel of non-overlapping answers.

## Selection Strategy

Inspired by Reddit's "Controversial" algorithm: select answers with high quality (many high scores) but high disagreement (high variance across evaluators). An answer that half the models love and half dislike is more interesting than one everyone rates 7.

Possible scoring approaches:
- **Controversy score:** `upvotes / (upvotes + downvotes)` close to 0.5 with high total
- **Score variance:** keep answers where evaluator scores have high standard deviation
- **Semantic deduplication:** cluster similar answers and keep one representative per cluster
- **Model-as-judge:** ask a model to assess whether two answers are "substantially different"

## Flags

- `--panel-size` — number of diverse answers to return (default: 3-5)
- `--diversity-threshold` — how different answers must be to count as distinct
- `--max-rounds` — rounds for breeding diversity

## Prompts

Different from converge:
- System prompt encouraging original thinking and novel perspectives
- Propose prompt that explicitly asks for unique angles (round 2+ shows what's already been said)
- Evaluate prompt that scores on originality and insight, not just correctness

## Open Questions

- How to measure diversity without embeddings?
- Should models see each other's answers between rounds (breeds conformity) or not (breeds repetition)?
- Does the closing strategy need a diversity metric alongside score?

## References

- Brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
