---
date: 2026-05-21
topic: brainstorm-smoke-test-field-report
todo: 019-brainstorm-smoke-test-field-report
---

# Brainstorm Smoke Test Field Report

## Summary

The smoke test did **not** establish a valid multi-model v0 brainstorm baseline because only `codex-cli/gpt-5.4` produced usable responses in this environment.

The important finding is operational/UX rather than strategic: `refinery brainstorm` can silently degrade from a requested multi-model panel to a single-provider run when provider calls fail. In that degraded mode there are no peer evaluations, so panel scores are all `0.0`, `per_evaluator_scores` is empty, and controversial selection is not exercised.

## Environment

- Repo: `refinery` on `main`
- Command runner: `cargo run -q -p refinery_cli -- brainstorm ...`
- Requested model set: `claude-code,codex-cli,gemini-cli`
- Working provider observed: `codex-cli/gpt-5.4`
- Blocked providers observed:
  - `claude-code/claude-opus-4-6`: direct debug run reported `api_error_status: 403` / "Your organization does not have access to Claude."
  - `gemini-cli/gemini-3.1-pro-preview`: direct CLI run reported `RetryableQuotaError: You have exhausted your capacity on this model.`

Artifacts and raw command outputs were captured under `target/brainstorm-smoke/`.

## Commands Run

### Dry run

```sh
cargo run -q -p refinery_cli -- brainstorm --dry-run "test" \
  --models claude-code,codex-cli,gemini-cli \
  --max-rounds 2 \
  --panel-size 3
```

Result:

```text
Dry run estimate:
  Models: 3
  Max rounds: 2
  Calls per round: 9
  Total calls (max): 18
  Panel size: 3
```

### Product / strategy prompt

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Generate unconventional but practical product ideas for a privacy-first personal knowledge assistant for software teams. Focus on ideas a small startup could prototype in 6 weeks." \
  --models claude-code,codex-cli,gemini-cli \
  --max-rounds 2 \
  --panel-size 3 \
  --output-format json \
  --output-dir target/brainstorm-smoke/product \
  > target/brainstorm-smoke/logs/product.json \
  2> target/brainstorm-smoke/logs/product.stderr
```

Result: exit `0`, but only one panel entry:

- `model_id`: `codex-cli/gpt-5.4`
- `total_rounds`: `2`
- `total_calls`: `6`
- `mean_score`: `0.0`
- `stddev`: `0.0`
- `controversy_score`: `0.0`
- `per_evaluator_scores`: `[]`

Output excerpt:

> The better wedge is tacit engineering memory: why decisions were made, what failed before, and which fixes only ever lived in someone’s terminal. Privacy-first is not just a compliance feature here; it is what makes teams willing to capture the messy, high-value stuff.

Notable proposed ideas included:

- Shadow Notebook → Shared Playbook
- Why-Not Blame
- Knowledge Half-Life Engine
- Review Precedent Copilot
- Negative Knowledge Vault
- Architecture Drift Witness
- Handoff Capsule Generator

### Technical / design prompt with requested model set

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Design a lightweight artifact format for recording multi-model brainstorming runs so later benchmarking can compare divergence, novelty, and feasibility without storing secrets. Include schema shape and implementation trade-offs." \
  --models claude-code,codex-cli,gemini-cli \
  --max-rounds 2 \
  --panel-size 3 \
  --output-format json \
  --output-dir target/brainstorm-smoke/technical \
  > target/brainstorm-smoke/logs/technical.json \
  2> target/brainstorm-smoke/logs/technical.stderr
```

Result: exit `1`:

```json
{
  "status": "error",
  "error": {
    "code": "brainstorm_failed",
    "message": "round 2: All models failed to propose.",
    "provider": null,
    "round": null,
    "phase": "brainstorm",
    "retryable": true
  }
}
```

Round 1 did produce a Codex proposal artifact, but the overall run failed in round 2. No stdout JSON result was produced.

### Technical / design prompt with Codex only

```sh
cargo run -q -p refinery_cli -- brainstorm \
  "Design a lightweight artifact format for recording multi-model brainstorming runs so later benchmarking can compare divergence, novelty, and feasibility without storing secrets. Include schema shape and implementation trade-offs." \
  --models codex-cli \
  --max-rounds 2 \
  --panel-size 1 \
  --output-format json \
  --output-dir target/brainstorm-smoke/technical-codex-only \
  > target/brainstorm-smoke/logs/technical-codex-only.json \
  2> target/brainstorm-smoke/logs/technical-codex-only.stderr
```

Result: exit `0`, single-provider panel with no evaluation scores.

Output excerpt:

> Store the run as a lineage graph of ideas, not as a transcript. The artifact should preserve three things: who proposed what, how ideas branched/merged, and how peers judged them. It should not preserve raw prompts, raw outputs, API keys, pasted code, or anything whose only value is replay.

The answer proposed a `braid.v1.json` artifact with:

- prompt/text HMAC commitments instead of raw secret-bearing text
- delexicalized idea atoms
- turn/idea/judgment records
- MinHash/SimHash sketches instead of embeddings
- benchmark primitives such as pairwise distance, idea overlap, and branching factor

## Observations

### Score-only iteration

Inconclusive. With one surviving provider, the run exercised self-history iteration but not multi-model divergence. There was no peer score history to feed back between rounds.

### Controversial selection

Not exercised. With no peer evaluations, every selected candidate had:

- mean score `0.0`
- standard deviation `0.0`
- controversy score `0.0`
- no evaluator scores

The product run still returned `status: "brainstormed"`, which is misleading for a requested three-model controversial panel.

### Panel overlap/diversity

Inconclusive. The product run returned a single long answer containing multiple ideas, not a multi-model panel. The ideas were useful and varied internally, but that is not evidence that the v0 panel strategy creates non-overlapping answer lineages.

### Rubric behavior

Not observed in successful runs because no evaluation scores were produced. The failed/partial runs therefore cannot validate whether originality/insight/depth/feasibility scoring is calibrated.

### Progress, JSON, and artifacts UX

Worked:

- Dry-run call estimate was clear.
- `--output-dir` wrote proposal artifacts for successful proposal rounds.
- JSON success shape was readable.

Weak / failed:

- Provider failures were not visible in non-debug multi-model output.
- The product run exited successfully even though two requested providers did not contribute and no evaluations occurred.
- The technical all-model run failed in round 2 and discarded any structured success summary, despite a round-1 proposal artifact existing.
- `models_dropped` stayed empty in degraded runs, so JSON consumers cannot distinguish "single model requested" from "three models requested, two failed."
- The success response does not explain that `0.0` scores mean "not evaluated" rather than "evaluated and scored zero."

## Conclusions

This run should be treated as a provider-availability smoke test, not as a brainstorm strategy baseline.

Before benchmarking prompt reframing or Open Collider-style domain collisions, run the matrix again in an environment where at least two, ideally three, providers can complete both proposal and evaluation calls. Otherwise the main v0 claims — score-only divergence and controversial panel selection — remain untested.

## Follow-up TODOs

- Created `todos/020-brainstorm-provider-failure-observability.md` to track partial provider failure reporting and degraded-run semantics.
- Keep `todos/013-brainstorm-strategy-benchmarks.md` blocked on a valid multi-provider baseline.
- Keep `todos/018-brainstorm-divergence-expansion.md` deferred until the v0 baseline is actually observed.
- Re-run this smoke matrix after Claude access and Gemini quota/auth are fixed.
