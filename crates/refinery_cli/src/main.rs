mod progress;

use std::io::{IsTerminal as _, Read as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::info;

use tundish_core::ModelId;

use refinery_core::types::{ConvergenceStatus, RoundOutcome};
use refinery_core::{EngineConfig, ModelProvider};

/// Multi-model AI consensus engine.
///
/// Dispatch prompts to multiple AI models and apply different strategies.
#[derive(Parser, Debug)]
#[command(name = "refinery", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Reach consensus across multiple models.
    ///
    /// Each model proposes an answer, evaluates others' answers, and iterates
    /// until a convergence threshold is met or max rounds are reached.
    Converge(ConvergeArgs),
}

/// Shared options for all subcommands (embedded via #[command(flatten)]).
#[derive(Parser, Debug)]
struct SharedArgs {
    /// The prompt (or - for stdin, max 1MB). Optional when --file is used.
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// File(s) to include in the prompt, tagged by filename (repeatable, 1MB total)
    #[arg(long = "file", short = 'f', value_name = "PATH")]
    files: Vec<PathBuf>,

    /// Comma-separated model list [e.g., claude-code,codex-cli/o3-pro,gemini-cli]
    #[arg(short, long, value_delimiter = ',')]
    models: Vec<String>,

    /// Hard wall-clock timeout per call in seconds [default: 1800]
    #[arg(long, default_value = "1800")]
    timeout: u64,

    /// Idle timeout: max seconds of silence before killing a subprocess [default: 120]
    #[arg(long, default_value = "120")]
    idle_timeout: u64,

    /// Max concurrent subprocess calls [default: 0 = unlimited]
    #[arg(long, default_value = "0")]
    max_concurrent: usize,

    /// Output format [text|json]
    #[arg(short, long, default_value = "text")]
    output_format: OutputFormat,

    /// Show per-round progress
    #[arg(short, long)]
    verbose: bool,

    /// Show raw CLI invocations and responses
    #[arg(long)]
    debug: bool,

    /// Tools to allow: `web_fetch`, `web_search`, `file_read`, `file_write`, `shell`.
    #[arg(long = "allow-tools", value_delimiter = ',')]
    allow_tools: Vec<String>,

    /// Directory to save per-round artifacts (proposals, evaluations)
    #[arg(long = "output-dir", value_name = "DIR")]
    output_dir: Option<PathBuf>,

    /// Show estimated call count and cost, then exit
    #[arg(long)]
    dry_run: bool,
}

#[derive(Parser, Debug)]
struct ConvergeArgs {
    #[command(flatten)]
    shared: SharedArgs,

    /// Score threshold for convergence [default: 8.0] (range: 1.0-10.0)
    #[arg(short, long, default_value = "8.0")]
    threshold: f64,

    /// Maximum rounds [default: 5] (range: 1-20)
    #[arg(short = 'r', long, default_value = "5")]
    max_rounds: u32,

    /// Consecutive rounds the same model must lead to converge [default: 2]
    #[arg(short = 's', long, default_value = "2")]
    stability_rounds: u32,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

/// JSON output schema for successful runs.
#[derive(Serialize)]
struct JsonOutput {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    winner: Option<WinnerOutput>,
    final_round: u32,
    strategy: String,
    all_answers: Vec<AnswerOutput>,
    metadata: MetadataOutput,
}

#[derive(Serialize)]
struct WinnerOutput {
    model_id: String,
    answer: String,
}

#[derive(Serialize)]
struct AnswerOutput {
    model_id: String,
    answer: String,
    mean_score: f64,
}

#[derive(Serialize)]
struct MetadataOutput {
    total_rounds: u32,
    total_calls: u32,
    elapsed_ms: u128,
    models_dropped: Vec<String>,
}

/// JSON error output schema.
#[derive(Serialize)]
struct ErrorResponse {
    status: String,
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
    provider: Option<String>,
    round: Option<u32>,
    phase: Option<String>,
    retryable: bool,
}

fn main() -> ExitCode {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let stderr_fd = std::io::stderr().as_raw_fd();
        #[allow(unsafe_code)]
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(stderr_fd, &raw mut termios) == 0
                && termios.c_lflag & libc::ISIG == 0
            {
                termios.c_lflag |= libc::ISIG;
                libc::tcsetattr(stderr_fd, libc::TCSANOW, &raw const termios);
            }
        }
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
        .block_on(async_main())
}

