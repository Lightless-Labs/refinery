mod progress;

use std::io::{IsTerminal as _, Read as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use serde::Serialize;
use tracing::info;

use tundish_core::ModelId;

use refinery_core::types::{ConvergenceStatus, RoundOutcome};
use refinery_core::{EngineConfig, ModelProvider};

/// Iterative multi-model consensus engine.
///
/// Given a prompt, N models independently produce answers, cross-review each other's work,
/// score all answers — repeating until a configurable convergence criterion is met.
#[derive(Parser, Debug)]
#[command(name = "refinery", version, about)]
struct Cli {
    /// The prompt to reach consensus on (or - for stdin, max 1MB). Optional when --file is used.
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// File(s) to include in the prompt, tagged by filename (repeatable, 1MB total)
    #[arg(long = "file", short = 'f', value_name = "PATH")]
    files: Vec<PathBuf>,

    /// Comma-separated model list [e.g., claude-code,codex-cli/o3-pro,gemini-cli]
    #[arg(short, long, value_delimiter = ',')]
    models: Vec<String>,

    /// Score threshold for convergence [default: 8.0] (range: 1.0-10.0)
    #[arg(short, long, default_value = "8.0")]
    threshold: f64,

    /// Maximum rounds [default: 5] (range: 1-20)
    #[arg(short = 'r', long, default_value = "5")]
    max_rounds: u32,

    /// Hard wall-clock timeout per call in seconds [default: 1800] (range: 1-7200)
    #[arg(long, default_value = "1800")]
    timeout: u64,

    /// Idle timeout: max seconds of silence before killing a subprocess [default: 120] (range: 1-600)
    #[arg(long, default_value = "120")]
    idle_timeout: u64,

    /// Max concurrent subprocess calls [default: 0 = unlimited] (range: 0-50)
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
    /// Mapped to each provider's native tool names automatically.
    #[arg(long = "allow-tools", value_delimiter = ',')]
    allow_tools: Vec<String>,

    /// Directory to save per-round artifacts (proposals, evaluations)
    #[arg(long = "output-dir", value_name = "DIR")]
    output_dir: Option<PathBuf>,

    /// Show estimated call count and cost, then exit
    #[arg(long)]
    dry_run: bool,

    /// Run a UI test with mock data (no API calls). Scenarios: propose, evaluate, converge, multi-round
    #[arg(long = "test-ui", value_name = "SCENARIO", hide = true)]
    test_ui: Option<String>,
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
    winner: WinnerOutput,
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
    // Restore terminal sanity: a previous run (before the setsid fix) may have
    // left ISIG disabled, which prevents Ctrl+C from generating SIGINT.
    // This persists across commands, so we fix it on startup.
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

#[allow(clippy::too_many_lines)]
async fn async_main() -> ExitCode {
    let cli = Cli::parse();

    // Set up tracing
    let filter = if cli.debug {
        "debug"
    } else if cli.verbose {
        "info"
    } else {
        "warn"
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Resolve prompt text from positional arg or stdin
    let prompt_text: Option<String> = match cli.prompt.as_deref() {
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

    // At least one input source required
    if prompt_text.is_none() && cli.files.is_empty() {
        eprintln!("Error: a prompt or at least one --file must be provided");
        return ExitCode::from(4);
    }

    // Read and validate files (runs even during --dry-run for early validation)
    let prompt_bytes = prompt_text.as_deref().map_or(0, str::len);
    let file_budget = 1_000_000_usize.saturating_sub(prompt_bytes);
    let file_data: Vec<(String, String)> = if cli.files.is_empty() {
        Vec::new()
    } else {
        match read_and_validate_files(&cli.files, file_budget) {
            Ok(data) => data,
            Err(errors) => {
                for e in &errors {
                    eprintln!("Error: {e}");
                }
                return ExitCode::from(4);
            }
        }
    };

    // Assemble the final prompt
    let nonce = refinery_core::prompts::generate_nonce();
    let prompt =
        refinery_core::prompts::assemble_file_prompt(prompt_text.as_deref(), &file_data, &nonce);

    if cli.models.is_empty() {
        eprintln!("Error: at least one model must be specified with --models");
        return ExitCode::from(4);
    }

    let model_ids: Vec<ModelId> = match cli
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
        cli.max_rounds,
        cli.threshold,
        2, // stability_rounds
        Duration::from_secs(cli.timeout),
        cli.max_concurrent,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config error: {e}");
            return ExitCode::from(4);
        }
    };

    // Dry run: show cost estimate
    if cli.dry_run {
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

    // UI test mode: run mock scenarios without API calls
    if let Some(ref scenario) = cli.test_ui {
        return progress::run_test_ui(scenario);
    }

    // Build providers
    let timeout = Duration::from_secs(cli.timeout);
    let idle_timeout = Duration::from_secs(cli.idle_timeout);

    // Progress display: per-model spinners via indicatif, score tables via comfy-table.
    // Hidden when verbose/debug mode or non-TTY (piped output).
    let hidden = cli.verbose || cli.debug || !std::io::stderr().is_terminal();
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
            &cli.allow_tools,
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

    let strategy = Box::new(refinery_core::VoteThreshold::new(cli.threshold, 2));
    let engine =
        refinery_core::Engine::new(providers, strategy, config, consensus_progress.clone());

    info!("Starting consensus run with {} models", cli.models.len());

    let tick_handle = display.start_tick();
    let run_result = engine.run(&prompt).await;

    // Stop tick task and finalize display
    if let Some(handle) = tick_handle {
        handle.abort();
    }
    display.finish();

    match run_result {
        Ok((outcome, rounds)) => {
            // Save per-round artifacts if --output-dir is set
            if let Some(ref dir) = cli.output_dir {
                let run_dir = make_run_dir(dir, cli.prompt.as_deref());
                if let Err(e) = save_round_artifacts(&run_dir, &rounds) {
                    eprintln!("Warning: failed to save artifacts: {e}");
                }
            }

            match cli.output_format {
                OutputFormat::Json => {
                    let status_str = match serde_json::to_value(&outcome.status) {
                        Ok(serde_json::Value::String(s)) => s,
                        _ => format!("{:?}", outcome.status).to_lowercase(),
                    };
                    let json_output = JsonOutput {
                        status: status_str,
                        winner: WinnerOutput {
                            model_id: outcome.winner.to_string(),
                            answer: outcome.answer.clone(),
                        },
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
                    println!("Winner: {}", outcome.winner);
                    println!("Rounds: {}", outcome.final_round);
                    println!("Total calls: {}", outcome.total_calls);
                    println!("Elapsed: {:?}", outcome.elapsed);
                    println!("\n--- Answer ---\n");
                    println!("{}", outcome.answer);
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
            match cli.output_format {
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

        // Pre-read size guard to avoid allocating memory for huge files
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

/// Parse a CLI model spec into a `ModelId`.
///
/// Accepts `provider/model` or provider-only (applies default model).
fn parse_model_spec(input: &str) -> Result<ModelId, String> {
    if input.contains('/') {
        // Split on first slash: provider/model (model may contain slashes for opencode)
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

/// Build a per-run subdirectory inside the base output dir.
///
/// Format: `YYYYMMDD-HHMMSS_<prompt-slug>` where the slug is the first
/// 40 chars of the prompt, lowercased, with non-alphanumeric chars replaced by `-`.
fn make_run_dir(base: &std::path::Path, prompt: Option<&str>) -> std::path::PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Convert epoch seconds to UTC date-time components
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

/// Convert Unix epoch seconds to (year, month, day, hour, minute, second) in UTC.
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

        // Proposals: one file per model
        for (model_id, text) in &round.proposals.proposals {
            let safe_id = model_id.to_string().replace('/', "_");
            let path = round_dir.join(format!("propose-{safe_id}.md"));
            std::fs::write(&path, text)?;
        }

        // Evaluations: one file per (evaluator, evaluatee) pair
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
