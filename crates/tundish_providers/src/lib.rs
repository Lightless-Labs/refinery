use std::sync::Arc;
use std::time::Duration;

use tundish_core::ModelProvider;
use tundish_core::progress::ProgressFn;

pub mod credential;
pub mod process;
pub mod tools;

#[cfg(feature = "claude")]
pub mod claude;
#[cfg(feature = "codex")]
pub mod codex;
#[cfg(feature = "gemini")]
pub mod gemini;

/// Build a provider from a model name string.
///
/// Recognizes prefixes: `claude`, `codex`, `gemini`.
/// Defaults: `claude` -> `opus-4-6`, `codex` -> `gpt-5.4`, `gemini` -> `gemini-3.1-pro-preview`.
pub async fn build_provider(
    model: &str,
    allowed_tools: &[String],
    max_timeout: Duration,
    idle_timeout: Duration,
    progress: Option<ProgressFn>,
) -> Result<Arc<dyn ModelProvider>, Box<dyn std::error::Error>> {
    match model {
        #[cfg(feature = "claude")]
        m if m.starts_with("claude") => {
            let model_name = m.strip_prefix("claude-").unwrap_or("opus-4-6");
            let provider = claude::ClaudeProvider::new(
                model_name,
                allowed_tools,
                max_timeout,
                idle_timeout,
                progress,
            )
            .await?;
            Ok(Arc::new(provider))
        }
        #[cfg(feature = "codex")]
        m if m == "codex" || m.starts_with("codex-") => {
            let model_name = m.strip_prefix("codex-").unwrap_or("gpt-5.4");
            let provider = codex::CodexProvider::new(
                model_name,
                "xhigh",
                allowed_tools,
                max_timeout,
                idle_timeout,
                progress,
            )
            .await?;
            Ok(Arc::new(provider))
        }
        #[cfg(feature = "gemini")]
        m if m.starts_with("gemini") => {
            let model_name = if m == "gemini" {
                "gemini-3.1-pro-preview"
            } else {
                m
            };
            let provider = gemini::GeminiProvider::new(
                model_name,
                allowed_tools,
                max_timeout,
                idle_timeout,
                progress,
            )
            .await?;
            Ok(Arc::new(provider))
        }
        _ => Err(format!(
            "Unknown model: {model}. Supported: claude[-model], codex, gemini[-model]"
        )
        .into()),
    }
}
