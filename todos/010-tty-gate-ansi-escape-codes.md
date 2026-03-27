# TTY-gate ANSI escape codes in progress output

## Problem

Hard-coded ANSI escape sequences (`\x1b[32m`, `\x1b[31m`, etc.) in `synthesize.rs` and `progress.rs` emit raw escape codes when stderr isn't a TTY (piped logs, CI).

## Fix

Gate all color output on `std::io::stderr().is_terminal()`. Either use a small color abstraction or pass a `use_color: bool` flag through the display layer.

## Origin

CodeRabbit review comment on PR #26 (comment 2956854928).
