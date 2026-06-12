# Agent Handoff

Current state of the project and active work. Read this at session start. Update before compaction or at natural breakpoints.

**Last updated:** 2026-06-11

## Project State

5-crate Rust workspace: `refinery_core`, `refinery_cli`, `tundish_core`, `tundish_providers`, `tundish_cli`. GitHub: `Lightless-Labs/refinery`.

### Shipped Verbs

| Verb | PR | Status | Iteration | Selection |
|------|----|--------|-----------|-----------|
| `converge` | #24 | merged | own+reviews | vote-threshold |
| `synthesize` | #26 | merged | own+reviews (converge) → custom synthesis | highest score |
| `brainstorm` | #28, #37 | merged | score-only | controversial with quality floor |

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
- **Pi JSON event streams can be much larger than final text.** `PiProvider` now uses a streaming stdout path and stateful JSONL parser so it retains assistant text/error state instead of buffering the full event stream; generic non-Pi CLI capture remains bounded.

## Open TODOs

Check `todos/` for the full list. Key ones:

- **013** — brainstorm strategy benchmarks (in progress): design, analyzer, six-prompt v0 suite, quality-floor follow-up, meta-preamble prompt polish, benchmark-only iteration variants, L2 six-prompt variant suite, blind review pack, first-pass qualitative L2 panel review, hidden L3 prompt-reframing implementation, 3-model L3 smoke, updated-model 2-model L3 smoke, two-prompt 3-model L3 sample, and initial verified parser hardening for recoverable GLM-style invalid eval scores completed; next step is either live-validate the parser hardening with 2-4 more L3 prompts using Codex/GLM/Kimi-for-coding or continue deeper GLM triage if invalid eval scores persist; avoid full MiniMax M3 suites until runtime/output budget controls are explicit
- **025** — optional brainstorm lineage-reference polish if softer phrases like "builds on..." feel too process-oriented in demos
- **018** — brainstorm divergence expansion: first-stage prompt reframing implemented behind hidden `brainstorm --prompt-variants per-model`; next run L3 benchmarks and defer domain collisions
- **021** — evaluate TOON (`toon-format/toon`) for prompt-facing artifact export / benchmark fixtures
- **011** — evolve verb (designed, not started)

## CodeRabbit Review Process

**Critical:** CodeRabbit nests 5-8 nitpick findings inside `<details><summary>🧹 Nitpick comments</summary>` sections in review body text. These are NOT inline comments. Always read full review bodies — truncating misses them. See `memory/feedback_read_coderabbit_bodies.md`.

Triage pattern: fix P1/P2 with code, create TODOs for P3/nitpicks, reply to every comment with rationale.

## Recent Context

