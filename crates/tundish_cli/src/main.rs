use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;

use tundish_core::ModelProvider;

/// Dispatch one prompt to N models and get N answers back.
#[derive(Parser, Debug)]
#[command(name = "tundish", version, about)]
struct Cli {
    /// The prompt to send to all models.
    #[arg(value_name = "PROMPT")]
    prompt: String,

    /// Comma-separated model list [e.g., claude,codex,gemini]
    #[arg(short, long, value_delimiter = ',')]
    models: Vec<String>,

    /// Hard wall-clock timeout per call in seconds [default: 1800]
    #[arg(long, default_value = "1800")]
    timeout: u64,

    /// Idle timeout: max seconds of silence before killing a subprocess [default: 120]
    #[arg(long, default_value = "120")]
    idle_timeout: u64,

    /// Tools to allow: `web_fetch`, `web_search`, `file_read`, `file_write`, `shell`.
    #[arg(long = "allow-tools", value_delimiter = ',')]
    allow_tools: Vec<String>,

    /// Show debug output
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let filter = if cli.debug { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    if cli.models.is_empty() {
        eprintln!("Error: at least one model must be specified with --models");
        return ExitCode::from(1);
    }

    let timeout = Duration::from_secs(cli.timeout);
    let idle_timeout = Duration::from_secs(cli.idle_timeout);

    // Build providers
    let mut providers: Vec<Arc<dyn ModelProvider>> = Vec::new();
    for model in &cli.models {
        match tundish_providers::build_provider(model, &cli.allow_tools, timeout, idle_timeout, None)
            .await
        {
            Ok(p) => providers.push(p),
            Err(e) => {
                eprintln!("Failed to initialize provider '{model}': {e}");
                return ExitCode::from(1);
            }
        }
    }

    // Dispatch prompt to all models concurrently
    let messages = vec![
        tundish_core::Message::user(&cli.prompt),
    ];

    let handles: Vec<_> = providers
        .iter()
        .map(|provider| {
            let provider = provider.clone();
            let messages = messages.clone();
            tokio::spawn(async move {
                let model_id = provider.model_id().clone();
                let result = provider.send_message(&messages).await;
                (model_id, result)
            })
        })
        .collect();

    let mut had_errors = false;
    for handle in handles {
        match handle.await {
            Ok((model_id, Ok(answer))) => {
                println!("--- {model_id} ---\n");
                println!("{answer}\n");
            }
            Ok((model_id, Err(e))) => {
                eprintln!("Error from {model_id}: {e}");
                had_errors = true;
            }
            Err(e) => {
                eprintln!("Task join error: {e}");
                had_errors = true;
            }
        }
    }

    if had_errors {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
