# Agent Handoff

Current state of the project and active work. Read this at session start. Update before compaction or at natural breakpoints.

**Last updated:** 2026-04-04

## Project State

5-crate Rust workspace: `refinery_core`, `refinery_cli`, `tundish_core`, `tundish_providers`, `tundish_cli`. GitHub: `Lightless-Labs/refinery`.

### Shipped Verbs

| Verb | PR | Status | Iteration | Selection |
|------|----|--------|-----------|-----------|
| `converge` | #24 | merged | own+reviews | vote-threshold |
| `synthesize` | #26 | merged | own+reviews (converge) → custom synthesis | highest score |
| `brainstorm` | #27 | merged/queued | score-only | controversial (mean×stddev) |

### Planned Verbs

| Verb | TODO | Key idea |
|------|------|----------|
| `evolve` | 011 | Darwinian: blind variation + score-only pressure, cull below N σ, restart culled models |
| adversarial | moved to `~/Projects/lightless-labs/lightless-labs/crucible/` | Red/green team with information walls, separate project |

### Three-Axis Verb Model

Every verb combines choices on three orthogonal axes:
1. **Iteration strategy** — what models see between rounds (own+reviews, score-only, full visibility, blind, etc.)
2. **Evaluation strategy** — the rubric (accuracy vs originality vs integration, etc.)
3. **Selection strategy** — what "good" means given scores (consensus, controversial, cull-below-σ, etc.)

See `memory/verb_architecture.md` for full taxonomy with consistent terminology.

## Architecture Gotchas

- **`phases::propose::run()` and `phases::evaluate::run()` are hardcoded** to converge's prompts/schemas. New verbs must build own JoinSet loops calling `provider.send_message()` directly. See synthesize and brainstorm for the pattern.
- **`EchoProvider` FIFO queue + HashMap eval order** = non-deterministic test results. Use uniform scores per evaluator. See `docs/solutions/logic-errors/echo-provider-queue-ordering-in-multi-model-tests.md`.
- **New XML tags in prompts need sanitizers.** See `docs/solutions/security-issues/prompt-injection-prevention-multi-model.md`.
- **Rationale before score** in all eval schemas — autoregressive anti-manipulation measure.
- **Float score coercion** — `as_u64().or_else(as_f64())` pattern for score parsing.

## Open TODOs

Check `todos/` for the full list. Key ones:

- **004** — brainstorm verb (shipped, TODO has strategy taxonomy for future benchmarks)
- **011** — evolve verb (designed, not started)
- **013** — brainstorm strategy benchmarks (post-v0)
- **014-017** — P2/P3 nitpicks from brainstorm reviews (wording, JSON serialization, etc.)
- **010** — TTY-gate ANSI escape codes (affects all verbs)
- **016** — silent JSON serialization failure (affects all verbs)

## CodeRabbit Review Process

**Critical:** CodeRabbit nests 5-8 nitpick findings inside `<details><summary>🧹 Nitpick comments</summary>` sections in review body text. These are NOT inline comments. Always read full review bodies — truncating misses them. See `memory/feedback_read_coderabbit_bodies.md`.

Triage pattern: fix P1/P2 with code, create TODOs for P3/nitpicks, reply to every comment with rationale.

## Recent Context

- PR #27 (brainstorm) includes: core loop in `refinery_core::brainstorm::run()`, scoring in `refinery_core::scoring`, prompts in `prompts/brainstorm.rs`, CLI in `commands/brainstorm.rs`
- 132 tests across workspace
- `docs/solutions/` has 15 solution docs covering Ctrl+C/SIGINT, provider quirks, prompt injection, tiebreaking, etc.
