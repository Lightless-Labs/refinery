---
title: "Consensus tiebreaking and winner semantics"
category: logic-errors
tags: [consensus, tiebreaking, convergence, winner, hashmap, determinism, stability]
module: refinery_core
symptom: "Winner flips between tied models every round, preventing convergence despite score above threshold"
root_cause: "HashMap iteration order is random — max_by picks arbitrary winner among ties, resetting stability counter"
date: 2026-03-15
---

# Consensus tiebreaking and winner semantics

## Problem 1: Non-deterministic tiebreaking

When two models have the same score (e.g., both at 8.8), `max_by` on a `HashMap` picks whichever entry the iterator visits first — which is random per run and per round. The "winner" flips between rounds:

```
R1: opus=8.8 ★, kimi=8.8   (opus wins by luck)
R2: kimi=9.2 ★, opus=9.0   (kimi wins legitimately)
R3: glm=9.0 ★, kimi=9.0    (glm wins by luck)
```

The stability counter requires the *same* model to lead for 2 consecutive rounds. With random tiebreaking, ties reset stability every time, preventing convergence even when a model consistently scores above threshold.

### Fix

When scores are tied, prefer the previous winner (preserves stability). If no previous winner among leaders, use lexicographic model ID order for determinism:

```rust
let best_score = mean_scores.values().copied().fold(f64::NEG_INFINITY, f64::max);
let mut leaders: Vec<&ModelId> = mean_scores.iter()
    .filter(|(_, s)| (**s - best_score).abs() < f64::EPSILON)
    .map(|(id, _)| id)
    .collect();
leaders.sort_by_key(std::string::ToString::to_string);

let current_winner = if let Some(prev) = previous_winner {
    if leaders.contains(&prev) { Some(prev.clone()) }
    else { leaders.first().map(|id| (*id).clone()) }
} else {
    leaders.first().map(|id| (*id).clone())
};
```

## Problem 2: False winner on max_rounds_exceeded

When the consensus loop hits `max_rounds` without convergence, the engine was arbitrarily declaring the last round's leader as "winner" — even though no consensus was reached. With 6 models all scoring 8.5-9.2, a different model led each round. The "winner" was whoever happened to lead in round 5.

### Fix

`winner` and `answer` are now `Option`. When `MaxRoundsExceeded`, both are `None`. All answers with scores are still returned in `all_answers`:

```json
{
  "status": "max_rounds_exceeded",
  "final_round": 5,
  "all_answers": [
    { "model_id": "...", "answer": "...", "mean_score": 9.0 },
    { "model_id": "...", "answer": "...", "mean_score": 8.8 }
  ]
}
```

No fake winner. The consumer decides what to do with the ranked answers.

## Prevention

- Never use `max_by` on `HashMap` for deterministic selection — always sort first or use a tiebreaker
- Distinguish "converged" (consensus reached) from "finished" (ran out of rounds) in the output schema
- Make winner/answer optional in the type system so the compiler enforces handling of the no-consensus case

## Cross-references

- `crates/refinery_core/src/phases/close.rs` — tiebreaking logic
- `crates/refinery_core/src/engine.rs` — `finalize_with_status` with `Option` winner