async fn async_main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Converge(args) => run_converge(args).await,
    }
}

#[allow(clippy::too_many_lines)]
async fn run_converge(args: ConvergeArgs) -> ExitCode {
    let shared = &args.shared;

    // Set up tracing
    let filter = if shared.debug {
        "debug"
    } else if shared.verbose {
        "info"
    } else {
        "warn"
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Resolve prompt text
    let prompt_text: Option<String> = match shared.prompt.as_deref() {
        Some("-") => {
            let mut buf = String::new();
            let bytes_read = match std::io::stdin().take(1_000_001).read_to_string(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error reading stdin: {e}");
                    return ExitCode::from(4);
                }
            };
            if bytes_read > 1_000_000 {
                eprintln!("Error: stdin input exceeds 1MB limit");
                return ExitCode::from(4);
            }
            Some(buf)
        }
        Some(p) => Some(p.to_string()),
        None => None,
    };

    if prompt_text.is_none() && shared.files.is_empty() {
        eprintln!("Error: a prompt or at least one --file must be provided");
        return ExitCode::from(4);
    }

    let prompt_bytes = prompt_text.as_deref().map_or(0, str::len);
    let file_budget = 1_000_000_usize.saturating_sub(prompt_bytes);
    let file_data: Vec<(String, String)> = if shared.files.is_empty() {
        Vec::new()
    } else {
        match read_and_validate_files(&shared.files, file_budget) {
            Ok(data) => data,
            Err(errors) => {
                for e in &errors {
                    eprintln!("Error: {e}");
                }
                return ExitCode::from(4);
            }
        }
    };

    let nonce = refinery_core::prompts::generate_nonce();
    let prompt =
        refinery_core::prompts::assemble_file_prompt(prompt_text.as_deref(), &file_data, &nonce);

    if shared.models.is_empty() {
        eprintln!("Error: at least one model must be specified with --models");
        return ExitCode::from(4);
    }

    let model_ids: Vec<ModelId> = match shared
        .models
        .iter()
        .map(|m| parse_model_spec(m))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(4);
        }
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

    if shared.dry_run {
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
        return ExitCode::SUCCESS;
    }

    let timeout = Duration::from_secs(shared.timeout);
    let idle_timeout = Duration::from_secs(shared.idle_timeout);

    let hidden = shared.verbose || shared.debug || !std::io::stderr().is_terminal();
    let display = progress::ProgressDisplay::new(hidden);

    let tundish_progress: Option<tundish_core::ProgressFn> = if hidden {
        None
    } else {
        Some(display.tundish_callback())
    };

    let mut providers: Vec<Arc<dyn ModelProvider>> = Vec::new();

    for model_id in &model_ids {
        match tundish_providers::build_provider(
            model_id,
            &shared.allow_tools,
            timeout,
            idle_timeout,
            tundish_progress.clone(),
        )
        .await
        {
            Ok(p) => providers.push(p),
            Err(e) => {
                eprintln!("Failed to initialize provider '{model_id}': {e}");
                return ExitCode::from(4);
            }
        }
    }

    let consensus_progress: Option<refinery_core::ProgressFn> = if hidden {
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
                ConvergenceStatus::MaxRoundsExceeded => ExitCode::from(2),
                ConvergenceStatus::InsufficientModels => ExitCode::from(3),
                ConvergenceStatus::Cancelled => ExitCode::from(1),
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

// ── Helpers ─────────────────────────────────────────────────────────────

fn read_and_validate_files(
    paths: &[PathBuf],
    budget: usize,
) -> Result<Vec<(String, String)>, Vec<String>> {
    let mut errors: Vec<String> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();
    let mut total_bytes: usize = 0;

    for path in paths {
        let path_str = path.display().to_string();

        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("file '{path_str}': {e}"));
                continue;
            }
        };

        if !meta.is_file() {
            errors.push(format!("file '{path_str}': not a regular file"));
            continue;
        }

        let file_size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
        if file_size > budget {
            errors.push(format!("file '{path_str}': exceeds 1MB limit"));
            continue;
        }

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("file '{path_str}': {e}"));
                continue;
            }
        };

        let Ok(text) = String::from_utf8(bytes) else {
            errors.push(format!("file '{path_str}': not valid UTF-8"));
            continue;
        };

        total_bytes += text.len();
        files.push((path_str, text));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if total_bytes > budget {
        return Err(vec![format!(
            "total file size ({total_bytes} bytes) exceeds 1MB limit"
        )]);
    }

    Ok(files)
}

