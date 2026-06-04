---
title: "fix: stream-parse Pi JSON events"
type: fix
status: completed
date: 2026-06-04
todo: 026-stream-parse-pi-json-events
---

# Stream-Parse Pi JSON Events Plan

**Enhanced:** 2026-06-04 (L3 prompt-reframing smoke exposed 64MB cap again)
**Completed:** 2026-06-04

## Context

Prompt-reframing L3 smoke runs can multiply Pi calls and prompt sizes. A 3-model product smoke produced `ResponseTooLarge` for Codex proposals around the current 64MB bounded stdout cap, even though the final assistant answer should be much smaller. This is the exact issue described in `todos/026`: Pi JSON event streams can repeat accumulated content and should be parsed incrementally instead of buffered wholesale.

## Goals

- Keep the generic CLI stdout cap for non-Pi providers.
- Add a streaming stdout path that lets Pi retain only assistant text/error state.
- Preserve existing process isolation, timeout, idle timeout, progress callbacks, stderr draining, non-zero exit handling, and fatal event detection.
- Keep `extract_pi_response()` unit coverage by sharing parser logic with the streaming path.

## Non-Goals

- Do not change non-Pi provider behavior.
- Do not remove bounded error previews for failed subprocesses.
- Do not run a full L3 benchmark in this fix; rerun the previously degraded smoke as validation.

## Implementation

1. Add a `process::spawn_cli_stream_lines` helper that mirrors `spawn_cli` but invokes a line handler instead of retaining full stdout.
2. Refactor Pi JSON extraction into a small stateful parser with `observe_line()` and `finish()`.
3. Make `PiProvider::send_message()` call the streaming helper and return the parser result.
4. Keep `extract_pi_response(jsonl, model_id)` by feeding lines into the same parser for tests/backward compatibility.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p tundish_providers pi -q`
- `cargo test -p tundish_providers process::tests::spawn_cli_stream_lines_collects_stdout_and_drains_stderr -- --nocapture`
- `cargo clippy -p tundish_providers --all-targets -- -D warnings`
- `cargo test --workspace --no-fail-fast`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Reran the 3-model prompt-reframing product smoke that previously hit `ResponseTooLarge`; the post-fix run completed `degraded: false`, `evaluation_status: peer_evaluated`, `total_calls: 75`, no provider failures, and empty stderr.
