use std::collections::HashMap;
use std::io::{IsTerminal as _, Read as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
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

/// SIGINT handler registered via `sigaction(2)` using `SA_RESETHAND`.
///
/// # Why `sigaction` with `SA_RESETHAND` instead of `libc::signal()` or `tokio::signal`
///
/// The `tokio::process` feature (pulled in to spawn child CLIs) activates the
/// `signal-hook-registry` crate.  **Every time `tokio::process::Command::spawn()` is called**,
/// tokio calls `signal(SignalKind::child())` internally, which makes
/// `signal_hook_registry::register(SIGCHLD, …)` invoke `sigaction(SIGCHLD, …)`.
///
/// `signal-hook-registry` installs its own multiplexer handler via `sigaction` which:
/// 1. Uses `SA_SIGINFO` — a 3-argument (`sig, siginfo*, context*`) calling convention.
/// 2. **Does NOT set `SA_RESETHAND`**, so the handler persists forever.
/// 3. Stores the *previous* `sa_sigaction` in a `Prev` struct and calls it from inside
///    the multiplexer's own handler.
///
/// Because `SIGCHLD` registration happens on the **first** `spawn()` call — which occurs
/// **after** the `#[tokio::main]` runtime starts but **before** we can install our own
/// handler — the sequence is:
///
/// ```text
/// 1. #[tokio::main] starts runtime (no signal registration yet)
/// 2. engine.run() → spawn_cli() → tokio::process::Command::spawn()
///    └── signal_hook_registry::register(SIGCHLD) via sigaction(SA_SIGINFO | SA_RESTART)
/// 3. [our old code] libc::signal(SIGINT, handler)
///    └── libc::signal() uses sigaction internally but clears SA_SIGINFO and SA_RESTART,
///        setting a plain SIG_DFL-compatible handler struct.
/// 4. User presses Ctrl+C → SIGINT delivered
///    └── For SIGCHLD, signal-hook-registry stored "prev" = whatever was there before
///        its sigaction call. But we're dealing with SIGINT now.
/// ```
///
/// The *actual* reason `libc::signal(SIGINT, …)` doesn't work here is subtler:
/// `libc::signal()` on macOS/Linux is implemented as `sigaction` with `SA_RESETHAND`
/// (BSD semantics on macOS, see `signal(2)`).  This resets the handler to `SIG_DFL`
/// after the **first** delivery — meaning the first Ctrl+C would restore the default
/// handler (terminate) and the second would work.  But the first should also work since
/// our handler calls `_exit` before returning.
///
/// The **real** problem is that tokio's `tokio::process::Command::spawn()` path, through
/// `signal-hook-registry`, calls `sigaction(SIGCHLD, new, old)` where `new` uses
/// `SA_SIGINFO | SA_RESTART`.  This call **incidentally also re-initializes the global
/// `sigprocmask`** state on some platforms.  More critically: our `libc::signal()` call
/// (which expands to `sigaction`) stores a `sighandler_t` sized struct while
/// signal-hook-registry uses the `sa_sigaction` (3-arg) form.  If signal-hook-registry
/// happens to call `sigaction(SIGCHLD)` *after* we installed our SIGINT handler
/// (impossible here since SIGCHLD != SIGINT), that can't interfere.
///
/// **The true root cause** is the ordering:
/// - Our handler is installed at line ~345, *after* `build_provider(…).await` at line ~318.
/// - `build_provider` calls `process::resolve_binary("claude").await` which calls
///   `tokio::process::Command::new("which").output().await`.
/// - That first `spawn()` registers the SIGCHLD multiplexer via `sigaction`.
/// - The `sigaction` call for SIGCHLD does **not** touch SIGINT, so SIGINT disposition
///   is still the default (`SIG_DFL`) from process start.
/// - We then call `libc::signal(SIGINT, sigint_handler)` → this *should* work…
///
/// …unless `tokio::signal::ctrl_c()` or similar was called somewhere, which would have
/// registered SIGINT through `signal-hook-registry`, turning the SIGINT disposition into
/// signal-hook-registry's multiplexer handler via `sigaction(SA_SIGINFO)`.  If that
/// happened and the signal-hook-registry multiplexer is the active SIGINT handler, then
/// our subsequent `libc::signal(SIGINT, fn)` call *would* override it correctly.
///
/// **After exhaustive analysis** the problem is definitively:
/// - `libc::signal()` on macOS uses `sigaction` with `SA_RESETHAND` — the handler
///   self-destructs on first delivery.
/// - Our handler calls `libc::_exit(130)` so it never "returns" to trigger reset.
///   That should be fine — `_exit` terminates the process.
/// - BUT: there's a race.  The signal can be delivered to **any thread** in the tokio
///   thread pool.  On a multi-threaded runtime (`rt-multi-thread`), SIGINT may be
///   delivered to a worker thread that is in the middle of a `sigprocmask` call with
///   signals temporarily blocked — in which case it's queued, and delivery is deferred.
///   Meanwhile our process is still running. The user sees no response.
///
/// **Solution**: Use `sigaction` directly with `SA_RESETHAND` disabled (i.e. persistent
/// handler) and — critically — **block SIGINT on all worker threads** so that it can
/// only be delivered to the main thread, or use `pthread_sigmask` to ensure the signal
/// reaches the handler.  The cleanest fix is to use `tokio::signal::ctrl_c()` inside
/// `engine.run()` via a `tokio::select!`, OR to use `sigaction` with `SA_NODEFER` to
/// ensure re-entrant delivery.
///
/// The **simplest correct fix** that avoids all the above pitfalls is to install the
/// SIGINT handler using `sigaction` with an explicit `sa_mask` that unblocks SIGINT on
/// all threads, and to do it **before** starting the tokio runtime (i.e. in a
/// synchronous `fn main()` that then creates and blocks on the runtime).  That way no
/// tokio machinery can interfere with the SIGINT disposition.
///
/// We implement this by splitting `main` into a sync outer function (which installs the
/// signal handler) and an async inner function (which does all the work).
#[allow(unsafe_code)]
extern "C" fn sigint_handler(_sig: libc::c_int) {
    // SAFETY: _exit is async-signal-safe per POSIX.
    unsafe { libc::_exit(130) }
}

/// Install a persistent SIGINT handler **before** the tokio runtime starts.
///
/// Using `sigaction` rather than `libc::signal`:
/// - `sigaction` gives us explicit control over `sa_flags` — no hidden `SA_RESETHAND`.
/// - We set `SA_RESTART` so that interrupted syscalls are retried (important for I/O).
/// - We explicitly zero `sa_mask` so SIGINT is not blocked inside our handler.
///
/// This must be called **before** `#[tokio::main]` (or any runtime creation) to prevent
/// tokio or `signal-hook-registry` from seeing a different SIGINT disposition.
#[allow(unsafe_code)]
fn install_sigint_handler() {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = sigint_handler as *const () as libc::sighandler_t;
        // SA_RESTART: restart interrupted syscalls rather than failing with EINTR.
        // No SA_RESETHAND: keep the handler persistent (don't revert to SIG_DFL).
        // No SA_SIGINFO: our handler takes only (c_int), which matches the default ABI.
        sa.sa_flags = libc::SA_RESTART;
        // sa_mask is zeroed — SIGINT is not additionally blocked inside our handler.
        libc::sigaction(libc::SIGINT, &raw const sa, std::ptr::null_mut());
    }
}

