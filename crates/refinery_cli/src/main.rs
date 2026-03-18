mod commands;
mod progress;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    Converge(commands::converge::ConvergeArgs),

    /// Synthesize the best answers from multiple models.
    ///
    /// Runs converge rounds first to raise quality, then all models produce
    /// a synthesis of the qualifying answers. Syntheses are evaluated on
    /// integration, coherence, completeness, and fidelity.
    Synthesize(commands::synthesize::SynthesizeArgs),
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
        Command::Converge(args) => commands::converge::run(args).await,
        Command::Synthesize(args) => commands::synthesize::run(args).await,
    }
}
