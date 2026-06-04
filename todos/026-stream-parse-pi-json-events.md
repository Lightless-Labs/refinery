---
title: "fix: stream-parse Pi JSON events instead of buffering full stdout"
priority: medium
milestone: v0.4
status: completed
created: 2026-05-30
completed: 2026-06-04
---

# Stream-Parse Pi JSON Events

**Completed 2026-06-04:** Pi provider now uses a line-streaming subprocess path and stateful Pi JSON parser, retaining only assistant text/error state instead of full stdout. See `docs/plans/2026-06-04-002-fix-stream-parse-pi-json-events-plan.md` and `docs/brainstorms/2026-06-04-brainstorm-l3-prompt-reframing-smoke.md`.

## Problem

Pi `--mode json` can emit JSON event streams that are much larger than the final assistant text, because streamed update events may repeat accumulated content. During the 2026-05-30 L2 brainstorm benchmark, normal benchmark-sized answers exceeded the previous 1MB generic stdout cap and caused `ResponseTooLarge` failures.

As a tactical unblocker, `tundish_providers::process::MAX_RESPONSE_SIZE` was raised to 64MB. That keeps a bounded capture but is not ideal: it still buffers transport data that the Pi adapter only needs to parse into the latest assistant text.

## Goal

Teach the Pi provider path to parse JSONL incrementally and retain only the assistant text / relevant error state, avoiding large transport buffers while keeping existing timeout, process cleanup, and error behavior.

## Notes

- Keep generic provider stdout capture bounded for other CLIs.
- Preserve fatal stream-error detection.
- Preserve `extract_pi_response()` unit coverage, or split a streaming parser into a separately testable helper.
- Verify with a real Pi-backed brainstorm smoke run that previously exceeded 1MB.

## References

- `crates/tundish_providers/src/pi.rs`
- `crates/tundish_providers/src/process.rs`
- `docs/brainstorms/2026-05-30-brainstorm-l2-iteration-strategy-benchmark.md`
