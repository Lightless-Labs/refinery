---
title: "feat: brainstorm divergence expansion via prompt reframing and domain collisions"
priority: medium
milestone: v0.4
depends_on: 004-verb-brainstorm
created: 2026-04-24
---

# Brainstorm Divergence Expansion

## Goal

Extend `refinery brainstorm` with optional upstream divergence mechanisms that generate a wider set of independent lineages before score-only iteration and controversial panel selection.

The v0 brainstorm loop already **preserves divergence** by preventing models from seeing peer answers or review rationale. This TODO is about **injecting divergence** before the loop starts.

## Mechanism 1: Prompt Reframing Expansion

For `n` models:

1. Keep the user's original prompt as the anchor (`P0`).
2. Ask each model to generate one strategically different prompt variation (`P1..Pn`).
3. Have every model independently brainstorm against every prompt.

This yields:

```text
lineages = n models × (1 original prompt + n model-generated variations)
         = n(n + 1)
```

For 3 models, this creates 12 independent lineages.

### Prompt Variation Requirements

Prompt variations should not be paraphrases. They should reframe the problem by changing one or more of:

- assumed user or neglected stakeholder
- success criterion
- time horizon
- causal model
- domain metaphor
- constraint set
- level of abstraction
- risk appetite
- inversion of the goal
- mechanism-vs-idea framing
- second-order effects

The original prompt must remain in the lineage set as an anchor against drift.

## Mechanism 2: Domain Collision Expansion

Inspired by Open Collider (`https://github.com/CL-ML/open-collider`) and Koestler-style bisociation: generate structurally distant domains and force isolated collisions between the brief and each domain.

Each domain entry should include:

- specialist persona
- counterintuitive mechanism
- bridge question into the user's problem

Example lineage expansion:

```text
lineages = n models × (1 original + p prompt variations) × d distant domains
```

If `p = n`, this becomes `n(n + 1)d`, which is high-volume and needs budget controls.

## Proposed CLI Controls

Do not make full expansion default. Add explicit controls:

```sh
refinery brainstorm "..." \
  --prompt-variants off|per-model|N \
  --lineage-budget N \
  --collide-domains N \
  --ideas-per-lineage N \
  --evaluation-sample-size N
```

Possible staged rollout:

1. `--prompt-variants per-model` only, no domain collisions.
2. `--collide-domains N` as a separate strategy.
3. Combination mode once budget controls and selection are proven.

## Selection Implications

The v0 panel selection formula (`mean * stddev`) may not be enough once there are many prompt/domain lineages. Benchmark alternatives before changing defaults.

Potential future composite:

```text
panel_score = quality
            + disagreement_bonus
            + prompt_variant_coverage_bonus
            + novelty_bonus
            - redundancy_penalty
```

Need to preserve weird-but-useful candidates; do not over-normalize into consensus during curation.

## Benchmark Questions

- Does prompt reframing produce more useful diversity than v0 score-only brainstorm?
- Does domain collision outperform a simple "be original" prompt?
- Does `prompt variants × domain collisions` justify the call cost?
- Does controversial selection still work when many lineages share the same model but different frames?
- Do judges over-favor flashy weirdness over operationally useful ideas?

## References

- Brainstorm v0 plan: `docs/plans/2026-03-31-001-feat-brainstorm-verb-plan.md`
- Brainstorm strategy benchmarks: `todos/013-brainstorm-strategy-benchmarks.md`
- Brainstorm verb: `todos/004-verb-brainstorm.md`
- Open Collider: `https://github.com/CL-ML/open-collider`
