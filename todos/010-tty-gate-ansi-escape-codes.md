---
title: "fix: TTY-gate ANSI escape codes in progress output"
priority: low
milestone: v0.3
status: completed
completed: 2026-05-22
---

# TTY-gate ANSI escape codes in progress output

Completed in implementation plan `docs/plans/2026-05-22-002-fix-tty-gate-ansi-escape-codes-plan.md`.

## Problem

Hard-coded ANSI escape sequences (`\x1b[32m`, `\x1b[31m`, etc.) in `synthesize.rs` and `progress.rs` emit raw escape codes when stderr isn't a TTY (piped logs, CI).

## Fix

Gate all color output on `std::io::stderr().is_terminal()`. Either use a small color abstraction or pass a `use_color: bool` flag through the display layer.

## Origin

CodeRabbit review comment on PR #26 (comment 2956854928).
