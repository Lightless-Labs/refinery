---
title: "research: evaluate TOON for artifact prompt export"
priority: low
milestone: post-v0.3
depends_on: 019-brainstorm-smoke-test-field-report
created: 2026-05-21
---

# Evaluate TOON for Artifact Prompt Export

## Problem

Refinery artifacts contain repeated structured records: rounds, proposals, evaluations, provider statuses, scores, panel candidates, and future benchmark lineages. When these are fed back into LLM prompts, JSON can be token-expensive.

TOON (Token-Oriented Object Notation) is a compact encoding of the JSON data model designed for LLM contexts. It may be a good prompt-facing export format for structured Refinery artifacts.

Research note: `docs/brainstorms/2026-05-21-toon-format-for-refinery-artifacts.md`.

## Candidate Uses

- Prompt-facing summaries of previous runs.
- Brainstorm benchmark fixtures.
- Score/evaluation/provider-status tables.
- Optional artifact sidecars (`*.toon`) alongside canonical JSON/JSONL.
- Future `--output-format toon` or `refinery export --format toon`.

## Constraints / Caveats

- Keep canonical storage as JSON/JSONL until proven otherwise.
- TOON is best for uniform arrays of primitive-valued records, not raw multi-paragraph answers.
- If using the Rust crate, avoid default CLI/TUI dependencies unless needed:

  ```toml
  toon-format = { version = "0.4", default-features = false }
  ```

- Validate spec-version alignment before adoption. The observed spec repo was v3.3, while the Rust README advertised v3.0.

## Acceptance Criteria

- Take one real multi-provider brainstorm artifact once provider availability is fixed.
- Compare pretty JSON, compact JSON, TOON, and Markdown tables for:
  - token count,
  - readability,
  - strict parse/validation behavior,
  - model retrieval accuracy on questions over the artifact.
- Document whether TOON should become:
  - unsupported,
  - an internal prompt serialization helper,
  - an artifact sidecar format,
  - or a public CLI output format.

## Origin

Follow-up from the 2026-05-21 brainstorm smoke test and user suggestion to investigate `https://github.com/toon-format/toon`.
