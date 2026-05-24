# Refinery Verbs

Refinery verbs are different orchestration strategies over the same basic substrate: multiple model providers, prompt construction, proposal calls, evaluation calls, score aggregation, and artifact output.

Each verb is defined by three mostly independent choices:

1. **Iteration strategy** — what each model sees between rounds.
2. **Evaluation strategy** — what rubric/models use to judge outputs.
3. **Selection strategy** — how Refinery chooses the returned answer(s).

## Shipped Verbs

| Verb | Purpose | Iteration | Evaluation | Selection | Output |
|---|---|---|---|---|---|
| `converge` | Find a single answer that models broadly agree is strong. | Own prior answer + peer reviews. | Correctness/quality scoring. | Vote threshold with stable leader. | Single winner or no consensus. |
| `synthesize` | Merge the best converged answers into one stronger answer. | Converge phase, then custom synthesis phase. | Synthesis rubric: integration, coherence, completeness, fidelity. | Highest-scoring synthesis. | Single synthesized answer. |
| `brainstorm` | Explore a solution space and return a diverse panel. | Score-only: each model sees only its own prior answers + scores. | Brainstorm rubric: originality, insight, depth, feasibility. | Controversial: high quality + evaluator disagreement. | Panel of answers. |

## `converge`

Use `converge` when you want one reliable answer and consensus is desirable.

```sh
refinery converge "What are the key trade-offs in this design?" \
  --models codex-cli,opencode/kimi-for-coding/kimi-k2-thinking
```

Mechanics:

1. Each model proposes an answer.
2. Models evaluate each other's answers.
3. Each model refines using its own answer plus peer feedback.
4. The loop stops when a model's mean score crosses the threshold for the configured stability rounds.

Good for:

- factual questions,
- architecture decisions where agreement matters,
- code/design review synthesis where correctness matters more than diversity.

## `synthesize`

Use `synthesize` when the best answer may require combining parts of several model outputs.

```sh
refinery synthesize "Design an auth architecture for this app" \
  --models codex-cli,opencode/zai-coding-plan/glm-5.1 \
  --converge-rounds 2
```

Mechanics:

1. Run a bounded converge phase.
2. Keep qualifying answers above the synthesis threshold.
3. Ask models to synthesize those answers.
4. Evaluate syntheses and return the highest-scoring one.

Good for:

- design docs,
- implementation plans,
- combining complementary model strengths into one coherent result.

## `brainstorm`

Use `brainstorm` when you want breadth: multiple distinct, useful answers rather than a single consensus answer.

```sh
refinery brainstorm \
  "Generate unconventional but practical product ideas for a privacy-first team memory assistant" \
  --models codex-cli,opencode/zai-coding-plan/glm-5.1,opencode/kimi-for-coding/kimi-k2-thinking,opencode/minimax-coding-plan/MiniMax-M2.5 \
  --max-rounds 2 \
  --panel-size 3 \
  --max-concurrent 1
```

Mechanics:

1. Each model proposes independently.
2. Models evaluate each other's proposals on originality, insight, depth, and feasibility.
3. Next round, each model sees only its own prior answers and aggregate scores — no peer text or rationales.
4. Refinery selects a final panel using a diversity-oriented selector.

The current production selector is raw controversy:

```text
controversy_score = mean_score * stddev(per_evaluator_scores)
```

This favors answers that are both reasonably strong and divisive among evaluators.

### Brainstorm benchmark results

A six-prompt v0 benchmark was run with Codex, GLM, Kimi, and MiniMax. Each successful run used 4 models, 2 rounds, and 32 calls. The prompts covered product strategy, technical design, architecture, debugging/process, research/science, and governance.

Full report: [`docs/brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md`](../brainstorms/2026-05-23-six-prompt-brainstorm-benchmark.md)

Aggregate selector results:

| Selector | Mean quality | Min quality | Disagreement | Lexical overlap | Takeaway |
|---|---:|---:|---:|---:|---|
| `mean` | 8.093 | 7.778 | 0.288 | 0.130 | Highest quality, least diversity pressure. |
| `controversy` | 7.685 | 6.833 | 0.551 | 0.100 | More diverse, but can select low-quality divisive answers. |
| `controversy_floor_7` | 7.963 | 7.500 | 0.386 | 0.118 | Best immediate improvement candidate. |
| `quality_x_lexdiv` | 8.056 | 7.667 | 0.340 | 0.121 | Good cheap heuristic; not clearly better yet. |

Key findings:

- Raw controversy reduces lexical overlap, so it does increase diversity.
- Raw controversy can over-reward disagreement. In one debugging prompt it selected a `mean=5.67` answer first because evaluator disagreement was high.
- Adding a quality floor (`mean_score >= 7`) preserved some diversity while improving the panel quality floor.
- About one third of selected answers contained score-history meta-preambles like "Based on my Round 1 score..."; this needs prompt polish.

Current follow-ups:

- `todos/023-brainstorm-quality-floor-selection.md` — add/configure quality-floor selection.
- `todos/024-brainstorm-suppress-score-history-meta-preambles.md` — suppress score-history meta-commentary in final answers.
- `todos/013-brainstorm-strategy-benchmarks.md` — continue with iteration-strategy benchmarks after quality-floor/prompt-polish decisions.

### Benchmarking brainstorm artifacts

Saved brainstorm runs can be analyzed without new provider calls:

```sh
refinery benchmark-brainstorm path/to/brainstorm-run --output-format json
```

The analyzer compares selector counterfactuals over final-round proposals and evaluations:

- `mean`
- `stddev`
- `controversy`
- `controversy_floor_7`
- `quality_x_lexdiv`

It reports panel quality, quality floor, evaluator disagreement, lexical overlap, and meta-preamble rate.

## Planned Verbs

| Verb | Status | Idea |
|---|---|---|
| `evolve` | Designed, not implemented. | Darwinian blind variation with score-only pressure; cull low performers and restart lineages. |

## Provider Notes for Verb Benchmarks

When running multiple OpenCode-backed models (`opencode/...`) in the same panel, use serial execution for now:

```sh
--max-concurrent 1
```

Parallel OpenCode subprocesses have been observed to fail with SQLite/WAL startup errors (`PRAGMA journal_mode = WAL`). See `todos/022-opencode-concurrency-sqlite-wal.md`.

For longer brainstorm prompts, prefer a larger idle timeout:

```sh
--idle-timeout 480
```
