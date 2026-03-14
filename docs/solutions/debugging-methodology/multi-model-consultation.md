---
title: "Multi-model consultation for debugging: when to call in other models"
category: debugging-methodology
tags: [debugging, multi-model, codex, gemini, consultation, diagnosis]
module: general
symptom: "Single-model debugging loops through increasingly wrong hypotheses"
root_cause: "Anchoring bias: once a model commits to a hypothesis (signal handlers), it keeps iterating on that hypothesis instead of questioning the premise"
date: 2026-03-14
---

# Multi-model consultation for debugging: when to call in other models

## Context

While debugging a Ctrl+C handling issue in ConVerge Refinery, Claude Opus went through 7+ failed attempts over multiple hours. Each attempt was based on the same incorrect premise: that the problem was with signal handler installation, ordering, or thread delivery. The actual problem was that the terminal driver wasn't generating the signal at all.

Two other models (Codex 5.4 xhigh and Gemini 3.1 Pro), given the source code with no prior context, both independently identified the correct root cause in their first response.

## When to Consult Other Models

### Trigger: 3+ failed attempts at the same category of fix

If you've tried 3+ variations of the same approach (e.g., different signal handler installation strategies) and none work, the premise is likely wrong. A fresh perspective from another model won't share your anchoring bias.

### Trigger: Tests pass but the fix doesn't work

When your test methodology is flawed (as it was here — backgrounded processes ignore SIGINT), you can iterate forever on "verified" fixes that don't actually work. Another model will question your testing approach.

### Trigger: The symptom has an asymmetry you can't explain

"`kill -INT` works but Ctrl+C doesn't" is a strong signal that the problem is in the terminal layer, not the signal layer. If you can't explain the asymmetry, you're looking at the wrong layer.

## How to Consult Effectively

### Give them the source code, not a summary

The first consultation gave models only a summary of the problem and approaches tried. This led to the same incorrect hypotheses (signal-hook-registry, tokio thread delivery, etc.).

The second consultation gave models **the actual source code** of the relevant files:
- `main.rs` (CLI entry point)
- `process.rs` (child process spawning)
- `engine.rs` (consensus engine)
- `propose.rs` and `evaluate.rs` (phase implementations)

Both models immediately focused on the correct area: `process.rs` and how children are spawned.

### Don't pollute their context with your failed hypotheses

The first consultation included all previous attempts and reasoning. This biased the models toward the same signal-handler hypothesis. The second consultation presented only the problem, the symptom, the observation (`kill -INT` works, Ctrl+C doesn't), and the source code.

### Use the models' native CLIs directly

```bash
# Codex
codex exec --json --sandbox read-only --skip-git-repo-check \
  --model gpt-5.4 --config "model_reasoning_effort=xhigh" \
  -- "$(cat /tmp/problem_with_source.md)"

# Gemini
gemini --output-format json --model gemini-3.1-pro-preview \
  --sandbox --prompt "$(cat /tmp/problem_with_source.md)"
```

This gives each model a completely fresh context with no bias from your previous attempts.

## Results from This Case

| Model | Key Insight | Suggested Fix |
|---|---|---|
| Codex 5.4 | "stdin inherited → child can own/mutate the TTY" | `stdin(Stdio::null())` + `setsid()` |
| Gemini 3.1 Pro | "CLIs call tcsetattr() to disable ISIG" | `process_group(0)` (insufficient but correct direction) |
| Both | "This is a terminal delivery problem, not a signal handler problem" | - |

Both models identified the root cause in their first response. The key was: no anchoring bias, full source code, and the critical observation about `kill -INT` vs Ctrl+C.

## Prevention

- After 3 failed fix attempts on the same hypothesis, stop and consult another model
- Always include the actual source code when consulting
- Don't include your failed attempts in the consultation — let the model form its own hypothesis
- Use the asymmetry in symptoms as a diagnostic tool: if two paths to the same signal give different results, the difference is in the path, not the signal
