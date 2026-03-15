---
title: "OpenCode provider integration and multi-provider model dispatch"
category: integration-issues
tags: [opencode, provider, model-id, slash, jsonl, error-handling, streaming]
module: tundish_providers
symptom: "Need to support opencode CLI with nested sub-provider/model paths and JSONL output"
root_cause: "Each CLI tool has a different output format, error reporting, and model naming scheme"
date: 2026-03-15
---

# OpenCode provider integration and multi-provider model dispatch

## OpenCode CLI interface

```bash
opencode run --model provider/model --format json "prompt"
```

**Output:** JSONL with three event types:
- `{"type":"step_start", ...}` — session begins
- `{"type":"text", "part":{"text":"response"}}` — response text
- `{"type":"step_finish", ...}` — done, with token usage
- `{"type":"error", "error":{"data":{"message":"..."}}}` — error

**No system prompt flag.** Prepend system prompt to user message.

**No schema support.** The `_schema` parameter is ignored.

**Credentials:** Managed by opencode internally. Pass `HOME` env var so it can find its config.

## Model ID format

OpenCode models have nested paths: `opencode/minimax-m2.5-free`, `kimi-for-coding/kimi-k2-thinking`. In refinery, the provider is `opencode` and the model is the full opencode path:

```
refinery provider: opencode
refinery model:    kimi-for-coding/kimi-k2-thinking
opencode --model:  kimi-for-coding/kimi-k2-thinking
```

This required allowing slashes in `ModelId.model()`. The `parse()` method now splits on the *first* slash only:

```rust
// "opencode/kimi-for-coding/kimi-k2-thinking"
// provider = "opencode", model = "kimi-for-coding/kimi-k2-thinking"
let (provider, model) = s.split_once('/').unwrap();
```

## Error handling across providers

Each provider reports errors differently. `spawn_cli` now scans JSONL for structured error events before falling back to raw stderr:

| Provider | Error location | Format |
|---|---|---|
| Claude | JSONL `{"type":"assistant","error":"invalid_request"}` | Error in event stream |
| Claude | JSONL `{"type":"result","is_error":true}` | Error in result event |
| Codex | JSONL `{"type":"error","message":"{\"detail\":\"...\"}"}` | JSON-encoded detail |
| Gemini | stderr text | `ModelNotFoundError: ...` |
| OpenCode | JSONL `{"type":"error","error":{"data":{"message":"..."}}}` | Nested error object |

### Early stream abort

Claude with `--json-schema` and an invalid model hangs indefinitely (streaming hundreds of lines). The fix: `detect_fatal_stream_error()` checks each JSONL line as it's read and aborts immediately on error events. Invalid model now fails in ~1.5s instead of timing out after minutes.

### Permanently dropped models

Models with permanent failures (`ProcessFailed`, `BinaryNotFound`, `MissingCredential`) are excluded from future rounds via `ProviderError::is_permanent()`. Transient failures (timeout, idle timeout) are retried.

## Available models

Run `opencode models` to list all available models. Examples:

```
opencode/minimax-m2.5-free
opencode/nemotron-3-super-free
kimi-for-coding/kimi-k2-thinking
minimax-coding-plan/MiniMax-M2.5
zai-coding-plan/glm-4.7
zai-coding-plan/glm-5
```

## Cross-references

- `crates/tundish_providers/src/opencode.rs` — provider implementation
- `crates/tundish_providers/src/process.rs` — `detect_fatal_stream_error`, `extract_error_from_jsonl`
- `docs/solutions/integration-issues/cli-provider-subprocess-isolation.md` — subprocess isolation pattern
