use std::io::{IsTerminal as _, Read as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use tundish_core::ModelId;

use refinery_core::ModelProvider;
use refinery_core::types::RoundOutcome;

use crate::progress;

/// Shared options for all subcommands.
#[derive(clap::Parser, Debug)]
pub struct SharedArgs {
    /// The prompt (or - for stdin, max 1MB). Optional when --file is used.
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    /// File(s) to include in the prompt, tagged by filename (repeatable, 1MB total)
    #[arg(long = "file", short = 'f', value_name = "PATH")]
    pub files: Vec<PathBuf>,

    /// Comma-separated model list [e.g., claude-code,codex-cli/o3-pro,gemini-cli]
    #[arg(short, long, value_delimiter = ',')]
    pub models: Vec<String>,

    /// Hard wall-clock timeout per call in seconds [default: 1800]
    #[arg(long, default_value = "1800")]
    pub timeout: u64,

    /// Idle timeout: max seconds of silence before killing a subprocess [default: 120]
    #[arg(long, default_value = "120")]
    pub idle_timeout: u64,

    /// Max concurrent subprocess calls [default: 0 = unlimited]
    #[arg(long, default_value = "0")]
    pub max_concurrent: usize,

    /// Output format [text|json]
    #[arg(short, long, default_value = "text")]
    pub output_format: OutputFormat,

    /// Show per-round progress
    #[arg(short, long)]
    pub verbose: bool,

    /// Show raw CLI invocations and responses
    #[arg(long)]
    pub debug: bool,

    /// Tools to allow: `web_fetch`, `web_search`, `file_read`, `file_write`, `shell`.
    #[arg(long = "allow-tools", value_delimiter = ',')]
    pub allow_tools: Vec<String>,

    /// Directory to save per-round artifacts (proposals, evaluations)
    #[arg(long = "output-dir", value_name = "DIR")]
    pub output_dir: Option<PathBuf>,

    /// Show estimated call count and cost, then exit
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

// ── JSON output schemas ─────────────────────────────────────────────────

#[derive(Serialize)]
pub struct JsonOutput {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner: Option<WinnerOutput>,
    pub final_round: u32,
    pub strategy: String,
    pub all_answers: Vec<AnswerOutput>,
    pub metadata: MetadataOutput,
}

#[derive(Serialize)]
pub struct WinnerOutput {
    pub model_id: String,
    pub answer: String,
}

#[derive(Serialize)]
pub struct AnswerOutput {
    pub model_id: String,
    pub answer: String,
    pub mean_score: f64,
}

#[derive(Serialize)]
pub struct MetadataOutput {
    pub total_rounds: u32,
    pub total_calls: u32,
    pub elapsed_ms: u128,
    pub models_dropped: Vec<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub status: String,
    pub error: ErrorDetail,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub provider: Option<String>,
    pub round: Option<u32>,
    pub phase: Option<String>,
    pub retryable: bool,
}

// ── Shared functions ────────────────────────────────────────────────────

/// Set up tracing based on verbose/debug flags.
pub fn init_tracing(shared: &SharedArgs) {
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
}

/// Resolve the prompt from positional arg, stdin, or files.
pub fn resolve_prompt(shared: &SharedArgs) -> Result<String, ExitCode> {
    let prompt_text: Option<String> = match shared.prompt.as_deref() {
        Some("-") => {
            let mut buf = String::new();
            let bytes_read = match std::io::stdin().take(1_000_001).read_to_string(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error reading stdin: {e}");
                    return Err(ExitCode::from(4));
                }
            };
            if bytes_read > 1_000_000 {
                eprintln!("Error: stdin input exceeds 1MB limit");
                return Err(ExitCode::from(4));
            }
            Some(buf)
        }
        Some(p) => Some(p.to_string()),
        None => None,
    };

    if prompt_text.is_none() && shared.files.is_empty() {
        eprintln!("Error: a prompt or at least one --file must be provided");
        return Err(ExitCode::from(4));
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
                return Err(ExitCode::from(4));
            }
        }
    };

    let nonce = refinery_core::prompts::generate_nonce();
    Ok(refinery_core::prompts::assemble_file_prompt(
        prompt_text.as_deref(),
        &file_data,
        &nonce,
    ))
}

/// Parse model specs and build providers.
pub async fn build_providers(
    shared: &SharedArgs,
) -> Result<
    (
        Vec<ModelId>,
        Vec<Arc<dyn ModelProvider>>,
        progress::ProgressDisplay,
    ),
    ExitCode,
> {
    if shared.models.is_empty() {
        eprintln!("Error: at least one model must be specified with --models");
        return Err(ExitCode::from(4));
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
            return Err(ExitCode::from(4));
        }
    };

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
                return Err(ExitCode::from(4));
            }
        }
    }

    Ok((model_ids, providers, display))
}

pub fn parse_model_spec(input: &str) -> Result<ModelId, String> {
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

pub fn read_and_validate_files(
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

pub fn make_run_dir(base: &std::path::Path, prompt: Option<&str>) -> std::path::PathBuf {
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
    let random: u32 = rand::random::<u32>() & 0xFFFF;
    base.join(format!("{timestamp}_{slug}_{random:04x}"))
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

pub fn save_round_artifacts(
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

pub fn converge_error_to_detail(err: &refinery_core::ConvergeError) -> ErrorDetail {
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
