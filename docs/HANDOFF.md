# Agent Handoff

Current state of the project and active work. Read this at session start. Update before compaction or at natural breakpoints.

**Last updated:** 2026-05-22

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

- **013** — brainstorm strategy benchmarks (in progress): phase-1 benchmark design completed; next step is artifact analyzer for selector counterfactuals + panel metrics
- **018** — brainstorm divergence expansion: each model reframes the initial prompt, all models work all prompt variants; optional future domain collisions
- **021** — evaluate TOON (`toon-format/toon`) for prompt-facing artifact export / benchmark fixtures
- **011** — evolve verb (designed, not started)

## CodeRabbit Review Process

**Critical:** CodeRabbit nests 5-8 nitpick findings inside `<details><summary>🧹 Nitpick comments</summary>` sections in review body text. These are NOT inline comments. Always read full review bodies — truncating misses them. See `memory/feedback_read_coderabbit_bodies.md`.

Triage pattern: fix P1/P2 with code, create TODOs for P3/nitpicks, reply to every comment with rationale.

## Recent Context

- PR #28 / `feat/brainstorm-verb` merged the brainstorm verb: core loop in `refinery_core::brainstorm::run()`, scoring in `refinery_core::scoring`, prompts in `prompts/brainstorm.rs`, CLI in `commands/brainstorm.rs`.
- PR #29 merged post-merge documentation cleanup: brainstorm TODO 004 completed, wording TODOs 014/015 completed, handoff updated.
- 2026-05-21 brainstorm smoke test field report completed (`todos/019`, `docs/brainstorms/2026-05-21-brainstorm-smoke-test-field-report.md`). Result: not a valid multi-model baseline because only `codex-cli/gpt-5.4` produced usable responses; Claude failed with 403/no access and Gemini hit capacity/quota. Created `todos/020-brainstorm-provider-failure-observability.md` because partial provider failures can look like successful single-provider brainstorms with zero/no eval scores.
- 2026-05-21 TOON research note added (`docs/brainstorms/2026-05-21-toon-format-for-refinery-artifacts.md`, `todos/021`). Recommendation: keep canonical JSON/JSONL, but evaluate TOON as a token-efficient prompt-export/benchmark fixture format for uniform artifact records. If using Rust crate, prefer `toon-format = { version = "0.4", default-features = false }` and verify spec alignment.
- 2026-05-21 brainstorm provider failure observability completed (`todos/020`, `docs/plans/2026-05-21-001-fix-brainstorm-provider-failure-observability-plan.md`): core tracks proposal/evaluation failures, CLI JSON/text exposes degraded runs, unevaluated scores serialize as null, and artifact output includes `provider-failures.json`. Verified with `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and a manual degraded `claude-code,codex-cli` smoke.
- PR #31 merged 2026-05-22: `fix: surface degraded brainstorm runs`.
- PR #32 merged 2026-05-22: `chore: address brainstorm P3 nitpicks` (`todos/017` completed). Adds JSON dry-run output for converge/synthesize/brainstorm, explicit empty-provider validation in `brainstorm::run()`, and `ScoreHistoryEntry { proposal, mean_score }`.
- 2026-05-22 JSON serialization follow-up completed (`todos/016`, `docs/plans/2026-05-22-001-fix-json-serialization-failures-plan.md`): remaining silent `serde_json::to_string_pretty` handling in synthesize output paths now logs serialization errors and returns non-zero where appropriate. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_cli`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, and an `rg` check for the old silent pattern.
- 2026-05-22 TTY ANSI follow-up completed (`todos/010`, `docs/plans/2026-05-22-002-fix-tty-gate-ansi-escape-codes-plan.md`): direct synthesize progress logs now colorize only when stderr is a TTY; non-TTY logs keep plain symbols with no raw ANSI color escapes. Added helper tests and verified with `cargo fmt --all -- --check`, `cargo test -p refinery_cli`, and `cargo clippy -p refinery_cli --all-targets -- -D warnings`.
- 2026-05-23 valid brainstorm smoke baseline completed with Codex + GLM + Kimi + MiniMax (`docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md`). Successful runs used `--max-concurrent 1`; both product and technical prompts completed non-degraded with 32 calls, peer evaluations, and controversial panel selection. `todos/013` and `todos/018` are now unblocked for benchmark/prompt-reframing planning. Created `todos/022-opencode-concurrency-sqlite-wal.md` after parallel OpenCode-backed runs hit `PRAGMA journal_mode = WAL` startup failures.
- 2026-05-23 brainstorm strategy benchmark design started for `todos/013` (`docs/plans/2026-05-23-001-research-brainstorm-strategy-benchmarks-plan.md`, `docs/brainstorms/2026-05-23-brainstorm-strategy-benchmark-design.md`). Defined L0/L1/L2/L3 benchmark protocol, prompt suite, metrics, budget model, and first offline selector counterfactuals on the valid baseline. Recommendation: implement an artifact analyzer before adding new strategy variants.
- 2026-05-21 local repo sync/cleanup completed: local `main` fast-forwarded to `origin/main` (`de1f051`), obsolete local brainstorm branches deleted, working tree clean before this handoff update branch.
- PR #28 verification before merge: `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` passed after CodeRabbit follow-up fixes.
- Brainstorm divergence discussion captured in `docs/plans/2026-03-31-001-feat-brainstorm-verb-plan.md` addendum and `todos/018-brainstorm-divergence-expansion.md`: v0 preserves divergence through score-only controversial selection; future work should inject divergence via prompt reframing (`n(n+1)` lineages) and optional Open Collider-style domain collisions (`n(1+p)d` lineages).
- `docs/solutions/` has solution docs covering Ctrl+C/SIGINT, provider quirks, prompt injection, tiebreaking, etc.

## Next Clean Session

Recommended order:

1. Start from clean `main` and read this handoff plus the valid baseline in `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md`.
2. If continuing `todos/013`, implement the artifact analyzer described in `docs/brainstorms/2026-05-23-brainstorm-strategy-benchmark-design.md` before adding new strategy variants.
3. Consider addressing `todos/022` before running more OpenCode-heavy multi-model panels; for now use `--max-concurrent 1` with multiple OpenCode-backed models.
4. Do not implement Open Collider-style domain collisions before benchmark budget constraints are explicit.
