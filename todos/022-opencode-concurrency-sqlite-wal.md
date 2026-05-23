---
title: "fix: handle OpenCode concurrent subprocess SQLite/WAL failures"
priority: medium
milestone: v0.3
created: 2026-05-23
---

# Handle OpenCode Concurrent Subprocess SQLite/WAL Failures

## Problem

During the 2026-05-23 brainstorm smoke baseline, a parallel four-model run using three OpenCode-backed models degraded because several OpenCode subprocesses failed at startup with:

```text
Failed to run the query 'PRAGMA journal_mode = WAL'
```

The successful workaround was `--max-concurrent 1`, which serialized all provider calls. This is reliable but slow and underuses concurrency for non-OpenCode providers.

Report: `docs/brainstorms/2026-05-23-brainstorm-smoke-baseline.md`.

## Candidate Fixes

- Add provider-level concurrency limits, e.g. cap OpenCode subprocesses to one at a time while allowing Codex/other providers to run concurrently.
- Give each OpenCode subprocess an isolated config/state directory if OpenCode supports it safely.
- Detect this specific failure and recommend `--max-concurrent 1` in the error message until a proper provider-level limiter exists.

## Acceptance Criteria

- A model set with multiple OpenCode-backed models can run without WAL startup failures.
- Non-OpenCode providers are not unnecessarily serialized if provider-level limits are implemented.
- Degraded JSON/provider-failure output remains accurate when failures still occur.

## Origin

Observed while re-running the brainstorm smoke baseline with Codex, GLM, Kimi, and MiniMax.
