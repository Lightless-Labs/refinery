---
date: 2026-05-25
topic: brainstorm-score-history-meta-preambles
todo: 024-brainstorm-suppress-score-history-meta-preambles
status: completed
completed: 2026-05-25
---

# Plan: Suppress Brainstorm Score-History Meta-Preambles

**Completed:** 2026-05-25

**Validation:** `docs/brainstorms/2026-05-25-brainstorm-meta-preamble-prompt-polish.md`

## Context

The six-prompt brainstorm benchmark found frequent final-answer preambles such as "Based on my Round 1 score..." and "My previous proposal got...". The score-only iteration signal is useful, but the selected panel should read as direct answers to the user's prompt rather than process artifacts.

## Goals

- Keep score-only iteration intact: models still see their own prior proposals and aggregate scores.
- Add prompt instructions telling models to use score history internally without mentioning scores, previous rounds, feedback, benchmark mechanics, or selection mechanics in the final answer.
- Add prompt tests that lock in the no-meta-commentary instruction and verify score history is still present.

## Non-Goals

- Do not add a cleanup/rewrite provider pass yet.
- Do not expose evaluator rationales or peer content to brainstorm proposers.
- Do not run a broad provider benchmark unless credentials/capacity are available and the run budget is explicitly accepted.

## Implementation Steps

1. Update `brainstorm_system_prompt()` with a general instruction that final answers should be standalone and omit process/meta commentary.
2. Update `propose_with_score_history_prompt()` for round 2+ to explicitly forbid score-history, previous-round, feedback, benchmark, and selection-mechanics mentions in the returned answer.
3. Add prompt tests covering:
   - the forbidden meta-commentary instruction appears in score-history prompts;
   - score history tags and scores remain present;
   - the brainstorm system prompt carries the standalone-answer instruction.
4. Run prompt/core tests plus formatting and clippy for touched crates.

## Verification

```sh
cargo fmt --all -- --check
cargo test -p refinery_core prompts
cargo clippy -p refinery_core --all-targets -- -D warnings
```

If provider budget is approved, rerun two brainstorm benchmark prompts and compare `meta_preamble_rate` with `refinery benchmark-brainstorm`.
