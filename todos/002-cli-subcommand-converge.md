---
title: "refactor: restructure CLI into refinery converge subcommand"
priority: high
milestone: v0.2
---

# Restructure CLI into `refinery converge` subcommand

## Problem

The current CLI is flat: `refinery "prompt" --models ...`. This doesn't scale when adding new verbs (`synthesize`, `brainstorm`, etc.) that need their own flags and behavior.

## Solution

Move all current behavior under `refinery converge "prompt" --models ...`. Split flags into shared (top-level) and verb-specific:

**Shared:** `--models`, `--timeout`, `--idle-timeout`, `--max-concurrent`, `--output-format`, `--output-dir`, `--allow-tools`, `--verbose`, `--debug`, `--dry-run`

**converge-specific:** `--threshold`, `--max-rounds`, `--stability-rounds`

No backward compatibility — `refinery "prompt"` without a verb should error with usage help.

## References

- Brainstorm: `docs/brainstorms/2026-03-17-cli-subcommand-verbs-brainstorm.md`