- 2026-06-11 GLM invalid-evaluation parser hardening completed (`todos/013`, plan `docs/plans/2026-05-23-001-research-brainstorm-strategy-benchmarks-plan.md`). `parse_brainstorm_evaluation_response()` now accepts recoverable score variants seen/plausible in expanded brainstorm evals: scaled score text like `"8 out of 10"`, nested score objects, `overall_score`, and a missing-overall fallback to the four required dimension scores. It rejects incomplete dimension sets even if extra numeric fields are present. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_core parse_brainstorm_evaluation -q`, `cargo test -p refinery_core brainstorm -q`, and `cargo clippy -p refinery_core --all-targets -- -D warnings`. Next useful validation is a live Pi-backed L3 run with Codex/GLM/Kimi-for-coding; if invalid eval scores persist, capture/preserve raw invalid responses for deeper GLM triage.
- 2026-06-09 three-model L3 sample completed (`todos/013`, `docs/brainstorms/2026-06-09-brainstorm-l3-three-model-sample.md`) after PR #44 merged. Compared `--prompt-variants off` vs `per-model` on product and technical prompts using `pi/openai-codex/gpt-5.4:off`, `pi/zai/glm-5.1:off`, and `pi/kimi-coding/kimi-for-coding:off`, serial with `--max-concurrent 1`. Baseline runs completed clean (`18` calls each, ~7-8m). Per-model runs completed with full 12-candidate final sets but degraded evaluation status (`75` calls each, ~30-38m): product had one GLM invalid eval score; technical had one Codex SSE response-header timeout and one GLM invalid eval score. `controversy_floor_7` two-prompt averages improved mean quality `7.83 → 8.25`, min quality `7.00 → 8.00`, disagreement `0.33 → 0.75`; lexical overlap also rose `0.056 → 0.074`; meta-preamble stayed `0.0`. Promising but not enough for default changes because both expanded runs degraded.
- 2026-06-05 updated-model L3 smoke completed (`todos/013`, `docs/brainstorms/2026-06-05-brainstorm-l3-updated-model-smoke.md`) after Pi exposed `pi/kimi-coding/kimi-for-coding` (Kimi K2.6 for coding) and `pi/minimax/MiniMax-M3`. Single-model smoke calls for both worked. A two-model product baseline (`prompt-variants off`) completed clean with `total_calls: 8`, `degraded: false`, `controversy_floor_7` mean/min quality `7.50/7.00`, lexical overlap `0.073`. A two-model prompt-reframing run completed degraded with `total_calls: 25/26`, final candidates `5`, and one MiniMax M3 round-2 proposal timeout on the legal-scrutiny variant after 900s; `controversy_floor_7` mean/min quality `8.33/8.00`, lexical overlap `0.080`, meta-preamble `0.0`. Because two-model runs have only one evaluator per candidate, disagreement/controversy is not meaningful. A four-model updated sample (Codex + GLM + Kimi-for-coding + MiniMax M3) was stopped after ~14 minutes while still in the first baseline run; partial artifacts showed round-1 progress, so treat it as a budget/runtime caution rather than a correctness failure. Keep production defaults unchanged.
- 2026-06-04 Pi stream parsing completed (`todos/026`, plan `docs/plans/2026-06-04-002-fix-stream-parse-pi-json-events-plan.md`) after L3 prompt-reframing smoke re-exposed the 64MB Pi stdout cap. Added `process::spawn_cli_stream_lines()` with stderr draining, timeout/idle-timeout handling, and bounded error previews; `PiProvider` now feeds JSONL lines into a stateful parser shared with `extract_pi_response()`. The pre-fix 3-model L3 smoke degraded with Codex `ResponseTooLarge` (~64MB) and invalid GLM eval scores. The post-fix rerun completed `degraded: false`, `evaluation_status: peer_evaluated`, `total_calls: 75`, no provider failures, empty stderr. Smoke report: `docs/brainstorms/2026-06-04-brainstorm-l3-prompt-reframing-smoke.md`. Verified with `cargo fmt --all -- --check`, `cargo test -p tundish_providers pi -q`, streaming process test, `cargo clippy -p tundish_providers --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast`, and `cargo clippy --workspace --all-targets -- -D warnings`.
- 2026-06-04 prompt-reframing expansion implementation completed on branch `feat/brainstorm-prompt-reframing-l3` (`todos/018`, plan `docs/plans/2026-06-04-001-feat-brainstorm-prompt-reframing-expansion-plan.md`). Added hidden/internal `brainstorm --prompt-variants off|per-model`; `per-model` first asks each provider for one strategic prompt reframing, then runs every provider over the original anchor plus all generated variants. Expanded lineages use flat `ModelId`s like `provider/model+variant-1`, preserving the existing `round-N/propose-*.md` / `evaluate-*.json` benchmark artifact layout. Peer evaluation remains provider-owned, so a provider evaluates other providers' lineages but not its own. CLI text/JSON/dry-run output and `metadata.json` expose prompt-variant strategy/counts; `benchmark-brainstorm` surfaces prompt-variant metadata. Final verification observed clean: `cargo fmt --all -- --check`, `cargo check --workspace`, targeted `cargo test -p refinery_core brainstorm -q`, `cargo test -p refinery_core prompts -q`, `cargo test -p refinery_cli brainstorm -q`, full `cargo test --workspace`, targeted clippy for core/CLI, and `cargo clippy --workspace --all-targets -- -D warnings`. Manual dry-run and synthetic `benchmark-brainstorm` text/JSON smoke checks passed. Next step: run real Pi-backed L3 prompt-reframing benchmark using `score-only` baseline and `--max-concurrent 1`.
- 2026-06-03 PR #42 (`feat: add brainstorm panel review pack`) merged through the merge queue after follow-up fixes. Added `refinery review-brainstorm-panels`, recorded Pi-backed L2 benchmark results and first-pass panel review docs, and kept `score-only` as the recommended default. Review feedback addressed: bare `--key-path` filenames no longer call `create_dir_all("")`; `--strategies` and artifact metadata now parse through `BrainstormIterationStrategy`; saved `propose-*.md` text is preserved verbatim in review packs; handoff timestamp refreshed. Final checks observed before merge: GitHub Actions Build/Check/Test passed, Buildkite build #36 passed, and CodeRabbit passed. Local verification included `cargo fmt --all`, `cargo test -p refinery_cli review_brainstorm_panels`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, `cargo build --workspace`, and `cargo test --workspace`.
- 2026-06-01 first-pass brainstorm L2 panel review completed (`todos/013`, `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`): reviewed the blind pack at `target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/l2-panel-review-pack.md` and unblinded with `l2-panel-review-key.json`. Qualitative result: `score-only` looked strongest on useful diversity/non-overlap, `full-visibility` strongest on actionability/coverage, and `own-reviews` did not dominate globally but produced the strongest debugging/process panel. Recommendation remains: keep production default `score-only`; do not promote `full-visibility` despite higher automated quality scores until stronger human/calibrated judge evidence exists. For L3 prompt-reframing, use `score-only` as baseline and include `own-reviews` only if budget allows.
- 2026-05-30 brainstorm L2 iteration strategy benchmark completed (`todos/013`, `docs/brainstorms/2026-05-30-brainstorm-l2-iteration-strategy-benchmark.md`): ran 24 clean Pi-backed runs (6 prompts × `blind`, `score-only`, `own-reviews`, `full-visibility`) with `pi/openai-codex/gpt-5.4:off`, `pi/zai/glm-5.1:off`, `pi/kimi-coding/kimi-k2-thinking:off`, and `pi/minimax/MiniMax-M2.7:off`. Used `--max-concurrent 1` to avoid Pi local config lock contention and raised bounded stdout capture from 1MB to 64MB because Pi JSON event streams can exceed 1MB. Analyzer outputs live under `target/brainstorm-benchmark-2026-05-29-l2-pi-serial/logs/` (`run-dirs-clean.txt`, `l2-analysis-clean.json`, `l2-analysis-clean.txt`). Current `controversy_floor_7` aggregate: `full-visibility` highest quality (`mean=8.204`, `min=7.944`) but highest lexical overlap (`0.132`); `score-only` lowest lexical overlap (`0.097`) but lower quality (`mean=7.889`); `own-reviews` middle-ground (`mean=8.019`, disagreement `0.517`). Added `refinery review-brainstorm-panels` and generated blind review artifacts for `score-only`, `own-reviews`, and `full-visibility`: `l2-panel-review-pack.md` plus `l2-panel-review-key.json` in the same logs dir. Recommendation: keep production default `score-only` until whole-panel diversity/human or calibrated model-judge review checks semantic convergence and best-answer regret. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_cli review_brainstorm_panels`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, `cargo test -p tundish_providers`, and `cargo clippy -p tundish_providers --all-targets -- -D warnings`.
- 2026-05-31 PR #40 (`feat: add Pi provider and brainstorm benchmark variants`) passed final review/checks after follow-up commits. Addressed CodeRabbit/GHA feedback with nested `pi`/`opencode` model-spec validation, plan review-date refresh, and Clippy sort lint fixes; addressed Gemini feedback by comparing `ModelId` directly where compatible, accepting string evaluation scores, and preserving `USERPROFILE`; addressed Codex feedback by forwarding a whitelist of Pi credential/config env vars after `env_clear`. Final observed checks before merge: GitHub Actions Build/Check/Test passed, Buildkite build #31 passed, CodeRabbit approved. Local verification included `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`, and `cargo test --workspace`.
- 2026-05-26 Buildkite baked-image follow-up: PR #39 (`ci: use baked Linux ARM64 Buildkite image`) opened from fork branch `El-Fitz:chore/buildkite-baked-linux-image`. It switches `.buildkite/pipeline.yml` to Tart image `ci-linux-arm64-rust-bazel`, removes per-job apt/rustup bootstrap, keeps HOME/Cargo/Rustup normalization, and adds `/opt/cargo/bin` after review feedback. GitHub Actions checks passed. Buildkite did not appear as a PR check from the fork branch; the upstream Buildkite pipeline may still be using inline pipeline settings, so a maintainer should either update the Buildkite pipeline configuration to upload `.buildkite/pipeline.yml` from the repo or manually run/patch the Buildkite pipeline before merging.
- 2026-05-26 Pi provider adapter added: `tundish_providers::pi::PiProvider` supports model specs like `pi/openai/gpt-5.4`, invokes `pi --mode json --no-session --no-context-files --model <provider/model>`, disables tools by default, preserves Pi local config via `HOME`, and extracts assistant text from Pi JSON event streams. Default provider features are now `codex`, `opencode`, and `pi`; `claude`/`gemini` are no longer default options. `opencode` remains supported for users with local OpenCode config, but benchmarks should prefer Pi routing. Docs and `.env.example` updated accordingly. Verified with `cargo test -p tundish_providers`, `cargo clippy -p tundish_providers --all-targets -- -D warnings`, `cargo test -p refinery_cli`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, and a manual `brainstorm --dry-run` using `pi/openai/gpt-5.4` + `opencode/...`.
- 2026-05-26 brainstorm L2 benchmark-only iteration variants implemented (`todos/013`, `docs/plans/2026-05-23-001-research-brainstorm-strategy-benchmarks-plan.md`): added `BrainstormIterationStrategy` with hidden CLI flag `brainstorm --iteration-strategy {blind,score-only,own-reviews,full-visibility}`. Production default remains `score-only`. Core now builds strategy-specific proposer prompts, captures evaluation rationales for `own-reviews`/`full-visibility`, writes `metadata.json` with `iteration_strategy`, includes rationale in evaluation artifacts, exposes `iteration_strategy` in text/JSON/dry-run output, and `benchmark-brainstorm` reads run metadata for grouping. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_core brainstorm`, `cargo test -p refinery_cli`, `cargo clippy -p refinery_core --all-targets -- -D warnings`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, and a manual JSON dry-run using `--iteration-strategy blind`.
- PR #37 merged 2026-05-26 (`feat: add brainstorm quality floor and prompt polish`): includes brainstorm quality-floor selection (`todos/023`) and score-history meta-preamble prompt polish (`todos/024`). Quality-floor default is `--quality-floor 7.0`, `--quality-floor 0` preserves raw controversy, and brainstorm output exposes `selection_strategy`. Prompt polish validation is documented in `docs/brainstorms/2026-05-25-brainstorm-meta-preamble-prompt-polish.md`; product and technical reruns completed non-degraded with Codex + GLM + Kimi + MiniMax and analyzer reported `meta_preamble_rate: 0.0` for all selectors. Gemini and CodeRabbit review feedback was addressed by comparing `ModelId` directly, reusing public core quality-floor helpers from the CLI, validating core quality-floor config, adding core quality-floor tests, emitting JSON config errors for invalid brainstorm quality floors, handling NaN mean scores during quality-floor backfill, and updating docs/style. Local pi review with `openai-codex/gpt-5.5` at `xhigh` was run twice: first review found wording/validation/test/docs issues, all fixed; re-review reported no blocking or actionable issues. GitHub Actions, Buildkite, and CodeRabbit passed; PR was merged through the queue. Verification run across the branch: `cargo fmt --all -- --check`, `cargo test -p refinery_core scoring`, `cargo test -p refinery_core brainstorm`, `cargo test -p refinery_core prompts`, `cargo test -p refinery_cli`, `cargo clippy -p refinery_core --all-targets -- -D warnings`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, plus manual brainstorm dry-run/config-error checks.
- 2026-05-25 brainstorm score-history meta-preamble prompt polish completed (`todos/024`, `docs/plans/2026-05-25-001-fix-brainstorm-score-history-meta-preambles-plan.md`, `docs/brainstorms/2026-05-25-brainstorm-meta-preamble-prompt-polish.md`): `brainstorm_system_prompt()` and `propose_with_score_history_prompt()` now tell models to use scores internally and return standalone user-facing answers without mentioning scores, prior rounds, feedback, benchmarks, or selection mechanics. Added prompt tests verifying the instruction and that score history is still present. Reran product and technical benchmark prompts with Codex + GLM + Kimi + MiniMax; both completed non-degraded and analyzer reported `meta_preamble_rate: 0.0` for all selectors, improved from the prior 0.333 baseline. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_core prompts`, and `cargo clippy -p refinery_core --all-targets -- -D warnings`.
- 2026-05-25 Buildkite migration completed: PR #35 (`ci: add Buildkite Linux ARM64 pipeline`) merged. The pipeline uses `github.com/Bande-a-Bonnot/tart-ci#v0.1.1` on queue `ci-linux-arm64`; Buildkite pipeline `la-bande-a-bonnot/refinery` is active. Build #6 passed on the persistent `big-cabbage` Tart runner after builds #1-#5 exposed Buildkite shell interpolation and Rust home-dir issues; fixed by normalizing `HOME`, forcing `CARGO_HOME`/`RUSTUP_HOME`, and escaping shell variables as `$$` in Buildkite YAML. Review feedback then flagged non-deterministic `ubuntu:latest` and redundant Cargo commands; fixed by pinning the Tart Ubuntu image to digest `sha256:e90dfc9e6dffb742809f32e61ee03daf5fa6ee30e24ee05c105beffa3b7c9540` and dropping `cargo check` / duplicate clippy `-D warnings`. Later tag-fetch fixes were merged (`7ba25cc`, `514db51`). Latest observed `main` status on 2026-05-26: Buildkite `buildkite/refinery` build #20 passed. Baked CI image work is explicitly out of scope for this agent lane.
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
- 2026-05-23 brainstorm strategy benchmark design started for `todos/013` (`docs/plans/2026-05-23-001-research-brainstorm-strategy-benchmarks-plan.md`, `docs/brainstorms/2026-05-23-brainstorm-strategy-benchmark-design.md`). Defined L0/L1/L2/L3 benchmark protocol, prompt suite, metrics, budget model, and first offline selector counterfactuals on the valid baseline. Implemented `refinery benchmark-brainstorm`, which loads brainstorm run dirs and emits selector counterfactuals (`mean`, `stddev`, `controversy`, `controversy_floor_7`, `quality_x_lexdiv`) plus panel metrics. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_cli`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, and manual analyzer runs.
- 2026-05-23 six-prompt v0 benchmark suite completed (`docs/brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md`). Aggregate: raw controversy lowered lexical overlap (0.130 → 0.100) but reduced min quality (7.778 → 6.833); `controversy_floor_7` improved min quality to 7.500 while preserving some diversity. Created `todos/023` for quality-floor selection and `todos/024` for suppressing score-history meta-preambles. Research prompt initially degraded at idle timeout 180; retry with idle timeout 480 succeeded.
- 2026-05-24 brainstorm quality-floor selection completed (`todos/023`, `docs/plans/2026-05-24-001-feat-brainstorm-quality-floor-selection-plan.md`): core now supports raw controversy plus quality-floor panel selection, CLI defaults to `--quality-floor 7.0` and accepts `--quality-floor 0` for raw controversy, and text/JSON/dry-run output expose `selection_strategy`. Verified with `cargo fmt --all -- --check`, `cargo test -p refinery_core scoring`, `cargo test -p refinery_cli`, `cargo clippy -p refinery_core --all-targets -- -D warnings`, `cargo clippy -p refinery_cli --all-targets -- -D warnings`, and manual brainstorm dry-run checks.
- 2026-05-23 verbs/commands README added at `crates/refinery_cli/src/commands/README.md` and linked from root `README.md`. It documents shipped verbs, the three-axis verb model, brainstorm benchmark results, benchmark analyzer usage, and OpenCode benchmark caveats.
- 2026-05-21 local repo sync/cleanup completed: local `main` fast-forwarded to `origin/main` (`de1f051`), obsolete local brainstorm branches deleted, working tree clean before this handoff update branch.
- PR #28 verification before merge: `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` passed after CodeRabbit follow-up fixes.
- Brainstorm divergence discussion captured in `docs/plans/2026-03-31-001-feat-brainstorm-verb-plan.md` addendum and `todos/018-brainstorm-divergence-expansion.md`: v0 preserves divergence through score-only controversial selection; future work should inject divergence via prompt reframing (`n(n+1)` lineages) and optional Open Collider-style domain collisions (`n(1+p)d` lineages).
- `docs/solutions/` has solution docs covering Ctrl+C/SIGINT, provider quirks, prompt injection, tiebreaking, etc.

## Next L3 Validation Runbook

Purpose: live-validate commit `dc805a7` (`fix: harden brainstorm evaluation score parsing`) against the GLM invalid-evaluation failures observed in the 2026-06-09 expanded L3 runs.

Suggested artifact root:

```text
target/brainstorm-benchmark-2026-06-11-l3-parser-validation/
```

For each selected prompt, run a paired baseline and expanded prompt-reframing run with the same three-model panel:

```sh
ROOT=target/brainstorm-benchmark-2026-06-11-l3-parser-validation
PROMPT='<one L3 benchmark prompt>'
mkdir -p "$ROOT/logs"

