---
title: "feat: evolve verb — Darwinian blind variation with score-only selection pressure"
priority: medium
milestone: v0.4
depends_on: 002-cli-subcommand-converge
---

# Verb: `refinery evolve`

## Behavior

Darwinian evolutionary process. Models iterate on their own answers in independent lineages. No model ever sees another model's work. Selection pressure comes from scores alone — no rationale, no feedback, no cross-pollination.

Key distinction from converge: converge is **Lamarckian** (directed improvement — models see evaluations, suggestions, other answers). Evolve is **Darwinian** (blind variation + external selection).

## Process

1. All models propose from the prompt (round 1)
2. External evaluation scores each answer (standard evaluate phase)
3. **Survivors** (above cull threshold): receive the original prompt, their own prior answers, and each answer's average score. No rationale, no other models' work.
4. **Culled** (below cull threshold): receive only the original prompt. Fresh start — lineage destroyed.
5. Repeat until `--max-rounds`
6. Return all surviving answers from the final round

## What Makes It Distinct

- **Independent lineages:** models never see each other's answers. Each evolutionary tree is isolated.
- **No directed feedback:** surviving models get scores but no rationale or suggestions. The only signal is "how well did I do."
- **Death = restart:** culled models don't get dropped — they start fresh from the prompt, injecting diversity naturally. A model starting from scratch in round 4 will produce something different from round 1.
- **The wall:** variation (proposing) and selection (evaluating) are strictly separated. Evaluating models never inform proposing models beyond a number.

## Selection Strategy

Default: cull models whose scores fall more than N standard deviations below the group mean.

- Adaptive to the actual distribution — if all models cluster tight, nobody dies (noise, not signal)
- Genuinely bad answers are far from the group and get culled
- Early rounds (noisy, wide spread) → more culling. Later rounds (answers improve, cluster) → fewer restarts. Self-regulating.
- This is a strategy concern, not baked into the process — other selection strategies (fixed threshold, bottom-N%, tournament) could be swapped in.

## Flags

- `--max-rounds` — total generations (default: 5-10, higher than converge since progress is slower)
- `--cull-threshold` — standard deviations below mean to trigger restart (default: 1.0)
- Standard shared flags (models, timeout, output-format, etc.)

## Prompt Design Challenges

- LLMs are trained to be helpful — asking them to "vary without necessarily improving" may not produce genuine blind variation
- With score-only feedback (no rationale), models may not know what to change — but that's the point. The randomness of their attempts IS the mutation.
- The propose prompt for survivors needs to present prior answers + scores without directing improvement

## Output

All surviving answers from the final round, with their lineage metadata (which rounds they survived, score trajectory).

## Open Questions

- Does showing a model its own score history give too much directed signal? Should survivors get even less — just their prior answers, no scores?
- What's the right default for `--max-rounds`? Evolutionary processes need more generations than converge.
- Should crossover exist? (Model receives TWO parent answers from different lineages and must combine them.) This breaks lineage isolation but could be powerful.
- What's the minimum number of models for evolve to be meaningful? With 2 models, one dies and restarts every round — is that useful?

## References

- Verbs brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