fn parse_model_spec(input: &str) -> Result<ModelId, String> {
    if input.contains('/') {
        let (provider, model) = input.split_once('/').unwrap();
        if provider.is_empty() || model.is_empty() {
            return Err(format!("Invalid model spec: '{input}'"));
        }
        Ok(ModelId::from_parts(provider, model))
    } else {
        match input {
            "claude-code" => Ok(ModelId::from_parts("claude-code", "claude-opus-4-6")),
            "codex-cli" => Ok(ModelId::from_parts("codex-cli", "gpt-5.4")),
            "gemini-cli" => Ok(ModelId::from_parts("gemini-cli", "gemini-3.1-pro-preview")),
            "claude" | "codex" | "gemini" => Err(format!(
                "Unknown provider '{input}'. The format is now 'provider/model'. \
                 Did you mean '{input}-code' or '{input}-cli'? \
                 Supported providers: claude-code, codex-cli, gemini-cli, opencode"
            )),
            _ => Err(format!(
                "Unknown provider '{input}'. Supported: claude-code, codex-cli, gemini-cli, opencode"
            )),
        }
    }
}

fn make_run_dir(base: &std::path::Path, prompt: Option<&str>) -> std::path::PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = epoch_to_utc(secs);
    let timestamp = format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}");
    let slug: String = prompt
        .unwrap_or("no-prompt")
        .chars()
        .take(40)
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    base.join(format!("{timestamp}_{slug}"))
}

fn epoch_to_utc(epoch: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = epoch % 60;
    let mi = (epoch / 60) % 60;
    let h = (epoch / 3600) % 24;
    let mut days = epoch / 86400;
    let mut y = 1970;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        mo += 1;
    }
    (y, mo + 1, days + 1, h, mi, s)
}

fn save_round_artifacts(
    base_dir: &std::path::Path,
    rounds: &[RoundOutcome],
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(base_dir)?;

    for round in rounds {
        let round_dir = base_dir.join(format!("round-{}", round.round));
        std::fs::create_dir_all(&round_dir)?;

        for (model_id, text) in &round.proposals.proposals {
            let safe_id = model_id.to_string().replace('/', "_");
            let path = round_dir.join(format!("propose-{safe_id}.md"));
            std::fs::write(&path, text)?;
        }

        for ((evaluator, evaluatee), eval) in &round.evaluations.evaluations {
            let safe_evaluator = evaluator.to_string().replace('/', "_");
            let safe_evaluatee = evaluatee.to_string().replace('/', "_");
            let path = round_dir.join(format!("evaluate-{safe_evaluator}-{safe_evaluatee}.json"));
            let content = serde_json::json!({
                "evaluator": evaluator.to_string(),
                "evaluatee": evaluatee.to_string(),
                "score": eval.score.value(),
                "rationale": eval.rationale,
                "strengths": eval.review.strengths,
                "weaknesses": eval.review.weaknesses,
                "suggestions": eval.review.suggestions,
                "overall_assessment": eval.review.overall_assessment,
            });
            std::fs::write(&path, serde_json::to_string_pretty(&content)?)?;
        }
    }

    Ok(())
}

fn converge_error_to_detail(err: &refinery_core::ConvergeError) -> ErrorDetail {
    match err {
        refinery_core::ConvergeError::PhaseFailure {
            phase,
            model,
            source: _,
        } => ErrorDetail {
            code: "phase_failure".to_string(),
            message: err.to_string(),
            provider: Some(model.to_string()),
            round: None,
            phase: Some(phase.to_string()),
            retryable: true,
        },
        refinery_core::ConvergeError::InsufficientModels { round, .. } => ErrorDetail {
            code: "insufficient_models".to_string(),
            message: err.to_string(),
            provider: None,
            round: Some(*round),
            phase: None,
            retryable: false,
        },
        refinery_core::ConvergeError::ConfigInvalid { .. } => ErrorDetail {
            code: "config_invalid".to_string(),
            message: err.to_string(),
            provider: None,
            round: None,
            phase: None,
            retryable: false,
        },
        refinery_core::ConvergeError::Cancelled => ErrorDetail {
            code: "cancelled".to_string(),
            message: err.to_string(),
            provider: None,
            round: None,
            phase: None,
            retryable: false,
        },
    }
}
