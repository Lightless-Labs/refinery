use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use serde::Serialize;

use refinery_core::brainstorm::{BrainstormConfig, BrainstormResult};
use refinery_core::types::ModelId;

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
    #[arg(long, default_value = "3", value_parser = clap::value_parser!(u32).range(1..=20))]
    panel_size: u32,
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
        let calls_per_round = (n + n * (n - 1)) as u32;
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
    let tick_handle = display.start_tick();

    let config = BrainstormConfig {
        max_rounds: args.max_rounds,
        panel_size: args.panel_size as usize,
        max_concurrent: shared.max_concurrent,
        timeout: Duration::from_secs(shared.timeout),
    };

    let result = refinery_core::brainstorm::run(&providers, &prompt, &config).await;

    if let Some(handle) = tick_handle {
        handle.abort();
    }
    display.finish();

    match result {
        Ok(br) => emit_success(shared, &br, args.max_rounds, start_time.elapsed()),
        Err(e) => {
            emit_error(shared, "brainstorm_failed", &e.to_string(), "brainstorm");
            ExitCode::from(1)
        }
    }
}

fn emit_success(
    shared: &SharedArgs,
    result: &BrainstormResult,
    max_rounds: u32,
    elapsed: std::time::Duration,
) -> ExitCode {
    if result.panel.is_empty() {
        emit_error(
            shared,
            "brainstorm_failed",
            "No answers survived to panel selection.",
            "brainstorm",
        );
        return ExitCode::from(1);
    }

    match shared.output_format {
        OutputFormat::Json => {
            let json_output = BrainstormJsonOutput {
                status: "brainstormed".to_string(),
                panel: result
                    .panel
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
                    total_rounds: max_rounds,
                    total_calls: result.total_calls,
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
            println!("Rounds: {max_rounds}");
            println!("Total calls: {}", result.total_calls);
            println!("Elapsed: {elapsed:?}");
            println!("\n── Panel ({} answers) ──\n", result.panel.len());
            for (i, candidate) in result.panel.iter().enumerate() {
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
