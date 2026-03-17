---
title: "refactor: Restructure CLI into refinery converge subcommand"
type: refactor
date: 2026-03-17
**Completed:** 2026-03-17
---

# Restructure CLI into `refinery converge` subcommand

## Overview

Move all current CLI behavior under `refinery converge` to make room for future verbs (`synthesize`, `brainstorm`, `mine`) that apply different selection strategies to the same multi-model loop.

## Problem Statement

The flat CLI (`refinery "prompt" --models ...`) doesn't scale. Each verb needs its own flags:
- `converge`: `--threshold`, `--max-rounds`, `--stability-rounds`
- `synthesize`: `--synthesis-threshold`
- `brainstorm`: `--panel-size`, `--diversity-threshold`

Mixing all flags at the top level creates confusion and namespace collisions.

## Proposed Solution

Use clap's `#[derive(Subcommand)]` to split into:

```
refinery <COMMAND>
  converge    Reach consensus across multiple models
  help        Print help
```

### Flag split

**Shared (via `#[command(flatten)]` on `SharedArgs`):**
- `PROMPT` (positional)
- `--file`, `--models`, `--timeout`, `--idle-timeout`, `--max-concurrent`
- `--output-format`, `--output-dir`, `--allow-tools`
- `--verbose`, `--debug`, `--dry-run`

**Converge-specific (on `ConvergeArgs`):**
- `--threshold` (default 8.0)
- `--max-rounds` (default 5)

### No backward compatibility

`refinery "prompt"` without a verb errors with subcommand help. Nobody is using the flat form in production.

## Implementation

### Files changed

| File | Change |
|---|---|
| `crates/refinery_cli/src/main.rs` | Split `Cli` into `Cli` (top-level + subcommand) + `SharedArgs` (shared flags) + `ConvergeArgs` (converge-specific). Extract `run_converge()` from `async_main()`. |
| `README.md` | All examples updated from `refinery "prompt"` to `refinery converge "prompt"` |
| `todos/002-cli-subcommand-converge.md` | Marked completed |

### Code structure

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Converge(ConvergeArgs),
}

#[derive(Parser)]
struct SharedArgs { /* models, timeout, output, etc. */ }

#[derive(Parser)]
struct ConvergeArgs {
    #[command(flatten)]
    shared: SharedArgs,
    threshold: f64,
    max_rounds: u32,
}

async fn async_main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Converge(args) => run_converge(args).await,
    }
}
```

### What didn't change

- `refinery_core` — no changes (engine, phases, strategy are verb-agnostic)
- `tundish_core`, `tundish_providers` — no changes (dispatch layer is verb-agnostic)
- `progress.rs` — no changes (display callbacks work the same)
- All tests pass unchanged

## Future verbs

Each new verb adds a variant to `Command` and its own `run_<verb>()` function. `SharedArgs` is reused via `#[command(flatten)]`.

| Verb | Tracked in | Milestone |
|---|---|---|
| `synthesize` | `todos/003-verb-synthesize.md` | v0.3 |
| `brainstorm` | `todos/004-verb-brainstorm.md` | v0.3 |
| `mine` | `todos/005-verb-mine.md` | v0.4 |

## References

- Brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
- PR: #24
