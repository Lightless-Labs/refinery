use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;

use tundish_core::{ModelId, ModelProvider};

/// Send a prompt to multiple AI models and print each answer.
///
/// Dispatches one prompt to N models across M providers concurrently
/// and prints the results. No consensus, no evaluation — just answers.
#[derive(Parser, Debug)]
#[command(name = "tundish", version, about)]
struct Cli {
    /// The prompt to send to all models.
    prompt: String,

    /// Models to query (provider/model format, e.g. claude-code/claude-opus-4-6).
    #[arg(short, long, required = true, value_delimiter = ',')]
    models: Vec<String>,

    /// Maximum timeout per model call in seconds.
    #[arg(long, default_value = "1800")]
    timeout: u64,

    /// Idle timeout per model (no output) in seconds.
    #[arg(long, default_value = "120")]
    idle_timeout: u64,

    /// Tools to allow (e.g. `web_search`, `code_execution`).
    #[arg(long, value_delimiter = ',', default_value = "")]
    allow_tools: Vec<String>,

    /// Enable verbose logging.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> ExitCode {
    #[allow(unsafe_code)]
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&raw mut set);
        libc::sigaddset(&raw mut set, libc::SIGINT);
        libc::pthread_sigmask(libc::SIG_BLOCK, &raw const set, std::ptr::null_mut());
    }

    std::thread::spawn(|| {
        #[allow(unsafe_code)]
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&raw mut set);
            libc::sigaddset(&raw mut set, libc::SIGINT);
            let mut sig: libc::c_int = 0;
            libc::sigwait(&raw const set, &raw mut sig);
            libc::_exit(130);
        }
    });

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
        .block_on(async_main())
}

async fn async_main() -> ExitCode {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("tundish=debug,tundish_providers=debug")
            .with_target(false)
            .init();
    }

    let timeout = Duration::from_secs(cli.timeout);
    let idle_timeout = Duration::from_secs(cli.idle_timeout);

    let model_ids: Vec<ModelId> = match cli
        .models
        .iter()
        .map(|s| ModelId::parse(s))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("Invalid model ID: {e}");
            return ExitCode::from(2);
        }
    };

    let mut providers: Vec<Arc<dyn ModelProvider>> = Vec::new();
    for model_id in &model_ids {
        match tundish_providers::build_provider(
            model_id,
            &cli.allow_tools,
            timeout,
            idle_timeout,
            None,
        )
        .await
        {
            Ok(p) => providers.push(p),
            Err(e) => {
                eprintln!("Failed to initialize '{model_id}': {e}");
                return ExitCode::from(4);
            }
        }
    }

    let messages = vec![tundish_core::Message::user(&cli.prompt)];

    let mut handles = tokio::task::JoinSet::new();
    for provider in providers {
        let msgs = messages.clone();
        handles.spawn(async move {
            let model_id = provider.model_id().clone();
            let result = provider.send_message(&msgs, None).await;
            (model_id, result)
        });
    }

    let mut exit_code = ExitCode::SUCCESS;
    while let Some(result) = handles.join_next().await {
        match result {
            Ok((model_id, Ok(answer))) => {
                println!("─── {model_id} ───");
                println!("{answer}");
                println!();
            }
            Ok((model_id, Err(e))) => {
                eprintln!("─── {model_id} (ERROR) ───");
                eprintln!("{e}");
                eprintln!();
                exit_code = ExitCode::from(1);
            }
            Err(join_err) => {
                eprintln!("Task panicked: {join_err}");
                exit_code = ExitCode::from(1);
            }
        }
    }

    exit_code
}
