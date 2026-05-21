---
title: "fix: brainstorm provider failure observability"
type: fix
status: active
date: 2026-05-21
todo: 020-brainstorm-provider-failure-observability
---

# Brainstorm Provider Failure Observability Plan

## Problem

`refinery brainstorm` can request multiple providers, have one provider produce usable proposals, and still emit a clean-looking successful result with zero/no evaluation scores. This makes smoke tests and strategy benchmarks unreliable.

## Goals

- Track provider failures in brainstorm proposal and evaluation phases.
- Surface failures in text and JSON output.
- Make degraded runs machine-detectable.
- Distinguish "not evaluated" from numeric score `0.0` in CLI output.
- Preserve current ability to return a best-effort answer when at least one model succeeds.

## Non-Goals

- Do not add provider-level schema/tool-call enforcement.
- Do not replace canonical artifact JSON/JSONL formats.
- Do not change provider subprocess behavior.
- Do not make TOON part of this implementation.

## Design

1. Add core result metadata:
   - `BrainstormProviderFailure` with round, phase, model, optional target model, and message.
   - `BrainstormEvaluationStatus` to summarize whether peer evaluation happened, was skipped, or was partial.
   - `BrainstormResult::provider_failures`.
   - `BrainstormResult::degraded`.
   - helper for unique failed model ids.

2. Track failures:
   - Proposal provider errors and timeouts become provider failures.
   - Empty proposal text is treated as an invalid proposal failure.
   - Evaluation provider errors, timeouts, and invalid/missing score JSON become provider failures.

3. CLI output:
   - JSON adds `degraded`, `evaluation_status`, and `provider_failures`.
   - `metadata.models_dropped` is populated with failed model ids for compatibility with existing metadata shape.
   - Panel score fields become `null` when a candidate has no evaluator scores.
   - Text output prints a degraded-run warning and provider failure summary.

4. Error output:
   - Brainstorm errors carry tracked provider failures where available.
   - JSON/text errors include those failures for troubleshooting.

## Verification

- Add focused core tests for partial proposal failure and evaluation failure reporting.
- Run `cargo fmt --all -- --check`.
- Run `cargo test -p refinery_core brainstorm`.
- Run `cargo test --workspace` if the focused tests pass.
