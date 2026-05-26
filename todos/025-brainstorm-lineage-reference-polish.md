---
title: "chore: consider suppressing brainstorm lineage references"
priority: low
milestone: v0.5
created: 2026-05-25
origin: 024-brainstorm-suppress-score-history-meta-preambles
---

# Consider Suppressing Brainstorm Lineage References

## Problem

`todos/024` suppresses explicit score-history meta-preambles such as "Based on my Round 1 score..." and benchmark/selection mechanics. The validation run reached `meta_preamble_rate: 0.0` using the current analyzer.

However, one selected answer still used softer lineage wording such as "Builds on ...". This does not mention scores, rounds, or benchmarks, so it is outside the measured `meta_preamble_rate`, but it can still make an answer feel slightly process-oriented rather than fully standalone.

## Candidate Fix

If future demos or human review consider this distracting, tighten brainstorm proposal prompts further:

- Ask models not to refer to their own prior concepts or say an answer "builds on" earlier attempts.
- Preserve the useful score-only trajectory internally; do not remove prior proposals from the prompt unless benchmarked.
- Optionally extend `benchmark-brainstorm` with a separate `lineage_reference_rate` detector so this is not conflated with score-history meta-preambles.

## Acceptance Criteria

- Decide whether lineage references are actually harmful in user-facing panels.
- If yes, add prompt tests for the narrower instruction.
- If analyzer support is added, keep `meta_preamble_rate` focused on explicit score/round/benchmark mechanics and add a separate metric for lineage references.

## Notes

This is not required for `todos/024`, which was completed by suppressing explicit score-history meta-commentary and validating a reduced `meta_preamble_rate`.
