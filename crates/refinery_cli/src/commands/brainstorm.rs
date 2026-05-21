use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use serde::Serialize;

use refinery_core::brainstorm::{
    BrainstormConfig, BrainstormError, BrainstormProviderFailure, BrainstormResult,
};
use refinery_core::types::ModelId;

use super::common::{
    DryRunOutput, MetadataOutput, OutputFormat, SharedArgs, build_providers, emit_dry_run_json,
    init_tracing, make_run_dir, resolve_prompt,
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
    degraded: bool,
    evaluation_status: String,
    panel: Vec<PanelAnswerOutput>,
    provider_failures: Vec<ProviderFailureOutput>,
    metadata: MetadataOutput,
}

#[derive(Serialize)]
struct PanelAnswerOutput {
    model_id: String,
    answer: String,
    evaluated: bool,
    mean_score: Option<f64>,
    stddev: Option<f64>,
    controversy_score: Option<f64>,
    per_evaluator_scores: Vec<EvaluatorScore>,
}

#[derive(Serialize)]
struct EvaluatorScore {
    evaluator: String,
    score: f64,
}

#[derive(Serialize)]
struct ProviderFailureOutput {
    round: u32,
    phase: String,
    model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_model_id: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct BrainstormErrorJsonOutput {
    status: String,
    error: super::common::ErrorDetail,
    provider_failures: Vec<ProviderFailureOutput>,
    metadata: BrainstormErrorMetadata,
}

#[derive(Serialize)]
struct BrainstormErrorMetadata {
    total_rounds: u32,
    total_calls: u32,
    models_dropped: Vec<String>,
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
        if n == 0 {
            eprintln!("Error: at least one model must be specified with --models");
            return ExitCode::from(4);
        }
        #[allow(clippy::cast_possible_truncation)]
        let calls_per_round = (n + n * (n - 1)) as u32;
        let total = calls_per_round * args.max_rounds;
        if matches!(shared.output_format, OutputFormat::Json) {
            return emit_dry_run_json(&DryRunOutput {
                status: "dry_run".to_string(),
                verb: "brainstorm".to_string(),
                models: n,
                max_rounds: Some(args.max_rounds),
                converge_rounds: None,
                calls_per_round: Some(calls_per_round),
                converge_calls: None,
                synthesis_calls: None,
                total_calls: total,
                panel_size: Some(args.panel_size),
                warning: None,
            });
        }
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

    let output_dir = shared
        .output_dir
        .as_ref()
        .map(|dir| make_run_dir(dir, shared.prompt.as_deref()));

    let config = BrainstormConfig {
        max_rounds: args.max_rounds,
        panel_size: args.panel_size as usize,
        max_concurrent: shared.max_concurrent,
        timeout: Duration::from_secs(shared.timeout),
        output_dir,
    };

    let result = refinery_core::brainstorm::run(&providers, &prompt, &config).await;

    if let Some(handle) = tick_handle {
        handle.abort();
    }
    display.finish();

