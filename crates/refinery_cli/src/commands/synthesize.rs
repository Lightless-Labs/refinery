use std::collections::HashMap;
use std::io::IsTerminal as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tokio::sync::Semaphore;
use tracing::info;

use refinery_core::EngineConfig;
use refinery_core::prompts;
use refinery_core::types::{Message, ModelId, RoundOutcome};

use super::common::{
    AnswerOutput, ErrorResponse, JsonOutput, MetadataOutput, OutputFormat, SharedArgs,
    WinnerOutput, build_providers, converge_error_to_detail, init_tracing, make_run_dir,
    resolve_prompt, save_round_artifacts,
};

#[derive(Parser, Debug)]
pub struct SynthesizeArgs {
    #[command(flatten)]
    pub shared: SharedArgs,

    /// Score threshold for convergence during converge rounds [default: 8.0]
    #[arg(short, long, default_value = "8.0")]
    threshold: f64,

    /// Number of converge rounds to run before synthesis [default: 2]
    #[arg(long, default_value = "2")]
    converge_rounds: u32,

    /// Minimum score for an answer to be included in synthesis [default: uses --threshold]
    #[arg(long)]
    synthesis_threshold: Option<f64>,

    /// Consecutive rounds the same model must lead during converge [default: 2]
    #[arg(short = 's', long, default_value = "2", value_parser = clap::value_parser!(u32).range(1..=20))]
    stability_rounds: u32,
}

