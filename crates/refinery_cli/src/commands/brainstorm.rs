use std::collections::HashMap;
use std::io::IsTerminal as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use serde::Serialize;
use tokio::sync::Semaphore;
use tracing::info;

use refinery_core::prompts;
use refinery_core::scoring::{self, PanelCandidate};
use refinery_core::types::{Message, ModelId, ScoreHistory};

use super::common::{
    ErrorResponse, MetadataOutput, OutputFormat, SharedArgs, build_providers, init_tracing,
    resolve_prompt,
};

#[derive(Parser, Debug)]
pub struct BrainstormArgs {
    #[command(flatten)]
    pub shared: SharedArgs,

    /// Maximum brainstorm rounds [default: 5]
    #[arg(short = 'r', long, default_value = "5", value_parser = clap::value_parser!(u32).range(1..=20))]
    max_rounds: u32,

    /// Number of diverse answers to return [default: 3]
    #[arg(long, default_value = "3")]
    panel_size: usize,
}

// ── JSON output types ───────────────────────────────────────────────────

#[derive(Serialize)]
struct BrainstormJsonOutput {
    status: String,
    panel: Vec<PanelAnswerOutput>,
    metadata: MetadataOutput,
}

#[derive(Serialize)]
struct PanelAnswerOutput {
    model_id: String,
    answer: String,
    mean_score: f64,
    stddev: f64,
    controversy_score: f64,
    per_evaluator_scores: Vec<EvaluatorScore>,
}

#[derive(Serialize)]
struct EvaluatorScore {
    evaluator: String,
    score: f64,
}