    match result {
        Ok(br) => emit_success(shared, &br, start_time.elapsed()),
        Err(e) => emit_brainstorm_error(shared, &e),
    }
}

fn emit_success(
    shared: &SharedArgs,
    result: &BrainstormResult,
    elapsed: std::time::Duration,
) -> ExitCode {
    if result.panel.is_empty() {
        return emit_empty_panel_error(shared, result);
    }

    match shared.output_format {
        OutputFormat::Json => emit_json_success(result, elapsed),
        OutputFormat::Text => {
            emit_text_success(result, elapsed);
            ExitCode::SUCCESS
        }
    }
}

fn emit_json_success(result: &BrainstormResult, elapsed: std::time::Duration) -> ExitCode {
    let json_output = BrainstormJsonOutput {
        status: if result.degraded {
            "degraded".to_string()
        } else {
            "brainstormed".to_string()
        },
        degraded: result.degraded,
        evaluation_status: result.evaluation_status.as_str().to_string(),
        panel: result
            .panel
            .iter()
            .map(|c| {
                let evaluated = !c.per_evaluator_scores.is_empty();
                PanelAnswerOutput {
                    model_id: c.model_id.to_string(),
                    answer: c.answer.clone(),
                    evaluated,
                    mean_score: evaluated.then_some(c.mean_score),
                    stddev: evaluated.then_some(c.stddev),
                    controversy_score: evaluated.then_some(c.controversy_score),
                    per_evaluator_scores: c
                        .per_evaluator_scores
                        .iter()
                        .map(|(id, s)| EvaluatorScore {
                            evaluator: id.to_string(),
                            score: *s,
                        })
                        .collect(),
                }
            })
            .collect(),
        provider_failures: result
            .provider_failures
            .iter()
            .map(provider_failure_output)
            .collect(),
        metadata: MetadataOutput {
            total_rounds: result.rounds_completed,
            total_calls: result.total_calls,
            elapsed_ms: elapsed.as_millis(),
            models_dropped: result
                .failed_model_ids()
                .iter()
                .map(ToString::to_string)
                .collect(),
        },
    };
    match serde_json::to_string_pretty(&json_output) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("Failed to serialize brainstorm output: {e}");
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

fn emit_text_success(result: &BrainstormResult, elapsed: std::time::Duration) {
    if result.degraded {
        println!("Status: Degraded brainstorm");
    } else {
        println!("Status: Brainstormed");
    }
    println!("Rounds: {}", result.rounds_completed);
    println!("Total calls: {}", result.total_calls);
    println!("Evaluation status: {}", result.evaluation_status.as_str());
    println!("Elapsed: {elapsed:?}");
    if !result.provider_failures.is_empty() {
        println!("\n── Provider failures ──");
        for failure in &result.provider_failures {
            println!("{}", format_provider_failure(failure));
        }
    }
    println!("\n── Panel ({} answers) ──\n", result.panel.len());
    for (i, candidate) in result.panel.iter().enumerate() {
        if candidate.per_evaluator_scores.is_empty() {
            println!("#{} — {} (not evaluated)", i + 1, candidate.model_id);
        } else {
            println!(
                "#{} — {} (mean: {:.1}, stddev: {:.2}, controversy: {:.2})",
                i + 1,
                candidate.model_id,
                candidate.mean_score,
                candidate.stddev,
                candidate.controversy_score,
            );
        }
        println!("{}\n", candidate.answer);
    }
}

fn provider_failure_output(failure: &BrainstormProviderFailure) -> ProviderFailureOutput {
    ProviderFailureOutput {
        round: failure.round,
        phase: failure.phase.to_string(),
        model_id: failure.model_id.to_string(),
        target_model_id: failure.target_model_id.as_ref().map(ToString::to_string),
        message: failure.message.clone(),
    }
}

fn format_provider_failure(failure: &BrainstormProviderFailure) -> String {
    let target = failure
        .target_model_id
        .as_ref()
        .map(|target| format!(" -> {target}"))
        .unwrap_or_default();
    format!(
        "round {} {}: {}{}: {}",
        failure.round, failure.phase, failure.model_id, target, failure.message
    )
}

fn failed_model_ids_from_failures(failures: &[BrainstormProviderFailure]) -> Vec<String> {
    let mut ids: Vec<String> = failures
        .iter()
        .map(|failure| failure.model_id.to_string())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn emit_empty_panel_error(shared: &SharedArgs, result: &BrainstormResult) -> ExitCode {
    let message = "No answers survived to panel selection.";
    match shared.output_format {
        OutputFormat::Json => {
            let err = BrainstormErrorJsonOutput {
                status: "error".to_string(),
                error: super::common::ErrorDetail {
                    code: "brainstorm_failed".to_string(),
                    message: message.to_string(),
                    provider: None,
                    round: None,
                    phase: Some("brainstorm".to_string()),
                    retryable: true,
                },
                provider_failures: result
                    .provider_failures
                    .iter()
                    .map(provider_failure_output)
                    .collect(),
                metadata: BrainstormErrorMetadata {
                    total_rounds: result.rounds_completed,
                    total_calls: result.total_calls,
                    models_dropped: result
                        .failed_model_ids()
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                },
            };
            match serde_json::to_string_pretty(&err) {
                Ok(json) => eprintln!("{json}"),
                Err(e) => eprintln!("{message} (also failed to serialize JSON error: {e})"),
            }
        }
        OutputFormat::Text => {
            eprintln!("{message}");
            for failure in &result.provider_failures {
                eprintln!("{}", format_provider_failure(failure));
            }
        }
    }

    ExitCode::from(1)
}

fn emit_brainstorm_error(shared: &SharedArgs, error: &BrainstormError) -> ExitCode {
    match shared.output_format {
        OutputFormat::Json => {
            let err = BrainstormErrorJsonOutput {
                status: "error".to_string(),
                error: super::common::ErrorDetail {
                    code: "brainstorm_failed".to_string(),
                    message: error.to_string(),
                    provider: None,
                    round: Some(error.round),
                    phase: Some("brainstorm".to_string()),
                    retryable: true,
                },
                provider_failures: error
                    .provider_failures
                    .iter()
                    .map(provider_failure_output)
                    .collect(),
                metadata: BrainstormErrorMetadata {
                    total_rounds: error.rounds_completed,
                    total_calls: error.total_calls,
                    models_dropped: failed_model_ids_from_failures(&error.provider_failures),
                },
            };
            match serde_json::to_string_pretty(&err) {
                Ok(json) => eprintln!("{json}"),
                Err(e) => eprintln!("{error} (also failed to serialize JSON error: {e})"),
            }
        }
        OutputFormat::Text => {
            eprintln!("{error}");
            for failure in &error.provider_failures {
                eprintln!("{}", format_provider_failure(failure));
            }
        }
    }

    ExitCode::from(1)
}