#[allow(clippy::too_many_lines)]
pub async fn run(args: SynthesizeArgs) -> ExitCode {
    let shared = &args.shared;
    init_tracing(shared);

    // Validate args before any I/O
    if args.stability_rounds > args.converge_rounds {
        eprintln!(
            "Error: --stability-rounds ({}) cannot exceed --converge-rounds ({})",
            args.stability_rounds, args.converge_rounds
        );
        return ExitCode::from(4);
    }

    let synthesis_threshold = args.synthesis_threshold.unwrap_or(args.threshold);

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
        if let Ok(config) = EngineConfig::new(
            model_ids,
            args.converge_rounds,
            args.threshold,
            args.stability_rounds,
            Duration::from_secs(shared.timeout),
            shared.max_concurrent,
        ) {
            let estimate = refinery_core::Engine::estimate(&config);
            #[allow(clippy::cast_possible_truncation)]
            let synthesis_calls = if n > 1 { (n + n * (n - 1)) as u32 } else { 1 };
            println!("Dry run estimate:");
            println!("  Models: {}", estimate.model_count);
            println!("  Converge rounds: {}", args.converge_rounds);
            println!("  Converge calls: {}", estimate.total_calls);
            println!("  Synthesis calls: {synthesis_calls}");
            println!(
                "  Total calls (max): {}",
                estimate.total_calls + synthesis_calls
            );
        }
        return ExitCode::SUCCESS;
    }

    let prompt = match resolve_prompt(shared) {
        Ok(p) => p,
        Err(code) => return code,
    };

    let (model_ids, providers, display) = match build_providers(shared).await {
        Ok(r) => r,
        Err(code) => return code,
    };

    let config = match EngineConfig::new(
        model_ids.clone(),
        args.converge_rounds,
        args.threshold,
        args.stability_rounds,
        Duration::from_secs(shared.timeout),
        shared.max_concurrent,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config error: {e}");
            return ExitCode::from(4);
        }
    };

    // ── Phase 1: Converge rounds ────────────────────────────────────────

    let hidden = shared.verbose || shared.debug || !std::io::stderr().is_terminal();
    let consensus_progress: Option<refinery_core::ProgressFn> = if hidden {
        None
    } else {
        Some(display.consensus_callback(model_ids.clone()))
    };

    let strategy = Box::new(refinery_core::VoteThreshold::new(
        args.threshold,
        args.stability_rounds,
    ));
    let engine = refinery_core::Engine::new(
        providers.clone(),
        strategy,
        config,
        consensus_progress.clone(),
    );

    info!(
        "Starting synthesize: {} converge rounds, then synthesis",
        args.converge_rounds
    );

    let tick_handle = display.start_tick();
    let converge_result = engine.run(&prompt).await;

    let (outcome, rounds) = match converge_result {
        Ok(r) => r,
        Err(e) => {
            if let Some(handle) = tick_handle {
                handle.abort();
            }
            display.finish();
            match shared.output_format {
                OutputFormat::Json => {
                    let err_response = ErrorResponse {
                        status: "error".to_string(),
                        error: converge_error_to_detail(&e),
                    };
                    if let Ok(json) = serde_json::to_string_pretty(&err_response) {
                        eprintln!("{json}");
                    }
                }
                OutputFormat::Text => eprintln!("Error during converge phase: {e}"),
            }
            return ExitCode::from(1);
        }
    };

    // ── Phase 2: Filter qualifying answers ──────────────────────────────

    let qualifying: Vec<_> = outcome
        .all_answers
        .iter()
        .filter(|a| a.mean_score >= synthesis_threshold)
        .collect();

    if qualifying.is_empty() {
        if let Some(handle) = tick_handle {
            handle.abort();
        }
        display.finish();
        eprintln!(
            "No answers scored above synthesis threshold ({synthesis_threshold:.1}). \
             Skipping synthesis."
        );
        match shared.output_format {
            OutputFormat::Json => {
                let json_output = JsonOutput {
                    status: "no_qualifying_answers".to_string(),
                    winner: None,
                    final_round: outcome.final_round,
                    strategy: "synthesize".to_string(),
                    all_answers: outcome
                        .all_answers
                        .iter()
                        .map(|a| AnswerOutput {
                            model_id: a.model_id.to_string(),
                            answer: a.answer.clone(),
                            mean_score: a.mean_score,
                        })
                        .collect(),
                    metadata: MetadataOutput {
                        total_rounds: outcome.final_round,
                        total_calls: outcome.total_calls,
                        elapsed_ms: outcome.elapsed.as_millis(),
                        models_dropped: vec![],
                    },
                };
                if let Ok(json) = serde_json::to_string_pretty(&json_output) {
                    println!("{json}");
                }
            }
            OutputFormat::Text => {
                println!("Status: NoQualifyingAnswers");
                println!("No answers met the synthesis threshold ({synthesis_threshold:.1}).");
            }
        }
        return ExitCode::from(2);
    }

    info!(
        "{}/{} answers qualify for synthesis (threshold {synthesis_threshold:.1})",
        qualifying.len(),
        outcome.all_answers.len()
    );

    // ── Phase 3: Synthesis ──────────────────────────────────────────────

    // Build synthesis prompt with qualifying answers (anonymized)
    let nonce = prompts::generate_nonce();
    let labels = prompts::shuffled_labels(qualifying.len());
    let labeled_answers: Vec<(String, &str)> = labels
        .iter()
        .zip(qualifying.iter())
        .map(|(label, a)| (label.clone(), a.answer.as_str()))
        .collect();

    let synthesis_prompt =
        prompts::synthesize::synthesize_prompt(&prompt, &labeled_answers, &nonce);
    let synthesis_system = prompts::system_prompt();

    let synthesis_messages = vec![
        Message::system(synthesis_system),
        Message::user(synthesis_prompt),
    ];

    // All models propose a synthesis
    let permits = if shared.max_concurrent == 0 {
        providers.len().max(1)
    } else {
        shared.max_concurrent
    };
    let semaphore = Arc::new(Semaphore::new(permits));

    eprintln!("\n  ── synthesize ──");

    let timeout = Duration::from_secs(shared.timeout);
    let mut synthesis_proposals: HashMap<ModelId, String> = HashMap::new();

    let mut handles = tokio::task::JoinSet::new();
    for provider in &providers {
        let sem = semaphore.clone();
        let p = provider.clone();
        let msgs = synthesis_messages.clone();
        let model_id = provider.model_id().clone();

        handles.spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");
            let result = tokio::time::timeout(
                timeout,
                p.send_message(&msgs, Some(prompts::synthesize::SYNTHESIS_SCHEMA)),
            )
            .await;
            (model_id, result)
        });
    }

    while let Some(result) = handles.join_next().await {
        match result {
            Ok((model_id, Ok(Ok(response)))) => {
                // Extract synthesis from structured output
                let synthesis = serde_json::from_str::<serde_json::Value>(&response)
                    .ok()
                    .and_then(|v| {
                        v.get("synthesis")
                            .and_then(|s| s.as_str())
                            .map(String::from)
                    })
                    .unwrap_or(response);
                let preview = refinery_core::progress::preview(&synthesis, 60);
                eprintln!("    \x1b[32m✓\x1b[0m {model_id} synthesized — \"{preview}\"");
                synthesis_proposals.insert(model_id, synthesis);
            }
            Ok((model_id, Ok(Err(e)))) => {
                eprintln!("    \x1b[31m✗\x1b[0m {model_id} synthesis failed — {e}");
            }
            Ok((model_id, Err(_))) => {
                eprintln!("    \x1b[31m✗\x1b[0m {model_id} synthesis timed out");
            }
            Err(e) => {
                eprintln!("    \x1b[31m✗\x1b[0m task panicked: {e}");
            }
        }
    }

    if synthesis_proposals.is_empty() {
        if let Some(handle) = tick_handle {
            handle.abort();
        }
        display.finish();
        eprintln!("All models failed to produce syntheses.");
        return ExitCode::from(1);
    }

    // ── Phase 4: Evaluate syntheses ─────────────────────────────────────

    // Single synthesis: skip evaluation, return it directly
    if synthesis_proposals.len() == 1 {
        if let Some(handle) = tick_handle {
            handle.abort();
        }
        display.finish();

        let (winner_id, winner_answer) = synthesis_proposals.into_iter().next().unwrap();
        return emit_synthesis_result(
            shared,
            &winner_id,
            &winner_answer,
            &[(winner_id.clone(), winner_answer.clone(), 0.0)],
            outcome.final_round,
            outcome.total_calls + 1,
            outcome.elapsed,
            &rounds,
        );
    }

    eprintln!("  ── evaluate syntheses ──");

    // Use synthesis-specific rubric: build custom eval prompts per pair
    let synthesis_nonce = prompts::generate_nonce();
    let synth_labels = prompts::shuffled_labels(synthesis_proposals.len());
    let synth_ids: Vec<ModelId> = synthesis_proposals.keys().cloned().collect();
    let label_map: HashMap<ModelId, String> = synth_ids
        .iter()
        .zip(synth_labels.iter())
        .map(|(id, label)| (id.clone(), label.clone()))
        .collect();

    let mut eval_handles = tokio::task::JoinSet::new();

    for evaluator_provider in &providers {
        let evaluator_id = evaluator_provider.model_id().clone();
        if !synthesis_proposals.contains_key(&evaluator_id) {
            continue;
        }

        for (evaluatee_id, synthesis_text) in &synthesis_proposals {
            if *evaluatee_id == evaluator_id {
                continue;
            }

            let eval_label = label_map.get(evaluatee_id).cloned().unwrap_or_default();
            let eval_prompt_text = prompts::synthesize::synthesize_evaluate_prompt(
                &prompt,
                synthesis_text,
                &eval_label,
                &synthesis_nonce,
                qualifying.len(),
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
                    provider
                        .send_message(&messages, Some(prompts::synthesize::SYNTHESIS_EVAL_SCHEMA)),
                )
                .await;
                (evaluator, evaluatee, result)
            });
        }
    }

    // Collect synthesis evaluation scores
    let mut synthesis_scores: HashMap<ModelId, Vec<f64>> = HashMap::new();
    let mut eval_count: u32 = 0;

    #[allow(clippy::similar_names)]
    while let Some(result) = eval_handles.join_next().await {
        match result {
            Ok((from, to, Ok(Ok(response)))) => {
                eval_count += 1;
                let parsed: serde_json::Value = serde_json::from_str(&response).unwrap_or_default();
                let score = parsed
                    .get("score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let rationale = parsed
                    .get("rationale")
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                let preview = refinery_core::progress::preview(rationale, 60);
                eprintln!("    \x1b[32m✓\x1b[0m {from} → {to}: {score:.1} — \"{preview}\"");
                synthesis_scores.entry(to).or_default().push(score);
            }
            Ok((from, to, Ok(Err(e)))) => {
                eval_count += 1;
                eprintln!("    \x1b[31m✗\x1b[0m {from} → {to} eval failed — {e}");
            }
            Ok((from, to, Err(_))) => {
                eval_count += 1;
                eprintln!("    \x1b[31m✗\x1b[0m {from} → {to} eval timed out");
            }
            Err(e) => {
                eprintln!("    \x1b[31m✗\x1b[0m eval task panicked: {e}");
            }
        }
    }

    // ── Phase 5: Pick best synthesis ────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    let mean_scores: HashMap<ModelId, f64> = synthesis_scores
        .iter()
        .map(|(model, scores)| {
            (
                model.clone(),
                scores.iter().sum::<f64>() / scores.len() as f64,
            )
        })
        .collect();

    let best = mean_scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));

    if let Some(handle) = tick_handle {
        handle.abort();
    }
    display.finish();

    #[allow(clippy::cast_possible_truncation)]
    let total_calls = outcome.total_calls + synthesis_proposals.len() as u32 + eval_count;

    if let Some((winner_id, best_score)) = best {
        let winner_answer = synthesis_proposals
            .get(winner_id)
            .cloned()
            .unwrap_or_default();

        eprintln!("  \x1b[32m→ Best synthesis:\x1b[0m {winner_id} ({best_score:.1})");

        // Build all_answers for output
        let all_synthesis_answers: Vec<AnswerOutput> = synthesis_proposals
            .iter()
            .map(|(model_id, answer)| {
                let score = mean_scores.get(model_id).copied().unwrap_or(0.0);
                AnswerOutput {
                    model_id: model_id.to_string(),
                    answer: answer.clone(),
                    mean_score: score,
                }
            })
            .collect();

        match shared.output_format {
            OutputFormat::Json => {
                let json_output = JsonOutput {
                    status: "synthesized".to_string(),
                    winner: Some(WinnerOutput {
                        model_id: winner_id.to_string(),
                        answer: winner_answer,
                    }),
                    final_round: outcome.final_round,
                    strategy: "synthesize".to_string(),
                    all_answers: all_synthesis_answers,
                    metadata: MetadataOutput {
                        total_rounds: outcome.final_round,
                        total_calls,
                        elapsed_ms: outcome.elapsed.as_millis(),
                        models_dropped: vec![],
                    },
                };
                if let Ok(json) = serde_json::to_string_pretty(&json_output) {
                    println!("{json}");
                }
            }
            OutputFormat::Text => {
                println!("Status: Synthesized");
                println!("Winner: {winner_id}");
                println!("Converge rounds: {}", outcome.final_round);
                println!("Total calls: {total_calls}");
                println!("Elapsed: {:?}", outcome.elapsed);
                println!("\n--- Synthesis ---\n");
                println!("{winner_answer}");
            }
        }

        // Save artifacts
        if let Some(ref dir) = shared.output_dir {
            let run_dir = make_run_dir(dir, shared.prompt.as_deref());
            let _ = save_round_artifacts(&run_dir, &rounds);
        }

        ExitCode::SUCCESS
    } else {
        eprintln!("No synthesis evaluations completed.");
        ExitCode::from(1)
    }
}

