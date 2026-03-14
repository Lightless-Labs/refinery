---
title: "feat: Replace custom spinner with indicatif multi-spinner and comfy-table"
type: feat
date: 2026-03-14
**Enhanced:** 2026-03-14 (via `/deepen-plan`)
---

# Replace custom spinner with indicatif multi-spinner and comfy-table

## Overview

Replace the hand-rolled spinner and score table rendering in `refinery_cli` with `indicatif::MultiProgress` for per-model concurrent spinners and `comfy-table` for score tables. Clear previous round output when starting a new round. Show all models simultaneously during propose/evaluate phases.

## Problem Statement

The current CLI progress display has several UX issues:

1. **Single-line spinner** — only shows the last model that reported output, hiding concurrent activity from other models
2. **Verbose per-event output** — each proposal/evaluation prints its own line, creating long scrollback that pushes the score table offscreen
3. **No round cleanup** — previous round output accumulates, making it hard to see the current state
4. **Manual ANSI escape codes** — fragile, not width-aware, no alignment

## Proposed Solution

### New UX Layout (per round)

```
  Round 2/5
                                    R1    R2
    claude-code/claude-opus-4-6    9.0   ···
    codex-cli/gpt-5.4              8.0   ···
    gemini-cli/gemini-3.1-pro      6.5   ···

  ── propose ──
    ⠋ claude-code/claude-opus-4-6    — 12 lines, 26s
    ✓ codex-cli/gpt-5.4              — 42 words
    ⠸ gemini-cli/gemini-3.1-pro      — 8 lines, 18s
```

- Score table is compact, always visible at the top
- Each model gets its own spinner line
- Completed models show ✓/✗ with result summary
- In-progress models show animated spinner with line count + elapsed
- When a new round starts, clear the previous round's output

### Dependencies

```toml
# Add to workspace Cargo.toml [workspace.dependencies]
indicatif = "0.17"
comfy-table = "7"
```

### Architecture

**Current flow:**
```
tundish ProgressFn → SpinnerState (Mutex) → tick task (single line eprint!)
refinery ProgressFn → render_progress() → SpinnerState → tick task
```

**New flow:**
```
tundish ProgressFn → indicatif ProgressBar per model (set_message)
refinery ProgressFn → indicatif MultiProgress (add/finish bars)
Score table → comfy-table (printed above MultiProgress)
```

Key change: `indicatif::MultiProgress` manages cursor movement for multiple lines automatically. No manual ANSI escape codes.

## Implementation Phases

### Phase 1: Add dependencies, create progress module

**File: `crates/refinery_cli/Cargo.toml`**
- Add `indicatif` and `comfy-table` workspace dependencies

**File: `crates/refinery_cli/src/progress.rs` (new)**
- `RoundDisplay` struct wrapping `MultiProgress` + `HashMap<ModelId, ProgressBar>`
- `fn new_round(round, total, models, scores)` — clear previous, print table, create spinners
- `fn update_model(model, lines, elapsed)` — update spinner message
- `fn model_done(model, success, summary)` — finish with ✓/✗
- `fn model_failed(model, error)` — finish with ✗
- `fn print_convergence(result)` — print convergence status below spinners

### Phase 2: Replace SpinnerState + render_progress

**File: `crates/refinery_cli/src/main.rs`**
- Remove `SpinnerState` struct
- Remove `render_progress()` function
- Remove tick_handle spawn (indicatif ticks its own spinners)
- Create `RoundDisplay` and wire tundish + consensus progress callbacks to it

### Phase 3: Score table with comfy-table

**File: `crates/refinery_cli/src/progress.rs`**
- `fn render_score_table(scores: &[HashMap<String, f64>], winner: Option<&str>)` → `String`
- Use `comfy-table` with minimal styling (no heavy borders)
- Color the winner row green
- Print to stderr via `MultiProgress::println()`

### Phase 4: Round transitions

- On `RoundStarted` event: clear all spinners, print score table, create new spinners
- On `ConvergenceCheck`: print result, finalize score table
- Track which models are active vs completed within each phase

## Acceptance Criteria

