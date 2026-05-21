# Agent Handoff

Current state of the project and active work. Read this at session start. Update before compaction or at natural breakpoints.

**Last updated:** 2026-05-21

## Project State

5-crate Rust workspace: `refinery_core`, `refinery_cli`, `tundish_core`, `tundish_providers`, `tundish_cli`. GitHub: `Lightless-Labs/refinery`.

### Shipped Verbs

| Verb | PR | Status | Iteration | Selection |
|------|----|--------|-----------|-----------|
| `converge` | #24 | merged | own+reviews | vote-threshold |
| `synthesize` | #26 | merged | own+reviews (converge) → custom synthesis | highest score |
| `brainstorm` | #28 | merged | score-only | controversial (mean×stddev) |

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

- **019** — brainstorm smoke tests and field report (recommended next clean-session entrypoint)
- **017** — remaining brainstorm P3 nitpicks: JSON dry-run output, empty-provider validation, `ScoreHistory` named struct
- **013** — brainstorm strategy benchmarks (post-v0), now including prompt-reframing and Open Collider-style domain-collision baselines
- **018** — brainstorm divergence expansion: each model reframes the initial prompt, all models work all prompt variants; optional future domain collisions
- **011** — evolve verb (designed, not started)
- **010** — TTY-gate ANSI escape codes (affects all verbs)
- **016** — JSON serialization consistency follow-up; partially addressed in PR #28, verify remaining synthesize paths before closing

## CodeRabbit Review Process

**Critical:** CodeRabbit nests 5-8 nitpick findings inside `<details><summary>🧹 Nitpick comments</summary>` sections in review body text. These are NOT inline comments. Always read full review bodies — truncating misses them. See `memory/feedback_read_coderabbit_bodies.md`.

Triage pattern: fix P1/P2 with code, create TODOs for P3/nitpicks, reply to every comment with rationale.

## Recent Context

- PR #28 / `feat/brainstorm-verb` merged the brainstorm verb: core loop in `refinery_core::brainstorm::run()`, scoring in `refinery_core::scoring`, prompts in `prompts/brainstorm.rs`, CLI in `commands/brainstorm.rs`.
- PR #29 merged post-merge documentation cleanup: brainstorm TODO 004 completed, wording TODOs 014/015 completed, handoff updated.
- 2026-05-21 local repo sync/cleanup completed: local `main` fast-forwarded to `origin/main` (`de1f051`), obsolete local brainstorm branches deleted, working tree clean before this handoff update branch.
- PR #28 verification before merge: `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` passed after CodeRabbit follow-up fixes.
- Brainstorm divergence discussion captured in `docs/plans/2026-03-31-001-feat-brainstorm-verb-plan.md` addendum and `todos/018-brainstorm-divergence-expansion.md`: v0 preserves divergence through score-only controversial selection; future work should inject divergence via prompt reframing (`n(n+1)` lineages) and optional Open Collider-style domain collisions (`n(1+p)d` lineages).
- `docs/solutions/` has solution docs covering Ctrl+C/SIGINT, provider quirks, prompt injection, tiebreaking, etc.

## Next Clean Session

Recommended order:

1. Start from clean `main` and read this handoff plus `todos/019-brainstorm-smoke-test-field-report.md`.
2. Run real `refinery brainstorm` smoke tests and capture a short field report before adding new strategies.
3. Use the field report to decide whether to do small polish first (`todos/017`) or create a dedicated implementation plan for prompt reframing (`todos/018`).
4. Do not implement Open Collider-style domain collisions before establishing the v0 brainstorm baseline and budget constraints.