/// Emit synthesis result (used for both single-model and multi-model paths).
#[allow(clippy::too_many_arguments)]
fn emit_synthesis_result(
    shared: &SharedArgs,
    winner_id: &ModelId,
    winner_answer: &str,
    all: &[(ModelId, String, f64)], // (model_id, answer, mean_score)
    final_round: u32,
    total_calls: u32,
    elapsed: Duration,
    rounds: &[RoundOutcome],
) -> ExitCode {
    eprintln!("  \x1b[32m→ Best synthesis:\x1b[0m {winner_id}");

    let all_synthesis_answers: Vec<AnswerOutput> = all
        .iter()
        .map(|(m, a, s)| AnswerOutput {
            model_id: m.to_string(),
            answer: a.clone(),
            mean_score: *s,
        })
        .collect();

    match shared.output_format {
        OutputFormat::Json => {
            let json_output = JsonOutput {
                status: "synthesized".to_string(),
                winner: Some(WinnerOutput {
                    model_id: winner_id.to_string(),
                    answer: winner_answer.to_string(),
                }),
                final_round,
                strategy: "synthesize".to_string(),
                all_answers: all_synthesis_answers,
                metadata: MetadataOutput {
                    total_rounds: final_round,
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
            println!("Status: Synthesized");
            println!("Winner: {winner_id}");
            println!("Total calls: {total_calls}");
            println!("Elapsed: {elapsed:?}");
            println!("\n--- Synthesis ---\n");
            println!("{winner_answer}");
        }
    }

    if let Some(ref dir) = shared.output_dir {
        let run_dir = make_run_dir(dir, shared.prompt.as_deref());
        let _ = save_round_artifacts(&run_dir, rounds);
    }

    ExitCode::SUCCESS
}
