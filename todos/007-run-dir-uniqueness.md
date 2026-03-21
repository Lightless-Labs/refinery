---
title: "fix: make run directories unique beyond one-second granularity"
priority: low
milestone: v0.3
---

# Make run directories unique beyond one-second granularity

## Problem

Two runs started in the same second with the same prompt produce identical directory names (`YYYYMMDD-HHMMSS_slug`), causing file overwrites. File-only runs (no prompt) in the same second also collide.

## Solution

Add a random suffix to the directory name:

```rust
let random: u32 = rand::random::<u32>() & 0xFFFF;
base.join(format!("{timestamp}_{slug}_{random:04x}"))
```

## References

- `crates/refinery_cli/src/commands/common.rs` — `make_run_dir()`
- Found by CodeRabbit review on PR #26
