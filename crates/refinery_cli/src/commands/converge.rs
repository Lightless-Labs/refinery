use std::io::IsTerminal as _;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use tracing::info;

use refinery_core::EngineConfig;
use refinery_core::types::ConvergenceStatus;

use super::common::{
    AnswerOutput, ErrorResponse, JsonOutput, MetadataOutput, OutputFormat, SharedArgs,
    WinnerOutput, build_providers, converge_error_to_detail, init_tracing, make_run_dir,
    resolve_prompt, save_round_artifacts,
};

#[derive(Parser, Debug)]
pub struct ConvergeArgs {
    #[command(flatten)]
    pub shared: SharedArgs,

    /// Score threshold for convergence [default: 8.0] (range: 1.0-10.0)
    #[arg(short, long, default_value = "8.0")]
    threshold: f64,

    /// Maximum rounds [default: 5] (range: 1-20)
    #[arg(short = 'r', long, default_value = "5")]
    max_rounds: u32,

    /// Consecutive rounds the same model must lead to converge [default: 2] (range: 1-20)
    #[arg(short = 's', long, default_value = "2", value_parser = clap::value_parser!(u32).range(1..=20))]
    stability_rounds: u32,
}

#[allow(clippy::too_many_lines)]
pub async fn run(args: ConvergeArgs) -> ExitCode {
    let shared = &args.shared;
    init_tracing(shared);

    // Validate args before any I/O
    if args.stability_rounds > args.max_rounds {
        eprintln!(
            "Error: --stability-rounds ({}) cannot exceed --max-rounds ({})",
            args.stability_rounds, args.max_rounds
        );
        return ExitCode::from(4);
    }

    // Dry run: estimate calls without resolving prompt or building providers
    if shared.dry_run {
        let model_ids: Vec<tundish_core::ModelId> = match shared
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
        if let Ok(config) = EngineConfig::new(
            model_ids,
            args.max_rounds,
            args.threshold,
            args.stability_rounds,
            Duration::from_secs(shared.timeout),
            shared.max_concurrent,
        ) {
            let estimate = refinery_core::Engine::estimate(&config);
            println!("Dry run estimate:");
            println!("  Models: {}", estimate.model_count);
            println!("  Calls per round: {}", estimate.calls_per_round);
            println!("  Max rounds: {}", estimate.max_rounds);
            println!("  Total calls (max): {}", estimate.total_calls);
            if estimate.model_count > 5 {
                eprintln!(
                    "Warning: N={} has quadratic cost scaling ({} calls/round)",
                    estimate.model_count, estimate.calls_per_round
                );
            }
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
        args.max_rounds,
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

    let consensus_progress: Option<refinery_core::ProgressFn> =
        if shared.verbose || shared.debug || !std::io::stderr().is_terminal() {
            None
        } else {
            Some(display.consensus_callback(model_ids.clone()))
        };

    let strategy = Box::new(refinery_core::VoteThreshold::new(
        args.threshold,
        args.stability_rounds,
    ));
    let engine =
        refinery_core::Engine::new(providers, strategy, config, consensus_progress.clone());

    info!("Starting consensus run with {} models", shared.models.len());

    let tick_handle = display.start_tick();
    let run_result = engine.run(&prompt).await;

    if let Some(handle) = tick_handle {
        handle.abort();
    }
    display.finish();

    match run_result {
        Ok((outcome, rounds)) => {
            if let Some(ref dir) = shared.output_dir {
                let run_dir = make_run_dir(dir, shared.prompt.as_deref());
                if let Err(e) = save_round_artifacts(&run_dir, &rounds) {
                    eprintln!("Warning: failed to save artifacts: {e}");
                }
            }

            match shared.output_format {
                OutputFormat::Json => {
                    let status_str = match serde_json::to_value(&outcome.status) {
                        Ok(serde_json::Value::String(s)) => s,
                        _ => format!("{:?}", outcome.status).to_lowercase(),
                    };
                    let json_output = JsonOutput {
                        status: status_str,
                        winner: outcome.winner.as_ref().map(|w| WinnerOutput {
                            model_id: w.to_string(),
                            answer: outcome.answer.clone().unwrap_or_default(),
                        }),
                        final_round: outcome.final_round,
                        strategy: "vote-threshold".to_string(),
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
                    match serde_json::to_string_pretty(&json_output) {
                        Ok(json) => println!("{json}"),
                        Err(e) => {
                            eprintln!("Failed to serialize output: {e}");
                            return ExitCode::from(1);
                        }
                    }
                }
                OutputFormat::Text => {
                    println!("Status: {:?}", outcome.status);
                    if let Some(ref winner) = outcome.winner {
                        println!("Winner: {winner}");
                    } else {
                        println!("Winner: none (no consensus)");
                    }
                    println!("Rounds: {}", outcome.final_round);
                    println!("Total calls: {}", outcome.total_calls);
                    println!("Elapsed: {:?}", outcome.elapsed);
                    if let Some(ref answer) = outcome.answer {
                        println!("\n--- Answer ---\n");
                        println!("{answer}");
                    } else {
                        println!("\n--- No consensus reached ---");
                        println!("All answers are included in the output.");
                    }
                }
            }

            match outcome.status {
                ConvergenceStatus::Converged | ConvergenceStatus::SingleModel => ExitCode::SUCCESS,
                ConvergenceStatus::MaxRoundsExceeded | ConvergenceStatus::NoQualifyingAnswers => {
                    ExitCode::from(2)
                }
                ConvergenceStatus::InsufficientModels => ExitCode::from(3),
                ConvergenceStatus::Cancelled | ConvergenceStatus::Synthesized => ExitCode::from(1),
            }
        }
        Err(e) => {
            match shared.output_format {
                OutputFormat::Json => {
                    let err_response = ErrorResponse {
                        status: "error".to_string(),
                        error: converge_error_to_detail(&e),
                    };
                    match serde_json::to_string_pretty(&err_response) {
                        Ok(json) => eprintln!("{json}"),
                        Err(ser_err) => eprintln!("Error: {e} (serialization failed: {ser_err})"),
                    }
                }
                OutputFormat::Text => {
                    eprintln!("Error: {e}");
                }
            }
            ExitCode::from(1)
        }
    }
}
