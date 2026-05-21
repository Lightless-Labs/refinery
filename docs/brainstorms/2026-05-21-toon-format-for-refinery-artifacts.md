---
date: 2026-05-21
topic: toon-format-for-refinery-artifacts
source: https://github.com/toon-format/toon
---

# TOON Format Notes for Refinery Artifacts

## What TOON Is

TOON (Token-Oriented Object Notation) is a compact, line-oriented encoding of the JSON data model for LLM contexts. It keeps deterministic JSON-like structure, but removes repeated object keys in uniform arrays by using array length and field headers.

Example from the spec:

```toon
users[2]{id,name}:
  1,Ada
  2,Linus
```

Key properties from `toon-format/toon` and `toon-format/spec`:

- Preserves the JSON data model: objects, arrays, primitives.
- Uses indentation instead of braces.
- Uses explicit array lengths: `[N]`.
- Uses field lists for uniform arrays of primitive-valued objects: `{field1,field2}`.
- Supports strict validation, including array length and row-width checks.
- Best fit is uniform arrays of objects; deeply nested or highly non-uniform data may be more compact as JSON compact.
- Intended primarily as an LLM prompt/context representation, not as a universal storage/API replacement.
- Current spec is a working draft (`v3.3` in `toon-format/spec` on 2026-05-21).

## Why It Matters for Refinery

Refinery produces and consumes structured, model-facing artifacts:

- rounds
- proposals
- evaluations
- panel candidates
- score histories
- future benchmark records
- possible prompt-reframing/domain-collision lineages

These are exactly the kind of repeated structured records that can become token-expensive when re-injected into later model prompts as JSON. TOON is worth considering as a **prompt-export format** for compactly showing models prior run data, benchmark fixtures, and score/evaluation tables.

## Fit for Brainstorm/Benchmark Artifacts

TOON appears especially useful for fields that are already structured and tabular:

```toon
participants[3]{id,provider,model,status}:
  m1,claude-code,claude-opus-4-6,failed
  m2,codex-cli,gpt-5.4,ok
  m3,gemini-cli,gemini-3.1-pro-preview,quota_exhausted

judgments[4]{round,evaluator,evaluatee,originality,insight,depth,feasibility,score}:
  1,m1,m2,8,7,7,6,7
  1,m1,m3,6,8,6,7,7
  1,m2,m1,9,7,8,5,7
  1,m3,m1,5,9,8,6,7
```

Potential uses:

1. **LLM-facing summaries of run state**
   - Score histories, provider statuses, and benchmark metrics can be serialized as TOON before being placed in prompts.

2. **Benchmark corpora for strategy comparisons**
   - Candidate lineages, extracted idea atoms, and rubric scores are repeated records; TOON could reduce prompt tokens for evaluator/reviewer models.

3. **Artifact sidecars**
   - Keep canonical `*.json`/`*.jsonl` for machine storage and add `*.toon` sidecars optimized for model consumption.

4. **Future `--output-format toon`**
   - Possibly useful for text-like CLI output intended to be pasted into another LLM. This should not replace JSON for automation unless TOON parser support is a project dependency.

## Caveats

- Long free-form answers are not TOON's strongest shape. TOON has line-oriented strings; embedded newlines are escaped as `\n`, so raw multi-paragraph model answers may be less readable than Markdown or JSON strings.
- TOON shines when answers are transformed into short abstracts, idea atoms, typed tags, scores, and other uniform records.
- The Rust crate exists (`toon-format`), but its default feature set includes CLI/TUI dependencies. If Refinery adopts it as a library dependency, prefer something like:

  ```toml
  toon-format = { version = "0.4", default-features = false }
  ```

  Validate spec-version alignment before use: the main spec observed was `v3.3`, while the Rust README currently advertises spec `v3.0`.

- TOON should be benchmarked against compact JSON for Refinery's actual artifacts. Its own docs note compact JSON can beat TOON for deeply nested or non-uniform structures.

## Recommendation

Do not change canonical artifact storage yet. Keep JSON/JSONL as the durable, tooling-friendly source of truth.

Add TOON to the shortlist for:

- prompt-facing artifact exports,
- benchmark fixtures passed into evaluator models,
- compact summaries of round/evaluation/provider status,
- possible future `--output-format toon` or `refinery export --format toon`.

The best first experiment is to take a real completed multi-provider brainstorm artifact and compare:

1. pretty JSON,
2. compact JSON,
3. TOON,
4. Markdown table summaries,

on token count, readability, parsing reliability, and model retrieval accuracy.
