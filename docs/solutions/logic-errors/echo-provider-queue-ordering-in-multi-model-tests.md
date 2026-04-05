---
title: EchoProvider queue ordering in multi-model evaluation tests
date: 2026-04-04
category: logic-errors
module: refinery_core
problem_type: logic_error
component: testing_framework
symptoms:
  - Integration tests with EchoProvider produce non-deterministic controversy scores
  - Panel selection tests fail intermittently depending on HashMap iteration order
  - Evaluatees receive wrong scores from evaluators in mock-based tests
root_cause: logic_error
resolution_type: test_fix
severity: medium
tags:
  - echo-provider
  - mock-testing
  - hashmap-ordering
  - multi-model
  - evaluation-loop
  - controversy-scoring
  - brainstorm-verb
---

# EchoProvider Queue Ordering in Multi-Model Evaluation Tests

## Problem

Integration tests using `EchoProvider` for multi-model evaluation loops (brainstorm, synthesize) produce non-deterministic results because the FIFO response queue interacts unpredictably with HashMap iteration order during the evaluate phase.

## Symptoms

- Tests asserting specific controversy scores or panel rankings fail intermittently
- A model queued with `[propose_answer, score_3_for_A, score_9_for_B]` sometimes gives score_3 to B and score_9 to A
- `controversial_answer_ranks_higher` test fails with unexpected stddev values

## What Didn't Work

- Queuing per-evaluatee scores in a specific order (e.g., alphabetical by model ID) — HashMap iteration is not ordered
- Using `BTreeMap` for proposals — would change the production code to accommodate tests
- Trying to predict the iteration order — platform-dependent, changes between runs

## Solution

Use **uniform scores per evaluator model** — every evaluatee gets the same score from a given evaluator. This makes the test deterministic regardless of HashMap iteration order.

```rust
// BAD: Non-deterministic — queue order depends on HashMap iteration
let pb = EchoProvider::new("test/b");
pb.queue_response(r#"{"answer": "safe answer"}"#.to_string());
pb.queue_response(eval_json(3)); // intended for model A
pb.queue_response(eval_json(9)); // intended for model C
// But model C might be evaluated first, getting score 3!

// GOOD: Deterministic — uniform scores per evaluator
let pb = EchoProvider::new("test/b");
pb.queue_response(r#"{"answer": "safe answer"}"#.to_string());
pb.queue_response(eval_json(3)); // B always gives 3, regardless of evaluatee
pb.queue_response(eval_json(3)); // B always gives 3
```

With 3 models (A gives 7, B gives 3, C gives 9), the scores each answer receives are deterministic:

| Answer | Evaluated by | Scores received | Mean | Stddev | Controversy |
|--------|-------------|-----------------|------|--------|-------------|
| A | B(3), C(9) | [3, 9] | 6.0 | 3.0 | 18.0 |
| B | A(7), C(9) | [7, 9] | 8.0 | 1.0 | 8.0 |
| C | A(7), B(3) | [7, 3] | 5.0 | 2.0 | 10.0 |

Model A is always most controversial — test passes deterministically.

## Why This Works

`EchoProvider` uses a single `VecDeque` FIFO queue for all `send_message()` calls. Within a round, each model is called once for propose (deterministic), then once per evaluatee for evaluate. The evaluate call order depends on iterating over `round_proposals: HashMap<ModelId, String>` — HashMap iteration is unordered. By making all eval responses from a given evaluator identical, the queue consumption order doesn't affect which evaluatee gets which score.

## Prevention

- When writing multi-model evaluation tests with `EchoProvider`, always use uniform scores per evaluator
- If you need per-evaluatee score differentiation, use a smarter mock that inspects the message content to determine the response (not currently implemented)
- Document this constraint in test helper comments near `EchoProvider` usage

## Related Issues

- PR #27: brainstorm verb integration tests
- `crates/refinery_core/src/testing.rs`: EchoProvider implementation
- `crates/refinery_core/src/brainstorm.rs`: tests using this pattern