cargo run -q -p refinery_cli -- brainstorm "$PROMPT" \
  --models pi/openai-codex/gpt-5.4:off,pi/zai/glm-5.1:off,pi/kimi-coding/kimi-for-coding:off \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --iteration-strategy score-only \
  --prompt-variants off \
  --output-dir "$ROOT/off/<prompt-slug>" \
  --output-format json \
  --verbose \
  --idle-timeout 480 \
  --timeout 1800 \
  --max-concurrent 1 > "$ROOT/logs/<prompt-slug>-off.json" 2> "$ROOT/logs/<prompt-slug>-off.stderr.log"

cargo run -q -p refinery_cli -- brainstorm "$PROMPT" \
  --models pi/openai-codex/gpt-5.4:off,pi/zai/glm-5.1:off,pi/kimi-coding/kimi-for-coding:off \
  --max-rounds 2 \
  --panel-size 3 \
  --quality-floor 7.0 \
  --iteration-strategy score-only \
  --prompt-variants per-model \
  --output-dir "$ROOT/per-model/<prompt-slug>" \
  --output-format json \
  --verbose \
  --idle-timeout 480 \
  --timeout 1800 \
  --max-concurrent 1 > "$ROOT/logs/<prompt-slug>-per-model.json" 2> "$ROOT/logs/<prompt-slug>-per-model.stderr.log"
