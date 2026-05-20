---
title: "feat: mine verb — extract recurring themes and outliers"
priority: low
milestone: v0.4
depends_on: 002-cli-subcommand-converge
---

# Verb: `refinery mine` (name TBD)

## Behavior

Run multiple rounds, then analyze all answers across all rounds to identify:
1. **Recurring themes** — what comes up the most across models and rounds
2. **Outliers** — what strays the most from the overall consensus

Returns a structured report: clusters of similar ideas + notable deviations.

## Alternative Names

- `distill` — extract the essence
- `extract` — pull out themes
- `survey` — map the landscape
- `diverge` — find what's different

## Selection Strategy

- **Theme detection:** Cluster answers by similarity, label each cluster
- **Outlier detection:** Find answers that don't fit any cluster, or that a minority of models consistently produce but others don't

Inspired by Reddit's algorithm landscape — not just what's popular, but what's *interesting* about the distribution of opinions.

## Flags

- `--cluster-count` — expected number of themes (or auto-detect)
- `--outlier-threshold` — how far from consensus counts as an outlier
- `--max-rounds` — rounds for collecting data

## Open Questions

- This verb might need a fundamentally different output format (not a single answer or panel, but a structured analysis)
- Whether to use the models themselves for clustering ("which of these answers are saying the same thing?") or statistical methods
- Whether this is different enough from `brainstorm` to justify a separate verb

## References

- Brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