// ── Main entry point ────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
pub async fn run(args: BrainstormArgs) -> ExitCode {
    let shared = &args.shared;
    init_tracing(shared);

    // Dry run: estimate calls without resolving prompt or building providers
    if shared.dry_run {
        let model_ids: Vec<ModelId> = match shared
            .models
            .iter()
            .map(|m| super::common::parse_model_spec(m))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(ids) => ids,
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::from(4);
            }
        };
        let n = model_ids.len();
        #[allow(clippy::cast_possible_truncation)]
        let calls_per_round = (n + n * (n - 1)) as u32; // N propose + N*(N-1) evaluate
        let total = calls_per_round * args.max_rounds;
        println!("Dry run estimate:");
        println!("  Models: {n}");
        println!("  Max rounds: {}", args.max_rounds);
        println!("  Calls per round: {calls_per_round}");
        println!("  Total calls (max): {total}");
        println!("  Panel size: {}", args.panel_size);
        return ExitCode::SUCCESS;
    }

    let prompt = match resolve_prompt(shared) {
        Ok(p) => p,
        Err(code) => return code,
    };

    let (_model_ids, providers, display) = match build_providers(shared).await {
        Ok(r) => r,
        Err(code) => return code,
    };

    let start_time = std::time::Instant::now();

    // Set up concurrency
    let permits = if shared.max_concurrent == 0 {
        providers.len().pow(2).max(1)
    } else {
        shared.max_concurrent
    };
    let semaphore = Arc::new(Semaphore::new(permits));
    let timeout = Duration::from_secs(shared.timeout);

    let hidden = shared.verbose || shared.debug || !std::io::stderr().is_terminal();
    let tick_handle = display.start_tick();

    // Per-model state: score-only history + latest answer
    let mut score_histories: HashMap<ModelId, ScoreHistory> = HashMap::new();
    let mut latest_answers: HashMap<ModelId, String> = HashMap::new();
    // Per-model, per-evaluator scores from the last round (for panel selection)
    let mut last_round_eval_scores: HashMap<ModelId, Vec<(ModelId, f64)>> = HashMap::new();
    let mut total_calls: u32 = 0;

    // ── Round loop ──────────────────────────────────────────────────────

    for round in 1..=args.max_rounds {
        if !hidden {
            eprintln!("\n  ── brainstorm round {round}/{} ──", args.max_rounds);
        }

        // ── Propose phase ───────────────────────────────────────────────

        let mut propose_handles = tokio::task::JoinSet::new();

        for provider in &providers {
            let model_id = provider.model_id().clone();
            let sem = semaphore.clone();
            let p = provider.clone();

            // Build messages: system + user prompt with score history
            let user_content = if let Some(history) = score_histories.get(&model_id) {
                prompts::propose_with_score_history_prompt(&prompt, history)
            } else {
                prompt.clone()
            };

            let messages = vec![
                Message::system(prompts::brainstorm_system_prompt()),
                Message::user(user_content),
            ];

            propose_handles.spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                let result = tokio::time::timeout(
                    timeout,
                    p.send_message(&messages, Some(prompts::ANSWER_SCHEMA)),
                )
                .await;
                (model_id, result)
            });
        }

        let mut round_proposals: HashMap<ModelId, String> = HashMap::new();
        let mut propose_count: u32 = 0;

        while let Some(result) = propose_handles.join_next().await {
            match result {
                Ok((model_id, Ok(Ok(response)))) => {
                    propose_count += 1;
                    let answer = serde_json::from_str::<serde_json::Value>(&response)
                        .ok()
                        .and_then(|v| v.get("answer").and_then(|a| a.as_str()).map(String::from))
                        .unwrap_or(response);
                    if answer.trim().is_empty() {
                        if !hidden {
                            eprintln!(
                                "    \x1b[31m✗\x1b[0m {model_id} returned empty answer, skipping"
                            );
                        }
                        continue;
                    }
                    let preview = refinery_core::progress::preview(&answer, 60);
                    if !hidden {
                        eprintln!("    \x1b[32m✓\x1b[0m {model_id} proposed — \"{preview}\"");
                    }
                    round_proposals.insert(model_id, answer);
                }
                Ok((model_id, Ok(Err(e)))) => {
                    propose_count += 1;
                    if !hidden {
                        eprintln!("    \x1b[31m✗\x1b[0m {model_id} propose failed — {e}");
                    }
                }
                Ok((model_id, Err(_))) => {
                    propose_count += 1;
                    if !hidden {
                        eprintln!("    \x1b[31m✗\x1b[0m {model_id} propose timed out");
                    }
                }
                Err(e) => {
                    propose_count += 1;
                    if !hidden {
                        eprintln!("    \x1b[31m✗\x1b[0m propose task panicked: {e}");
                    }
                }
            }
        }

        total_calls += propose_count;

        if round_proposals.is_empty() {
            if let Some(handle) = tick_handle {
                handle.abort();
            }
            display.finish();
            emit_error(
                shared,
                "brainstorm_failed",
                &format!("All models failed to propose in round {round}."),
                "brainstorm",
            );
            return ExitCode::from(1);
        }

        // ── Evaluate phase ──────────────────────────────────────────────

        if !hidden {
            eprintln!("  ── evaluate ──");
        }

        let nonce = prompts::generate_nonce();
        let proposed_ids: Vec<ModelId> = round_proposals.keys().cloned().collect();
        let labels = prompts::shuffled_labels(proposed_ids.len());
        let label_map: HashMap<ModelId, String> = proposed_ids
            .iter()
            .zip(labels.iter())
            .map(|(id, label)| (id.clone(), label.clone()))
            .collect();

        let mut eval_handles = tokio::task::JoinSet::new();

        for evaluator_provider in &providers {
            let evaluator_id = evaluator_provider.model_id().clone();
            if !round_proposals.contains_key(&evaluator_id) {
                continue;
            }

            for (evaluatee_id, answer_text) in &round_proposals {
                if *evaluatee_id == evaluator_id {
                    continue; // no self-evaluation
                }

                let eval_label = label_map.get(evaluatee_id).cloned().unwrap_or_default();
                let eval_prompt_text = prompts::brainstorm::brainstorm_evaluate_prompt(
                    &prompt,
                    answer_text,
                    &eval_label,
                    &nonce,
                );

                let messages = vec![
                    Message::system(prompts::system_prompt()),
                    Message::user(eval_prompt_text),
                ];

                let sem = semaphore.clone();
                let provider = evaluator_provider.clone();
                let evaluator = evaluator_id.clone();
                let evaluatee = evaluatee_id.clone();

                eval_handles.spawn(async move {
                    let _permit = sem.acquire().await.expect("semaphore closed");
                    let result = tokio::time::timeout(
                        timeout,
                        provider.send_message(
                            &messages,
                            Some(prompts::brainstorm::BRAINSTORM_EVAL_SCHEMA),
                        ),
                    )
                    .await;
                    (evaluator, evaluatee, result)
                });
            }
        }

        // Collect per-evaluator scores for this round
        let mut round_scores: HashMap<ModelId, Vec<(ModelId, f64)>> = HashMap::new();
        let mut eval_count: u32 = 0;

        #[allow(clippy::similar_names)]
        while let Some(result) = eval_handles.join_next().await {
            match result {
                Ok((from, to, Ok(Ok(response)))) => {
                    eval_count += 1;
                    let parsed: serde_json::Value =
                        serde_json::from_str(&response).unwrap_or_default();
                    #[allow(clippy::cast_precision_loss)]
                    let score_val = parsed
                        .get("score")
                        .and_then(|v| v.as_u64().map(|u| u as f64).or_else(|| v.as_f64()))
                        .filter(|s| (1.0..=10.0).contains(s));
                    if let Some(score) = score_val {
                        let rationale = parsed
                            .get("rationale")
                            .and_then(|r| r.as_str())
                            .unwrap_or("");
                        let preview = refinery_core::progress::preview(rationale, 60);
                        if !hidden {
                            eprintln!(
                                "    \x1b[32m✓\x1b[0m {from} → {to}: {score:.1} — \"{preview}\""
                            );
                        }
                        round_scores.entry(to).or_default().push((from, score));
                    } else if !hidden {
                        eprintln!(
                            "    \x1b[31m✗\x1b[0m {from} → {to} eval returned invalid score, skipping"
                        );
                    }
                }
                Ok((from, to, Ok(Err(e)))) => {
                    eval_count += 1;
                    if !hidden {
                        eprintln!("    \x1b[31m✗\x1b[0m {from} → {to} eval failed — {e}");
                    }
                }
                Ok((from, to, Err(_))) => {
                    eval_count += 1;
                    if !hidden {
                        eprintln!("    \x1b[31m✗\x1b[0m {from} → {to} eval timed out");
                    }
                }
                Err(e) => {
                    eval_count += 1;
                    if !hidden {
                        eprintln!("    \x1b[31m✗\x1b[0m eval task panicked: {e}");
                    }
                }
            }
        }

        total_calls += eval_count;

        // Update score histories and latest answers
        for (model_id, answer) in &round_proposals {
            let scores: Vec<f64> = round_scores
                .get(model_id)
                .map(|s| s.iter().map(|(_, score)| *score).collect())
                .unwrap_or_default();
            let mean = scoring::mean(&scores);

            score_histories
                .entry(model_id.clone())
                .or_default()
                .push((answer.clone(), mean));

            latest_answers.insert(model_id.clone(), answer.clone());
        }

        last_round_eval_scores = round_scores;

        info!(
            round,
            max_rounds = args.max_rounds,
            proposals = round_proposals.len(),
            evaluations = eval_count,
            "brainstorm round complete"
        );
    }

    // ── Panel selection ─────────────────────────────────────────────────

    if let Some(handle) = tick_handle {
        handle.abort();
    }
    display.finish();

    // Build PanelCandidates from final-round answers
    let mut candidates: Vec<PanelCandidate> = latest_answers
        .iter()
        .map(|(model_id, answer)| {
            let per_evaluator: Vec<(ModelId, f64)> = last_round_eval_scores
                .get(model_id)
                .cloned()
                .unwrap_or_default();
            let scores: Vec<f64> = per_evaluator.iter().map(|(_, s)| *s).collect();
            let m = scoring::mean(&scores);
            let s = scoring::stddev(&scores, m);
            let c = scoring::controversy_score(&scores);

            PanelCandidate {
                model_id: model_id.clone(),
                answer: answer.clone(),
                mean_score: m,
                stddev: s,
                controversy_score: c,
                per_evaluator_scores: per_evaluator,
            }
        })
        .collect();

    let panel = scoring::select_panel(&mut candidates, args.panel_size);

    if panel.is_empty() {
        emit_error(
            shared,
            "brainstorm_failed",
            "No answers survived to panel selection.",
            "brainstorm",
        );
        return ExitCode::from(1);
    }

    let elapsed = start_time.elapsed();

    // ── Output ──────────────────────────────────────────────────────────

    match shared.output_format {
        OutputFormat::Json => {
            let json_output = BrainstormJsonOutput {
                status: "brainstormed".to_string(),
                panel: panel
                    .iter()
                    .map(|c| PanelAnswerOutput {
                        model_id: c.model_id.to_string(),
                        answer: c.answer.clone(),
                        mean_score: c.mean_score,
                        stddev: c.stddev,
                        controversy_score: c.controversy_score,
                        per_evaluator_scores: c
                            .per_evaluator_scores
                            .iter()
                            .map(|(id, s)| EvaluatorScore {
                                evaluator: id.to_string(),
                                score: *s,
                            })
                            .collect(),
                    })
                    .collect(),
                metadata: MetadataOutput {
                    total_rounds: args.max_rounds,
                    total_calls,
                    elapsed_ms: elapsed.as_millis(),
                    models_dropped: vec![],
                },
            };
            if let Ok(json) = serde_json::to_string_pretty(&json_output) {
                println!("{json}");
            }
        }
        OutputFormat::Text => {
            println!("Status: Brainstormed");
            println!("Rounds: {}", args.max_rounds);
            println!("Total calls: {total_calls}");
            println!("Elapsed: {elapsed:?}");
            println!(
                "\n── Panel ({}/{} answers) ──\n",
                panel.len(),
                latest_answers.len()
            );
            for (i, candidate) in panel.iter().enumerate() {
                println!(
                    "#{} — {} (mean: {:.1}, stddev: {:.2}, controversy: {:.2})",
                    i + 1,
                    candidate.model_id,
                    candidate.mean_score,
                    candidate.stddev,
                    candidate.controversy_score,
                );
                println!("{}\n", candidate.answer);
            }
        }
    }

    ExitCode::SUCCESS
}

fn emit_error(shared: &SharedArgs, code: &str, message: &str, phase: &str) {
    match shared.output_format {
        OutputFormat::Json => {
            let err = ErrorResponse {
                status: "error".to_string(),
                error: super::common::ErrorDetail {
                    code: code.to_string(),
                    message: message.to_string(),
                    provider: None,
                    round: None,
                    phase: Some(phase.to_string()),
                    retryable: true,
                },
            };
            if let Ok(json) = serde_json::to_string_pretty(&err) {
                eprintln!("{json}");
            }
        }
        OutputFormat::Text => {
            eprintln!("{message}");
        }
    }
}
