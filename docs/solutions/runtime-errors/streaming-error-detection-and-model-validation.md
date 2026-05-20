---
title: "Streaming error detection and early abort for invalid models"
category: runtime-errors
tags: [streaming, jsonl, error-detection, invalid-model, claude, codex, opencode, early-abort, is_error]
module: tundish_providers
symptom: "Invalid model causes minutes of streaming output before failing, or error message treated as valid response"
root_cause: "spawn_cli reads all stdout before extraction; claude returns is_error in result event which was treated as valid"
date: 2026-03-15
---

# Streaming error detection and early abort for invalid models

## Problem 1: Claude is_error treated as valid response

Claude returns `{"type":"result","is_error":true,"result":"There's an issue with the selected model..."}` for invalid models. The `extract_from_result_event` function saw `type: "result"` with a non-empty `result` field and returned it as a valid proposal. The error message was then used as the model's "answer" and evaluated by other models.

### Fix

Check `is_error` before extracting the result:

```rust
fn extract_from_result_event(event: &Value) -> Result<Option<String>, String> {
    if event.get("is_error").and_then(Value::as_bool) == Some(true) {
        let msg = event.get("result").and_then(|r| r.as_str()).unwrap_or("unknown error");
        return Err(msg.to_string());
    }
    // ... normal extraction
}
```

## Problem 2: Claude with --json-schema hangs on invalid model

Without `--json-schema`, Claude returns an error instantly (3 lines, 1.5s). With `--json-schema`, it hangs indefinitely streaming hundreds of lines. The error event appears on line 2 (`{"type":"assistant","error":"invalid_request"}`), but `spawn_cli` reads all stdout before `extract_claude_response` runs.

### Fix

Scan each JSONL line during streaming and abort on fatal errors:

```rust
// In spawn_cli's read loop:
if let Some(msg) = detect_fatal_stream_error(line_buf.trim()) {
    return Err(ProviderError::ProcessFailed {
        model: model_clone.clone(),
        message: msg,
        exit_code: None,
    });
}
```

`detect_fatal_stream_error` checks for:
- Claude: `{"type":"assistant","error":"invalid_request"}`
- Claude: `{"type":"result","is_error":true}`
- Codex: `{"type":"error"}` or `{"type":"turn.failed"}`
- OpenCode: `{"type":"error","error":{"data":{"message":"..."}}}`

Result: invalid model fails in ~1.5s instead of timing out after minutes.

## Problem 3: Messy error messages from stderr dumps

When a process exits non-zero, `spawn_cli` used to pass the entire stderr (or stdout) as the error message — including stack traces, credential warnings, and JSONL dumps.

### Fix

`extract_error_from_jsonl` scans for structured error events first. Falls back to first non-noise line (skipping "Loaded cached credentials", "Warning:", stack frames starting with "at "):

| Provider | Before | After |
|---|---|---|
| Codex | Full JSONL dump | `The 'haha' model is not supported` |
| Gemini | Full stack trace | `ModelNotFoundError: Requested entity was not found.` |
| OpenCode | `no text events found...` | `Model not found: opencode/fake-model.` |

## Cross-references

- `crates/tundish_providers/src/process.rs` — `detect_fatal_stream_error`, `extract_error_from_jsonl`
- `crates/tundish_providers/src/opencode.rs` — OpenCode error event parsing
