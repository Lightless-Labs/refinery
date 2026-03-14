pub mod credential;
pub mod process;
pub mod tools;

#[cfg(feature = "claude")]
pub mod claude;
#[cfg(feature = "codex")]
pub mod codex;
#[cfg(feature = "gemini")]
pub mod gemini;
#[cfg(feature = "opencode")]
pub mod opencode;

use std::sync::Arc;
use std::time::Duration;

use tundish_core::{ModelId, ModelProvider, ProviderError};

/// Build a provider from a `ModelId`, dispatching on the provider name.
pub async fn build_provider(
    model_id: &ModelId,
    allowed_tools: &[String],
    max_timeout: Duration,
    idle_timeout: Duration,
    progress: Option<tundish_core::ProgressFn>,
) -> Result<Arc<dyn ModelProvider>, ProviderError> {
    match model_id.provider() {
        #[cfg(feature = "claude")]
        "claude-code" => {
            let provider = claude::ClaudeProvider::new(
                model_id.clone(),
                allowed_tools,
                max_timeout,
                idle_timeout,
                progress,
            )
            .await?;
            Ok(Arc::new(provider))
        }
        #[cfg(feature = "codex")]
        "codex-cli" => {
            let provider = codex::CodexProvider::new(
                model_id.clone(),
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
        "gemini-cli" => {
            let provider = gemini::GeminiProvider::new(
                model_id.clone(),
                allowed_tools,
                max_timeout,
                idle_timeout,
                progress,
            )
            .await?;
            Ok(Arc::new(provider))
        }
        #[cfg(feature = "opencode")]
        "opencode" => {
            let provider = opencode::OpenCodeProvider::new(
                model_id.clone(),
                max_timeout,
                idle_timeout,
                progress,
            )
            .await?;
            Ok(Arc::new(provider))
        }
        other => Err(ProviderError::ProcessFailed {
            model: model_id.clone(),
            message: format!(
                "Unknown provider: '{other}'. Supported: claude-code, codex-cli, gemini-cli, opencode"
            ),
            exit_code: None,
        }),
    }
}
