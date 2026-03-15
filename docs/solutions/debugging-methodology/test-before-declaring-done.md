---
title: "Test before declaring done: the cargo fmt gate and other pre-push checks"
category: debugging-methodology
tags: [testing, ci, cargo-fmt, clippy, pre-push, methodology]
module: general
symptom: "CI fails on formatting after declaring fix is done"
root_cause: "Running cargo clippy but not cargo fmt before pushing"
date: 2026-03-15
---

# Test before declaring done

## The pattern that wasted time

1. Make a code change
2. Run `cargo clippy` — passes
3. Commit and push
4. CI fails on `cargo fmt --check`
5. Fix formatting, commit, push again
6. Repeat

This happened multiple times in a single PR, each time adding a round-trip to CI.

## The gate

Before every push, run this sequence:

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo fmt --check --all  # Final verification
```

The last `cargo fmt --check` catches cases where clippy or test fixes introduced new formatting drift.

## Broader lesson: test what CI tests

Whatever CI checks, run locally first. Don't declare something "fixed" until you've verified it passes the same checks CI will run. This applies to:

- Formatting (`cargo fmt --check`)
- Linting (`cargo clippy -- -D warnings`)
- Tests (`cargo test`)
- Build (`cargo build --release`)

If CI also runs `cargo install`, test that too — it uses release profile which may surface different warnings.

## The user's feedback

> "How about testing your work before telling me you're done?"

This was said after a clippy fix was pushed without running full workspace clippy (only the single crate was checked). The fix passed for `refinery_core` but `refinery_cli` had a separate clippy warning.

**Always run checks on the full workspace**, not just the crate you changed.
