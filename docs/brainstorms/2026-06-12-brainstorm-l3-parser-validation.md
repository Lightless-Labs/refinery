---
date: 2026-06-12
topic: brainstorm-l3-parser-validation
todo: 013-brainstorm-strategy-benchmarks
plan: 2026-05-23-001-research-brainstorm-strategy-benchmarks-plan
related_commit: dc805a7
---

# Brainstorm L3 Parser Validation

## Summary

Ran one paired L3 validation prompt after commit `dc805a7` (`fix: harden brainstorm evaluation score parsing`) to see whether the parser hardening eliminated invalid-evaluation degradation from the Codex/GLM/Kimi-for-coding panel.

Result: both runs still degraded. The baseline (`--prompt-variants off`) still had one GLM invalid brainstorm evaluation score. The expanded run (`--prompt-variants per-model`) had Kimi overload/rate-limit failures and one Kimi invalid brainstorm evaluation score. This means the next useful triage step is to preserve bounded raw invalid evaluation responses in provider failure records rather than continuing to guess parser variants from failure summaries alone.

## Prompt

Architecture/design prompt:

```text
Design a plugin system for local AI tools with strong sandboxing, explicit user consent, and useful extension ergonomics. Generate unconventional but practical architecture ideas.
```

## Models

```text
pi/openai-codex/gpt-5.4:off
pi/zai/glm-5.1:off
pi/kimi-coding/kimi-for-coding:off
```

## Common Settings

```text
--max-rounds 2
--panel-size 3
--quality-floor 7.0
--iteration-strategy score-only
--idle-timeout 480
--timeout 1800
--max-concurrent 1
```

## Artifacts

Root:

```text
target/brainstorm-benchmark-2026-06-11-l3-parser-validation/
```

Run dirs:

```text
target/brainstorm-benchmark-2026-06-11-l3-parser-validation/off/architecture-plugin-sandbox/20260612-101507_design-a-plugin-system-for-local-ai-tool_f92e
target/brainstorm-benchmark-2026-06-11-l3-parser-validation/per-model/architecture-plugin-sandbox/20260612-103202_design-a-plugin-system-for-local-ai-tool_ae1d
```

Analyzer outputs:

```text
target/brainstorm-benchmark-2026-06-11-l3-parser-validation/logs/l3-parser-validation-analysis.txt
target/brainstorm-benchmark-2026-06-11-l3-parser-validation/logs/l3-parser-validation-analysis.json
```

`logs/run-dirs.txt` contained exactly the two run dirs above; the analyzer JSON also referenced exactly those two paths, so no stale artifacts were included in this result.

## Run Results

| Prompt variants | Status | Eval status | Calls | Elapsed | Provider failures |
|---|---|---|---:|---:|---|
| `off` | `degraded` | `partial` | 18 | ~16.7m | GLM invalid eval score |
| `per-model` | `degraded` | `partial` | 75 | ~43.5m | 3 Kimi 429 overloads; 1 Kimi invalid eval score |

Failure details:

- Baseline round 2: `pi/zai/glm-5.1:off` returned an invalid brainstorm evaluation score while evaluating `pi/openai-codex/gpt-5.4:off`.
- Expanded round 1: `pi/kimi-coding/kimi-for-coding:off` returned three provider errors: `429 {"error":{"type":"rate_limit_error","message":"The engine is currently overloaded, please try again later"}}`.
- Expanded round 2: `pi/kimi-coding/kimi-for-coding:off` returned an invalid brainstorm evaluation score while evaluating `pi/openai-codex_gpt-5.4:off+variant-1`.

## Production-Selector Metrics

`controversy_floor_7` view:

| Prompt variants | Mean quality | Min quality | Disagreement | Lexical overlap | Meta preamble rate |
|---|---:|---:|---:|---:|---:|
| `off` | 7.67 | 7.00 | 0.33 | 0.088 | 0.00 |
| `per-model` | 7.83 | 7.50 | 0.83 | 0.081 | 0.00 |

The expanded run again improved disagreement and slightly improved the selected panel quality floor, but degradation prevents drawing a default-change conclusion.

## Decision

Do not run more L3 prompts solely to validate parser hardening without better failure evidence. Invalid evaluation summaries still occur after `dc805a7`, and these artifacts do not contain the raw invalid response text from this run.

Follow-up implemented in the same session: bounded raw response preview capture was added to `BrainstormProviderFailure` for invalid structured-response parse failures and exposed in CLI JSON plus `provider-failures.json`.

Next step: rerun a small live validation so any remaining invalid-score failure includes enough evidence to distinguish malformed/empty/provider output from genuinely unhandled score JSON.
