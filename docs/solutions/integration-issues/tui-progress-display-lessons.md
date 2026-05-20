---
title: "TUI progress display: why simple eprintln beats terminal libraries"
category: integration-issues
tags: [tui, ratatui, indicatif, progress, terminal, eprintln, spinner, inline-viewport]
module: refinery_cli
symptom: "Progress display breaks with indicatif (ordering issues, duplicate headers) and ratatui (raw mode conflicts with tracing, viewport sizing)"
root_cause: "Terminal UI libraries manage cursor position and redraw state, which conflicts with concurrent async callbacks, tracing output, and multi-phase display requirements"
date: 2026-03-15
---

# TUI progress display: why simple eprintln beats terminal libraries

## Context

The refinery CLI needs to show progress during a multi-model consensus run: spinners during propose, completion lines, evaluation results, score tables, round transitions. Multiple attempts were made to use terminal UI libraries.

## What was tried (all failed)

### 1. indicatif MultiProgress

**Approach:** One `ProgressBar` per model/evaluation pair, managed by `MultiProgress`.

**Problems encountered:**
- `multi.println()` prints *above all managed bars*, so phase headers appeared in wrong positions relative to completion lines
- `finish_with_message()` keeps bars in the managed set — late completions from propose appeared mixed with evaluate spinners
- `finish_and_clear()` dropped ProgressBar references, causing indicatif to remove the lines entirely
- Keeping finished bars in a `Vec` to prevent removal caused score tables to duplicate (indicatif redraws all managed bars every tick)
- No way to "commit" bars to scrollback without managing their lifecycle

**Key insight:** indicatif's model of "managed bars + println scrollback" fundamentally conflicts with showing multiple phases where completed items from phase 1 should stay visible during phase 2.

### 2. ratatui with Viewport::Inline

**Approach:** Render the entire progress state as a single frame using `Viewport::Inline(N)`.

**Problems encountered:**
- Raw mode (`enable_raw_mode()`) conflicts with `tracing-subscriber` writing to stderr — tracing output corrupts the viewport
- `insert_before()` for flushing rounds to scrollback worked in isolation but the viewport sizing was fragile
- Terminal resize during rendering caused drift
- The complexity of managing `Terminal`, `Arc<Mutex<State>>`, viewport height calculation, and background tick tasks was disproportionate to the actual UX need

**Key insight:** ratatui is designed for full-screen or semi-persistent TUI apps. A CLI that prints results and exits doesn't benefit from its rendering model.

### 3. Manual ANSI escape codes (cursor up + clear)

**Approach:** Track line count, erase previous frame with `\x1b[{N}A` + `\x1b[2K`, redraw.

**Problems encountered:**
- Long error messages wrap to multiple terminal lines, but `last_frame_lines` only counts `\n` boundaries — cursor-up count is wrong, garbage accumulates
- Truncating to terminal width (via `ioctl TIOCGWINSZ`) helped but added complexity for marginal benefit

## What works: plain eprintln

```rust
// Print events as they happen. One line per event. No cursor management.
eprintln!("\r\x1b[2K    \x1b[32m✓\x1b[0m {model} proposed ({wc} words) — \"{preview}\"");
```

A background tokio task ticks a single-line spinner at 80ms for the current in-progress model. When an event arrives, clear the spinner line (`\r\x1b[2K`), print the event, and the spinner resumes on the next tick.

**279 lines total.** No terminal libraries, no cursor management, no raw mode, no viewport.

### Why it works despite being "primitive"

1. **No ordering issues** — events print in callback order, which is correct by construction
2. **No conflicts with tracing** — both use stderr normally
3. **No cursor math** — every line goes to scrollback immediately
4. **No cleanup needed** — just clear the spinner line on finish
5. **Non-TTY graceful degradation** — `is_terminal()` check skips spinner, events still print

### The tradeoff

Previous rounds accumulate in scrollback instead of being cleared. For a CLI that runs for 1-5 minutes, this is fine — users can scroll up to see history. The score table at each convergence check shows progressive columns (R1, R2, R3...) so the current state is always visible.

## Lesson

**Don't reach for a terminal library until you've exhausted what `eprintln!` can do.** The complexity budget of indicatif/ratatui is justified for interactive TUIs (dashboards, file managers, editors). For a CLI that prints progress and exits, `eprintln!` with a single-line spinner covers 95% of the UX need at 5% of the complexity.

If you need to clear previous output, you need a terminal library. If you don't need to clear previous output, you don't.

## Cross-references

- `docs/solutions/runtime-errors/ctrl-c-sigint-terminal-isolation.md` — raw mode + signal handling conflicts
- `docs/solutions/integration-issues/cli-provider-subprocess-isolation.md` — setsid prevents child TTY interference