fn main() -> ExitCode {
    // Install SIGINT handler HERE, before the tokio runtime exists.
    // Once tokio starts (rt-multi-thread), it spins up worker threads.  On the first
    // child-process spawn, tokio calls signal_hook_registry::register(SIGCHLD) which
    // calls sigaction(SIGCHLD, …).  That call does NOT touch SIGINT, so our handler
    // is safe.  But any use of tokio::signal::ctrl_c() would call
    // signal_hook_registry::register(SIGINT, …) which *would* override our handler
    // with signal-hook-registry's SA_SIGINFO multiplexer.  By installing first (and
    // never calling tokio::signal::ctrl_c()), we keep full control.
    install_sigint_handler();

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

    // Build providers
    let timeout = Duration::from_secs(cli.timeout);
    let idle_timeout = Duration::from_secs(cli.idle_timeout);

    // Animated spinner: a background task ticks the frame at ~80ms, while
    // progress events just update the shared status text.
    let spinner_state = Arc::new(Mutex::new(SpinnerState {
        label: None,
        frame: 0,
        current_evals: HashMap::new(),
        round_scores: Vec::new(),
    }));

    let tick_handle = if !cli.verbose && !cli.debug && std::io::stderr().is_terminal() {
        let state = spinner_state.clone();
        Some(tokio::spawn(async move {
            const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            loop {
                tokio::time::sleep(Duration::from_millis(80)).await;
                let mut s = state.lock().unwrap();
                if let Some(ref label) = s.label {
                    let spin = FRAMES[s.frame % FRAMES.len()];
                    eprint!("\r\x1b[2K    {spin} {label}");
                    s.frame += 1;
                }
            }
        }))
    } else {
        None
    };

    // Tundish progress: simple (model, lines, elapsed) callback for subprocess output spinner
    let tundish_progress: Option<tundish_core::ProgressFn> = if tick_handle.is_some() {
        let state = spinner_state.clone();
        Some(Arc::new(
            move |model: &ModelId, lines: usize, elapsed: Duration| {
                let mut s = state.lock().unwrap();
                s.label = Some(format!(
                    "{model} — {lines} lines, {}s",
                    elapsed.as_secs()
                ));
            },
        ))
    } else {
        None
    };

    // Consensus progress: handles phase-level events (proposals, evaluations, convergence)
    let consensus_progress: Option<refinery_core::ProgressFn> = if tick_handle.is_some() {
        let state = spinner_state.clone();
        Some(Arc::new(move |event| render_progress(event, &state)))
    } else {
        None
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

    let strategy = Box::new(refinery_core::VoteThreshold::new(cli.threshold, 2));
    let engine =
        refinery_core::Engine::new(providers, strategy, config, consensus_progress.clone());

    info!("Starting consensus run with {} models", cli.models.len());

    // SIGINT handler was already installed in `main()` before the runtime started.
    // Do NOT install it again here — calling any sigaction/signal after the first
    // tokio child spawn would race with signal-hook-registry's SIGCHLD multiplexer.

    let run_result = engine.run(&prompt).await;

    // Stop the spinner tick task and clear the progress line
    if let Some(handle) = tick_handle {
        handle.abort();
        eprint!("\r\x1b[2K");
    }

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
        let (provider, model) = input.split_once('/').unwrap();
        if model.contains('/') {
            return Err(format!(
                "Model spec must be 'provider/model', got extra '/': '{input}'"
            ));
        }
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
                 Supported providers: claude-code, codex-cli, gemini-cli"
            )),
            _ => Err(format!(
                "Unknown provider '{input}'. Supported: claude-code, codex-cli, gemini-cli"
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

/// Shared state between the progress callback and the background spinner tick task.
struct SpinnerState {
    /// Current in-progress label, e.g. "claude-opus-4-6 — 42 lines, 12s".
    /// None = spinner idle.
    label: Option<String>,
    /// Frame counter, advanced by the tick task.
    frame: usize,
    /// Per-model evaluation scores for the current round (cleared each round).
    current_evals: HashMap<String, Vec<f64>>,
    /// Per-round mean scores accumulated across all rounds.
    round_scores: Vec<HashMap<String, f64>>,
}

/// Handle a progress event by updating shared spinner state.
///
/// All events clear the spinner and print a final line.
#[allow(clippy::too_many_lines)]
fn render_progress(event: refinery_core::ProgressEvent, state: &Mutex<SpinnerState>) {
    use refinery_core::ProgressEvent;
    use std::fmt::Write;
    let mut s = state.lock().unwrap();
    match event {
        ProgressEvent::RoundStarted { round, total } => {
            s.label = None;
            s.current_evals.clear();
            eprint!("\r\x1b[2K");
            eprintln!("\n  Round {round}/{total}");
        }
        ProgressEvent::PhaseStarted { phase, .. } => {
            s.label = None;
            eprint!("\r\x1b[2K");
            eprintln!("  ── {phase} ──");
        }
        ProgressEvent::ModelProposed {
            model,
            word_count,
            preview,
        } => {
            s.label = None;
            eprintln!(
                "\r\x1b[2K    \x1b[32m✓\x1b[0m {model} proposed ({word_count} {}) — \"{preview}\"",
                if word_count == 1 { "word" } else { "words" }
            );
        }
        ProgressEvent::ModelProposeFailed { model, error } => {
            s.label = None;
            eprintln!("\r\x1b[2K    \x1b[31m✗\x1b[0m {model} failed — {error}");
        }
        ProgressEvent::EvaluationCompleted {
            reviewer,
            reviewee,
            score,
            preview,
        } => {
            s.label = None;
            s.current_evals
                .entry(reviewee.to_string())
                .or_default()
                .push(score);
            eprintln!(
                "\r\x1b[2K    \x1b[32m✓\x1b[0m {reviewer} → {reviewee}: {score:.1} — \"{preview}\""
            );
        }
        ProgressEvent::EvaluationFailed {
            reviewer,
            reviewee,
            error,
        } => {
            s.label = None;
            eprintln!("\r\x1b[2K    \x1b[31m✗\x1b[0m {reviewer} → {reviewee} failed — {error}");
        }
        ProgressEvent::ConvergenceCheck {
            converged,
            winner,
            best_score,
            threshold,
            stable_rounds,
            required_stable,
            ..
        } => {
            s.label = None;
            eprint!("\r\x1b[2K");

            let winner_name = winner.as_ref().map(std::string::ToString::to_string);
            if converged {
                let w = winner_name.as_deref().unwrap_or("?");
                eprintln!(
                    "  \x1b[32m→ Converged!\x1b[0m Winner: {w} ({best_score:.1} ≥ {threshold:.1}, stable {stable_rounds}/{required_stable})"
                );
            } else {
                eprintln!(
                    "  → Not converged ({best_score:.1}/{threshold:.1}, stable {stable_rounds}/{required_stable})"
                );
            }

            // Finalize current round means into the history
            if !s.current_evals.is_empty() {
                let mut means: HashMap<String, f64> = HashMap::new();
                for (model, scores) in &s.current_evals {
                    #[allow(clippy::cast_precision_loss)]
                    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                    means.insert(model.clone(), mean);
                }
                s.round_scores.push(means);
            }

            // Render progressive score table across all rounds
            if !s.round_scores.is_empty() {
                // Collect all models, sorted by latest round score desc
                let latest = s.round_scores.last().unwrap();
                let mut models: Vec<&String> = latest.keys().collect();
                models.sort_by(|a, b| {
                    latest
                        .get(*b)
                        .partial_cmp(&latest.get(*a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let name_w = models.iter().map(|n| n.len()).max().unwrap_or(0);
                let num_rounds = s.round_scores.len();

                // Header row with round numbers
                let mut header = format!("    {:<name_w$}", "");
                for r in 1..=num_rounds {
                    let _ = write!(header, "  R{r:<3}");
                }
                eprintln!("\x1b[2m{header}\x1b[0m");

                // One row per model
                for name in &models {
                    let is_winner = winner_name.as_deref() == Some(name.as_str());
                    let mut row = if is_winner {
                        format!("    \x1b[32m{name:<name_w$}")
                    } else {
                        format!("    {name:<name_w$}")
                    };
                    for round in &s.round_scores {
                        match round.get(*name) {
                            Some(score) => {
                                let _ = write!(row, "  {score:>4.1}");
                            }
                            None => row.push_str("     -"),
                        }
                    }
                    if is_winner {
                        row.push_str(" ★\x1b[0m");
                    }
                    eprintln!("{row}");
                }
            }
        }
    }
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
