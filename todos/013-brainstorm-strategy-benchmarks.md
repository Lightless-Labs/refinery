---
title: "research: benchmark iteration and selection strategies for brainstorm verb"
priority: low
milestone: v0.4
depends_on: 004-verb-brainstorm
---

# Benchmark: Brainstorm Iteration and Selection Strategies

## Goal

After brainstorm v0 ships (score-only iteration + controversial selection), benchmark alternative strategies on both axes to find what actually produces the best diverse panels.

## Iteration Strategies to Benchmark

What models see between rounds:

1. **Score-only** (v0 baseline) — prompt + own prior answers + scores only
2. **Own+reviews** (converge/synthesize today) — prompt + own prior answers + evaluations (scores + rationale + suggestions)
3. **Full visibility** — everything: all models' answers, all evaluations, all scores. Risk: conformity.
4. **Cluster labels** — prompt + topic summaries of what exists ("3 answers about cost, 2 about culture — go elsewhere"). Risk: shallow diversity.
5. **Negative-only** — prompt + list of taken topics ("these topics are taken: ..."). Risk: over-constraining.
6. **Blind** — prompt only, no context from prior rounds. Pure independent generation. Baseline for comparison.
7. **Similarity-based** — measure proximity between answers (TF-IDF, Jaccard, embeddings?) and use that signal somehow. Explore: as iteration feedback? As selection input? As post-processing dedup?

## Selection Strategies to Benchmark

1. **Controversial** (v0 baseline) — high quality + high evaluator disagreement
2. **Score variance** — keep answers with high standard deviation across evaluators
3. **Semantic deduplication** — cluster answers, keep one representative per cluster
4. **Model-as-judge diversity** — ask a model to assess pairwise diversity
5. **Combined** — controversy + diversity as a composite score

## Benchmark Design

TBD — needs a way to evaluate "panel quality" itself. Possible:
- Human evaluation of panel diversity and quality
- Automated: measure topic coverage across panel members
- Automated: pairwise similarity within the panel (lower = more diverse)

## References

- Brainstorm verb: `todos/004-verb-brainstorm.md`
