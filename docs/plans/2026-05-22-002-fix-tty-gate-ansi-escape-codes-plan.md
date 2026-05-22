---
title: "fix: TTY-gate ANSI escape codes"
type: fix
status: completed
date: 2026-05-22
completed: 2026-05-22
todo: 010-tty-gate-ansi-escape-codes
---

# TTY-Gate ANSI Escape Codes Plan

## Problem

Some CLI progress/status output embeds ANSI color escape codes directly. `ProgressDisplay` already avoids rendering when stderr is not a TTY, but `synthesize` has direct `eprintln!` progress lines that can emit raw color codes into piped logs, JSON stderr, or CI output.

## Goals

- Prevent raw ANSI color codes from `synthesize` progress/status lines when stderr is not a TTY.
- Keep colored output for interactive TTY sessions.
- Keep existing text content and exit-code behavior unchanged.
- Add a small test around the color formatting helper.

## Non-Goals

- Do not redesign progress rendering or add a dependency.
- Do not suppress synthesize progress messages entirely.
- Do not change JSON stdout schemas.

## Implementation Notes

- Add a tiny local helper in `synthesize.rs` that wraps text in ANSI color only when `std::io::stderr().is_terminal()` is true.
- Thread a `use_color` boolean through direct synthesize progress logging sites.
- Leave `ProgressDisplay` cursor-control/color output as-is because all those rendering paths are already guarded by `should_render()` (`!hidden && stderr.is_terminal()`).

## Verification

Completed:

- `cargo fmt --all -- --check`
- `cargo test -p refinery_cli`
- `cargo clippy -p refinery_cli --all-targets -- -D warnings`
- Source scan confirmed `synthesize.rs` ANSI escapes are centralized in the `colorize` helper/test only.
