---
title: "test: brainstorm verb integration tests"
type: fix
status: active
date: 2026-04-01
origin: docs/plans/2026-03-31-001-feat-brainstorm-verb-plan.md
---

# Brainstorm Verb Integration Tests

## Overview

Add integration tests for the brainstorm verb's round loop, panel selection, dry-run estimation, and single-model short-circuit — all using mock providers, no real model calls.

## Approach

The brainstorm round loop is currently inlined in the CLI command (`commands/brainstorm.rs`). To test it with mock providers without calling `build_providers()` (which resolves real CLI binaries), extract the core loop logic into a testable function in `refinery_core`.

## Implementation Units

- [ ] **Unit 1: Extract brainstorm loop into refinery_core**

  **Goal:** Move the core round loop logic from `commands/brainstorm.rs` into a testable function in `refinery_core` that accepts `&[Arc<dyn ModelProvider>]` directly.

  **Files:**
  - Create: `crates/refinery_core/src/brainstorm.rs`
  - Modify: `crates/refinery_core/src/lib.rs`
  - Modify: `crates/refinery_cli/src/commands/brainstorm.rs` (call extracted function)

- [ ] **Unit 2: Integration tests with mock providers**

  **Goal:** Test the brainstorm loop end-to-end using `EchoProvider` with queued responses.

  **Files:**
  - Test: `crates/refinery_core/src/brainstorm.rs` (inline tests)

  **Test scenarios:**
  - Round loop accumulates score histories correctly across 3 rounds (no review text leaks)
  - Panel selection returns controversial answers (high evaluator disagreement) over uniform ones
  - Single model returns that model's answer directly (no evaluation)
  - All models failing returns error