- [ ] Each model shows its own spinner line during propose and evaluate
- [ ] Completed models show ✓ with word count or ✗ with error
- [ ] Score table is rendered via comfy-table at the start of each round (round 2+)
- [ ] Previous round's propose/evaluate output is cleared when new round starts
- [ ] All progress output goes to stderr (stdout reserved for JSON output)
- [ ] Non-TTY mode (piped output) degrades gracefully (no spinners, just text)
- [ ] Existing test suite passes
- [ ] `--verbose` and `--debug` flags still work

## Technical Considerations

- `indicatif::MultiProgress` writes to stderr by default
- `MultiProgress::println()` prints text above the spinners without disrupting them
- Dropping a `MultiProgress` clears its spinners
- `ProgressBar::finish_with_message()` replaces spinner with final text
- `comfy-table` supports `Display` trait — `format!("{table}")` gives the rendered string
- The tundish `ProgressFn` callback signature (`&ModelId, usize, Duration`) maps directly to `ProgressBar::set_message()`

### Research Insights

**indicatif API details (from docs.rs):**
- `MultiProgress::new()` draws to stderr, max 15 Hz refresh
- `MultiProgress` is `Send + Sync` — safe to share via `Arc`
- `ProgressBar::new_spinner()` creates a spinner that draws to stderr
- Spinners do NOT auto-tick — call `enable_steady_tick(Duration)` to spawn a background tick thread
- Template placeholders: `{spinner}`, `{msg}`, `{elapsed}`, `{prefix}`, `{wide_msg}`
- Style colors: `{spinner:.green}`, `{msg:.cyan}`, `{wide_msg:^.red.on_blue}`
- `MultiProgress::clear()` clears all bars from display (for round transitions)
- `MultiProgress::suspend(f)` temporarily hides bars, runs f, redraws (for printing score tables)

**Spinner style for our use case:**
```rust
let style = ProgressStyle::with_template("    {spinner:.dim} {wide_msg}")
    .unwrap()
    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "]);
```

The last tick_string is the "done" character (space = invisible after finish).

**Non-TTY graceful degradation:**
- `MultiProgress::is_hidden()` returns true when not a terminal
- When hidden, `println()` and spinner updates are no-ops
- For `--verbose` mode: use `ProgressDrawTarget::hidden()` to disable spinners, print events as plain text

**Thread safety pattern for callbacks:**
```rust
// ProgressBar is Send+Sync, can be cloned and moved into closures
let pb = multi.add(ProgressBar::new_spinner());
let pb_clone = pb.clone();
// Use pb_clone in tundish callback (different thread)
```

**comfy-table minimal style:**
```rust
use comfy_table::{Table, ContentArrangement, presets::NOTHING};

let mut table = Table::new();
table.load_preset(NOTHING);  // No borders at all
table.set_content_arrangement(ContentArrangement::Dynamic);
```

**Terminal state (from docs/solutions/):**
- ISIG restoration on startup is already in place — safe to use terminal libraries
- `setsid()` on children prevents them from corrupting terminal state
- indicatif respects terminal capabilities automatically

**Edge cases to handle:**
- Model names have different lengths — use column alignment
- Round 1 has no scores yet — skip table or show "—"
- Evaluate phase has N*(N-1) pairs, not N models — show reviewer→reviewee pairs as spinners
- `--verbose` and `--debug` should bypass indicatif entirely (plain text output)

## Files Changed

| File | Change |
|---|---|
| `Cargo.toml` | Add `indicatif`, `comfy-table` to workspace deps |
| `crates/refinery_cli/Cargo.toml` | Add deps |
| `crates/refinery_cli/src/main.rs` | Remove SpinnerState, render_progress, tick_handle; wire new progress module |
| `crates/refinery_cli/src/progress.rs` | New file: RoundDisplay, score table rendering |

## References

- [indicatif MultiProgress docs](https://docs.rs/indicatif/latest/indicatif/struct.MultiProgress.html)
- [indicatif ProgressStyle template keys](https://docs.rs/indicatif/latest/indicatif/#templates)
- [comfy-table docs](https://docs.rs/comfy-table/latest/comfy_table/)
- [comfy-table presets](https://docs.rs/comfy-table/latest/comfy_table/presets/index.html)
- Current spinner: `crates/refinery_cli/src/main.rs:645-927` (SpinnerState + render_progress)
- Terminal safety: `docs/solutions/runtime-errors/ctrl-c-sigint-terminal-isolation.md`
