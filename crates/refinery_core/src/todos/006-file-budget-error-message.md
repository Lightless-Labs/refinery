---
title: "fix: misleading file budget error message"
priority: low
milestone: v0.3
---

# Fix misleading file budget error message

## Problem

When a prompt is provided alongside files, the file budget is reduced by the prompt size. But the error message still says "exceeds 1MB limit" even when the actual remaining budget is less than 1MB.

## Solution

Change the error to show the actual remaining budget:

```rust
errors.push(format!(
    "file '{path_str}': size ({file_size} bytes) exceeds remaining budget ({budget} bytes)"
));
```

## References

- `crates/refinery_cli/src/commands/common.rs:305-309`
- Found by CodeRabbit review on PR for synthesize verb
