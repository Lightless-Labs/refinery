---
date: 2026-03-17
topic: cli-subcommand-verbs
---

# CLI subcommand verbs

## What We're Building

Restructure the refinery CLI from a flat `refinery "prompt" --models ...` to a verb-based `refinery <verb> "prompt" --models ...` structure. The first verb is `converge` (existing behavior). Future verbs include `synthesize`, `brainstorm`, and others.

Each verb is a different *selection strategy* applied to the same multi-round multi-model loop. They differ in:
- What they optimize for (agreement vs diversity vs themes)
- The prompts sent to models at each phase
- What they return (single winner vs panel vs clusters)

## Why This Approach

A flat CLI with all flags at the top level doesn't scale. Adding `synthesize` would need `--synthesis-threshold`, `brainstorm` would need `--diversity-target`, etc. Subcommands give each verb its own flag namespace.

No backward compatibility needed — nobody is using the current flat form in production.

## Verb Design

### converge (existing)
Optimize for agreement. Return the winner (or no winner if max rounds exceeded).

**Selection model:** Reddit "Top/Best" — highest consensus score with stability requirement.

**Specific flags:** `--threshold`, `--max-rounds`, `--stability-rounds`

### synthesize (future)
Like converge, but after reaching consensus/max rounds, each model generates a synthesis of all answers scoring above threshold. Models then review each other's syntheses separately, score them, and return the best.

**Selection model:** Two-phase — converge to collect quality answers, then synthesize and rank.

**Specific flags:** `--synthesis-threshold`, `--max-rounds`

### brainstorm (future)
Multi-round process optimizing for quality AND variety. Breed diversity through rounds. Return a panel of non-overlapping answers.

**Selection model:** Reddit "Controversial" — high quality (many high scores) but high disagreement (high variance across evaluators). An answer that half the models love and half dislike is more interesting than one everyone rates 7.

**Specific flags:** `--panel-size`, `--diversity-threshold`, `--max-rounds`

**Open question:** How to score for diversity? Options:
- Controversy score: `upvotes / (upvotes + downvotes)` close to 0.5 with high total votes
- Wilson score lower bound (for confidence with few evaluators)
- Semantic similarity detection between answers (needs embedding or model-as-judge)
- Simple: keep answers where evaluator scores have high standard deviation

### mine (future, name TBD)
Find recurring themes AND outliers across rounds. Like brainstorm but focused on extraction rather than generation.

**Selection model:** Cluster analysis + outlier detection. What comes up the most? What strays the most from consensus?

**Specific flags:** `--cluster-count`, `--outlier-threshold`

## Reddit Algorithm Parallels

| Reddit Algorithm | Refinery Verb | What It Selects For |
|---|---|---|
| Top | converge | Highest raw score |
| Best (Wilson) | converge (with few models) | Confidence-adjusted score |
| Controversial | brainstorm | High engagement + high disagreement |
| Rising | *(potential verb)* | Ideas gaining traction across rounds |

## Flag Structure

**Shared (top-level) flags:**
- `--models` — comma-separated model list
- `--timeout` — per-call wall-clock timeout
- `--idle-timeout` — per-call idle timeout
- `--max-concurrent` — concurrent subprocess limit
- `--output-format` — text or json
- `--output-dir` — artifact output directory
- `--allow-tools` — tools to enable
- `--verbose` / `--debug` — logging
- `--dry-run` — cost estimate

**Per-verb flags:**
- `converge`: `--threshold`, `--max-rounds`, `--stability-rounds`
- `synthesize`: `--synthesis-threshold`, `--max-rounds`
- `brainstorm`: `--panel-size`, `--diversity-threshold`, `--max-rounds`

## Implementation Plan

1. **This PR:** Restructure CLI into `refinery converge ...`. Move all current flags under the `converge` subcommand. No new behavior.
2. **Future PRs:** Add verbs one at a time, each with their own prompts, selection strategy, and output format.

## Key Decisions

- **No backward compatibility:** `refinery "prompt"` without a verb will error
- **Shared flags on the parent command:** Models, timeout, output format are universal
- **Per-verb prompts:** Each verb customizes the system prompt, propose prompt, and evaluate prompt for its strategy
- **Engine reuse:** The core round loop (propose → evaluate → close) is shared; verbs differ in closing strategy and post-processing

## Open Questions

- Naming for the "mine" verb — alternatives: `distill`, `extract`, `survey`, `diverge`
- Whether `brainstorm` should use model-as-judge for semantic similarity or simpler statistical measures
- Whether `synthesize` needs its own closing strategy or just a post-processing step after `converge`

## Next Steps

→ `/workflows:plan` for the `converge` subcommand restructure
→ Create TODO files for `synthesize`, `brainstorm`, `mine`
