---
title: "feat: brainstorm verb — optimize for quality and diversity"
priority: medium
milestone: v0.3
depends_on: 002-cli-subcommand-converge
---

# Verb: `refinery brainstorm`

## Behavior

Multi-round process optimizing for quality AND variety. Instead of converging on a single winner, breed diversity through rounds and return a panel of non-overlapping answers.

## Two Orthogonal Axes

### Iteration Strategy (what models see between rounds)

**v0 choice: score-only.** Prompt + own prior answers + scores only. No other models' content, no rationale. Independent exploration, no conformity pressure.

Future iteration strategies to explore and benchmark (see TODO 013):
- Own+reviews (converge/synthesize today — prompt + own prior answers + evaluations with rationale)
- Full visibility (everything: all models' answers, all evaluations)
- Cluster labels ("3 about cost, 2 about culture — go elsewhere")
- Negative-only ("these topics are taken: ...")
- Blind (prompt only, no round context)
- Similarity-based (measure proximity between answers, do something with it)

### Selection Strategy (what makes a "good" brainstorm answer)

**v0 choice: Reddit "Controversial" algorithm.** Select answers with high quality (many high scores) BUT high disagreement (high variance across evaluators). An answer that half the models love and half dislike is more interesting than one everyone rates 7.

**v0 formula:** `controversy = mean * stddev` (population stddev of per-evaluator scores). Panel selection uses two-key sort `(controversy, mean)` descending for deterministic tiebreaking. Isolated in `refinery_core::scoring` — easy to swap.

Other possible implementations (deferred):
- **Reddit-style:** adapted from `upvotes / (upvotes + downvotes)` — needs binary mapping from continuous scores
- **Pure variance:** keep answers where evaluator scores have high standard deviation regardless of mean

Future selection strategies to explore and benchmark (see TODO 013):
- Semantic deduplication (cluster similar answers, keep one per cluster)
- Model-as-judge diversity assessment
- Combined controversy + diversity scoring

## Process

1. All models propose from the prompt (round 1)
2. External evaluation scores each answer (standard evaluate phase)
3. Models receive: the original prompt, their own prior answers, and each answer's average score. No rationale, no other models' work.
4. Repeat until `--max-rounds`
5. Select final panel using controversy/variance scoring
6. Return panel of diverse answers

## Flags

- `--panel-size` — number of diverse answers to return (default: 3-5)
- `--max-rounds` — rounds for breeding diversity
- Standard shared flags (models, timeout, output-format, etc.)

## Prompts

Different from converge:
- System prompt encouraging original thinking and novel perspectives
- Propose prompt presenting own prior answers + scores, asking for unique angles
- Evaluate prompt that scores on originality and insight, not just correctness

## Output

Panel of `--panel-size` answers, selected for quality + disagreement. Each with score distribution metadata (mean, variance, per-evaluator scores).

## Open Questions

- How to measure diversity without embeddings? (May not need to for v0 — controversy score handles it indirectly)
- Does the selection strategy need a diversity metric alongside score, or is controversy scoring sufficient?
- What's the right default for `--panel-size`?

## References

- Verbs brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
