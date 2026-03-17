---
title: "feat: synthesize verb — converge then synthesize best answers"
priority: medium
milestone: v0.3
depends_on: 002-cli-subcommand-converge
---

# Verb: `refinery synthesize`

## Behavior

1. Run the converge loop (propose → evaluate → close) for N rounds
2. After convergence or max rounds, collect all answers scoring above `--synthesis-threshold`
3. Each model generates a synthesis of those qualifying answers
4. Models review and score each other's syntheses (separately)
5. Return the best-scoring synthesis (or all with scores)

## Flags

- `--synthesis-threshold` — minimum score for an answer to be included in synthesis input (default: threshold)
- `--max-rounds` — rounds for the initial converge phase

## Prompts

Synthesis phase needs custom prompts:
- System prompt explaining the synthesis task
- User prompt presenting all qualifying answers
- Evaluation prompt for scoring syntheses (different criteria than converge — coherence, completeness, integration of perspectives)

## References

- Brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
