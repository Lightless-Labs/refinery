---
title: "Provider CLI quirks: flags, scores, previews, and model lifecycle"
category: integration-issues
tags: [codex, gemini, claude, float-score, preview, spinner, elapsed, permanently-dropped]
module: refinery_core, tundish_providers, refinery_cli
symptom: "Various small bugs: float scores fail parsing, preview shows only first line, spinner time resets, failed models retried"
root_cause: "Each issue has a different root cause — collected here as a reference"
date: 2026-03-15
---

# Provider CLI quirks and fixes

## Float score coercion (8.0 vs 8)

Models sometimes return evaluation scores as floats (`8.0`) instead of integers (`8`). The original code called `.as_u64()` which fails for floats.

**Fix:** Try `as_u64()` first, fall back to `as_f64()` with rounding and range check:

```rust
let score_value = parsed["score"]
    .as_u64()
    .or_else(|| {
        parsed["score"].as_f64().and_then(|f| {
            let rounded = f.round();
            if (0.0..=10.0).contains(&rounded) {
                Some(rounded as u64)
            } else {
                None
            }
        })
    })
```

## Preview newline collapsing

A response like `"42\n\nIn Douglas Adams'..."` previewed as just `"42"` because `preview()` took only the first line. With a word count of 40, `"42"` was confusing.

**Fix:** Collapse all whitespace into spaces:

```rust
pub fn preview(text: &str, max_chars: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // ... truncate to max_chars
}
```

## Spinner elapsed time reset

The spinner tracked its own `Instant` which reset whenever another model's progress event cleared the label. Model A at 45s would show "3s" after model B's event reset the timer.

**Fix:** Use the `elapsed` parameter from the tundish progress callback, which tracks from subprocess spawn:

```rust
// Tundish callback provides real elapsed
move |model: &ModelId, lines: usize, elapsed: Duration| {
    s.label = Some(format!("{model} — {lines} lines, {}s", elapsed.as_secs()));
}
```

## Codex --skip-git-repo-check

Codex refuses to run outside a trusted git directory: `"Not inside a trusted directory and --skip-git-repo-check was not specified."` Added `--skip-git-repo-check` to build_args.

## Gemini --allowed-tools deprecation

`--allowed-tools ""` causes `FatalCancellationError` with exit code 130. The flag is deprecated. Removed entirely — `--sandbox` + `--approval-mode plan` is sufficient for tool restriction.

## Permanently dropping failed models

Models with permanent failures (invalid model, auth, binary not found) were retried every round. Added `ProviderError::is_permanent()` and `permanently_dropped` list in the engine session. Transient failures (timeout) are still retried.

The progress display tracks dropped models separately so they don't appear as spinners in subsequent rounds.

## Word pluralization

`"1 words"` → `"1 word"`. Simple conditional:

```rust
let w = if word_count == 1 { "word" } else { "words" };
```

## Cross-references

- `crates/refinery_core/src/phases/evaluate.rs` — float score parsing
- `crates/refinery_core/src/progress.rs` — preview function
- `crates/refinery_cli/src/progress.rs` — spinner and display
- `crates/tundish_providers/src/codex.rs` — skip-git-repo-check
- `crates/tundish_providers/src/gemini.rs` — allowed-tools removal
- `crates/refinery_core/src/engine.rs` — permanently_dropped
