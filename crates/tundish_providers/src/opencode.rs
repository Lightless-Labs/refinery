use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tundish_core::ModelProvider;
use tundish_core::error::ProviderError;
use tundish_core::progress::ProgressFn;
use tundish_core::types::{Message, ModelId, Role};

use crate::process;

/// `OpenCode` CLI provider adapter.
///
/// Invokes: `opencode run --model provider/model --format json "PROMPT"`
///
/// `OpenCode` supports multiple sub-providers (opencode, kimi-for-coding,
/// minimax-cn-coding-plan, zai-coding-plan) each with their own models.
/// The model name passed to `--model` is the full `sub-provider/model` path.
///
/// No authentication env vars needed — opencode manages its own credentials.
pub struct OpenCodeProvider {
    model_id: ModelId,
    binary_path: PathBuf,
    /// The full model spec for `--model` (e.g. "opencode/minimax-m2.5-free")
    opencode_model: String,
    max_timeout: Duration,
    idle_timeout: Duration,
    progress: Option<ProgressFn>,
}

impl std::fmt::Debug for OpenCodeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenCodeProvider")
            .field("model_id", &self.model_id)
            .field("opencode_model", &self.opencode_model)
            .finish_non_exhaustive()
    }
}

impl OpenCodeProvider {
    /// Create a new `OpenCode` provider.
    ///
    /// The `model_id` model name is the full opencode model spec
    /// (e.g. `opencode/minimax-m2.5-free` or `kimi-for-coding/kimi-k2-thinking`).
    pub async fn new(
        model_id: ModelId,
        max_timeout: Duration,
        idle_timeout: Duration,
        progress: Option<ProgressFn>,
    ) -> Result<Self, ProviderError> {
        let binary_path = process::resolve_binary("opencode").await?;
        let opencode_model = model_id.model().to_string();

        Ok(Self {
            model_id,
            binary_path,
            opencode_model,
            max_timeout,
            idle_timeout,
            progress,
        })
    }

    fn build_args(&self, prompt: &str) -> Vec<String> {
        vec![
            "run".to_string(),
            "--model".to_string(),
            self.opencode_model.clone(),
            "--format".to_string(),
            "json".to_string(),
            prompt.to_string(),
        ]
    }
}

#[async_trait]
impl ModelProvider for OpenCodeProvider {
    async fn send_message(
        &self,
        messages: &[Message],
        _schema: Option<&str>,
    ) -> Result<String, ProviderError> {
        // `OpenCode` has no system prompt flag — prepend system to user prompt
        let system = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let user = messages
            .iter()
            .filter(|m| m.role == Role::User)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = if system.is_empty() {
            user
        } else {
            format!("{system}\n\n{user}")
        };

        let args = self.build_args(&prompt);
        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        // `OpenCode` manages its own credentials — pass HOME for config access
        let home = std::env::var("HOME").ok();
        let mut env_vars: Vec<(&str, &str)> = Vec::new();
        if let Some(ref h) = home {
            env_vars.push(("HOME", h.as_str()));
        }

        let output = process::spawn_cli(
            &self.binary_path,
            &args_refs,
            &env_vars,
            self.max_timeout,
            self.idle_timeout,
            &self.model_id,
            self.progress.clone(),
        )
        .await?;

        extract_opencode_response(&output)
    }

    fn model_id(&self) -> &ModelId {
        &self.model_id
    }
}

/// Extract the response text from `OpenCode`'s JSONL output.
///
/// Scans for `{"type":"text"}` events and concatenates their `part.text` fields.
pub fn extract_opencode_response(jsonl: &str) -> Result<String, ProviderError> {
    let model = ModelId::from_parts("opencode", "unknown");
    let preview: String = jsonl.chars().take(200).collect();

    let mut texts = Vec::new();

    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        let event_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");

        // Surface error events (e.g. "Model not found", auth failures)
        if event_type == "error" {
            let message = parsed
                .get("error")
                .and_then(|e| e.get("data"))
                .and_then(|d| d.get("message"))
                .and_then(|m| m.as_str())
                .or_else(|| {
                    parsed
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                })
                .unwrap_or("unknown error");
            return Err(ProviderError::ProcessFailed {
                model,
                message: message.to_string(),
                exit_code: None,
            });
        }

        if event_type == "text" {
            if let Some(text) = parsed
                .get("part")
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
            {
                texts.push(text.to_string());
            }
        }
    }

    if texts.is_empty() {
        return Err(ProviderError::InvalidJson {
            model,
            message: format!("no text events found in JSONL stream (raw: {preview})"),
        });
    }

    Ok(texts.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_basic_response() {
        let jsonl = r#"{"type":"step_start","timestamp":1234,"sessionID":"s1","part":{"type":"step-start"}}
{"type":"text","timestamp":1235,"sessionID":"s1","part":{"type":"text","text":"Hello! How can I help?"}}
{"type":"step_finish","timestamp":1236,"sessionID":"s1","part":{"type":"step-finish","reason":"stop"}}"#;
        let result = extract_opencode_response(jsonl).unwrap();
        assert_eq!(result, "Hello! How can I help?");
    }

    #[test]
    fn extract_multi_text_events() {
        let jsonl = r#"{"type":"step_start","part":{}}
{"type":"text","part":{"text":"Part 1. "}}
{"type":"text","part":{"text":"Part 2."}}
{"type":"step_finish","part":{}}"#;
        let result = extract_opencode_response(jsonl).unwrap();
        assert_eq!(result, "Part 1. Part 2.");
    }

    #[test]
    fn extract_no_text_event() {
        let jsonl = r#"{"type":"step_start","part":{}}
{"type":"step_finish","part":{}}"#;
        assert!(extract_opencode_response(jsonl).is_err());
    }

    #[test]
    fn extract_error_event() {
        let jsonl = r#"{"type":"error","error":{"name":"UnknownError","data":{"message":"Model not found: opencode/fake-model."}}}"#;
        let err = extract_opencode_response(jsonl).unwrap_err();
        assert!(err.to_string().contains("Model not found"));
    }
}