```

Record each generated run directory in `$ROOT/logs/run-dirs.txt`, then analyze with:

```sh
cargo run -q -p refinery_cli -- benchmark-brainstorm $(cat "$ROOT/logs/run-dirs.txt") --output-format text > "$ROOT/logs/l3-parser-validation-analysis.txt"
cargo run -q -p refinery_cli -- benchmark-brainstorm $(cat "$ROOT/logs/run-dirs.txt") --output-format json > "$ROOT/logs/l3-parser-validation-analysis.json"
```

Decision check: inspect every `provider-failures.json`. If GLM still reports `provider returned an invalid brainstorm evaluation score`, preserve/capture the raw invalid evaluation response in the next code pass before further parser guessing. If expanded runs are non-degraded, continue 2-4 total prompts and compare against `docs/brainstorms/2026-06-09-brainstorm-l3-three-model-sample.md`.

## Next Clean Session

Recommended order:

1. If continuing Buildkite migration, review PR #39 and either trigger a real Buildkite run against `ci-linux-arm64-rust-bazel` or update the Buildkite pipeline settings to upload `.buildkite/pipeline.yml` from the repo so PR pipeline changes are exercised.
2. Start from clean `main` and read this handoff plus the valid baseline in `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md`.
3. If continuing brainstorm strategy work, read `docs/brainstorms/2026-06-01-brainstorm-l2-panel-review.md`, `docs/brainstorms/2026-06-04-brainstorm-l3-prompt-reframing-smoke.md`, `docs/brainstorms/2026-06-05-brainstorm-l3-updated-model-smoke.md`, and `docs/brainstorms/2026-06-09-brainstorm-l3-three-model-sample.md`; then either run 2-4 more L3 prompt-reframing prompts with the Codex/GLM/Kimi-for-coding panel to live-validate the 2026-06-11 evaluation parser hardening, or continue deeper GLM invalid-score triage if failures persist. Do not launch a full MiniMax M3-heavy suite without explicit runtime/output budget controls.
4. For future Pi-backed benchmark runs, use `--max-concurrent 1` unless Pi config locking is fixed; for OpenCode-backed models use `--max-concurrent 1` and `--idle-timeout 480` until `todos/022` is fixed.
5. Do not implement Open Collider-style domain collisions before benchmark budget constraints are explicit; if moving to L3, start with prompt-reframing expansion from `todos/018`.
