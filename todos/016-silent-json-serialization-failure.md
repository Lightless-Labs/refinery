---
title: "fix: handle JSON serialization failures in all verb output paths"
priority: low
milestone: v0.3
---

# Handle JSON Serialization Failures

## Problem

All three verbs (converge, synthesize, brainstorm) use `if let Ok(json) = serde_json::to_string_pretty(...)` for JSON output, silently swallowing serialization failures. If serialization fails, the user gets no output and no error indication.

## Fix

Replace `if let Ok` with `match` and log the serialization error + return a non-zero exit code on failure. Apply consistently across all verbs:

- `crates/refinery_cli/src/commands/converge.rs`
- `crates/refinery_cli/src/commands/synthesize.rs`
- `crates/refinery_cli/src/commands/brainstorm.rs`

## Origin

CodeRabbit CLI review finding (P2). Pre-existing pattern across all verbs, not brainstorm-specific.
