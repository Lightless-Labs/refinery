---
title: "fix: report end-to-end elapsed time for synthesize runs"
priority: low
milestone: v0.3
---

# Report end-to-end elapsed time for synthesize

## Problem

Synthesize output uses `outcome.elapsed` which only covers the converge phase. The synthesis propose + evaluate time is not included in the reported elapsed time.

## Solution

Track a separate start time at the beginning of `run_synthesize()` and use it for the final elapsed calculation.

## References

- `crates/refinery_cli/src/commands/synthesize.rs`
- Found by CodeRabbit review on PR #26
