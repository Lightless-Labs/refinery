---
title: "fix: suppress brainstorm score-history meta-preambles"
priority: medium
milestone: v0.4
created: 2026-05-23
depends_on: 013-brainstorm-strategy-benchmarks
---

# Suppress Brainstorm Score-History Meta-Preambles

## Problem

Brainstorm score-only iteration prompts include prior answers and scores. Models often surface that process in final answers with phrases like:

- "Based on my Round 1 score..."
- "My previous proposal got..."
- "The feedback suggests..."

The six-prompt benchmark measured an average `meta_preamble_rate` of `0.333` across selectors. This is noisy in user-facing panels and can make outputs feel like benchmark artifacts rather than direct answers.

Reports:

- `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md`
- `docs/brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md`

## Candidate Fix

Prompt polish for `prompts::propose_with_score_history_prompt()` / brainstorm-specific instructions:

- Tell models to use prior scores internally, but do not mention scores, previous rounds, or benchmark mechanics in the final answer.
- Consider a final-answer cleanup pass only if prompt-only mitigation is insufficient.
- Keep score-only signal; do not feed evaluator rationales or other models' content.

## Acceptance Criteria

- Add prompt tests that the instruction forbidding score-history meta-commentary is present.
- Re-run at least two benchmark prompts and confirm `meta_preamble_rate` decreases.
- Ensure the prompt still includes the score history needed for score-only iteration.

## Origin

Follow-up from `todos/013` six-prompt benchmark.
