---
title: "fix: total_calls in synthesize should include failed synthesis attempts"
priority: low
milestone: v0.3
---

# Include failed synthesis attempts in total_calls

## Problem

`total_calls` in synthesize output only counts successful synthesis proposals. Failed/timed-out synthesis attempts disappear from reported metrics.

## Solution

Track all synthesis attempts (successful + failed) and include in total_calls.

## References

- `crates/refinery_cli/src/commands/synthesize.rs`
- Found by CodeRabbit review on PR #26
