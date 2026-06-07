use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tundish_core::ModelProvider;
use tundish_core::error::ProviderError;
use tundish_core::progress::ProgressFn;
use tundish_core::types::{Message, ModelId};

use crate::{process, tools};

/// pi CLI provider adapter.
///
/// Invokes: `pi --mode json --no-session --no-context-files --model provider/model "PROMPT"`
///
/// The model name is the full pi model spec after `pi/`, e.g.
/// `pi/openai/gpt-5.4` passes `openai/gpt-5.4` to `pi --model`.
/// pi manages its own credentials and model registry in local config.
pub struct PiProvider {
    model_id: ModelId,
    binary_path: PathBuf,
    pi_model: String,
    allowed_tools: Vec<String>,
    max_timeout: Duration,
    idle_timeout: Duration,
    progress: Option<ProgressFn>,
}

impl std::fmt::Debug for PiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PiProvider")
            .field("model_id", &self.model_id)
            .field("pi_model", &self.pi_model)
            .finish_non_exhaustive()
    }
}

impl PiProvider {
    /// Create a new pi provider.
    pub fn new(
        model_id: ModelId,
        canonical_tools: &[String],
        max_timeout: Duration,
        idle_timeout: Duration,
        progress: Option<ProgressFn>,
    ) -> Result<Self, ProviderError> {
        let binary_path = process::resolve_binary("pi")?;
        let pi_model = model_id.model().to_string();

        let (allowed_tools, unknown) = tools::resolve(canonical_tools, tools::pi_tool);
        for name in &unknown {
            tracing::warn!(provider = "pi", tool = %name, "unknown tool, skipping");
        }

        Ok(Self {
            model_id,
            binary_path,
            pi_model,
            allowed_tools,
            max_timeout,
            idle_timeout,
            progress,
        })
    }

    fn build_args(&self, system_prompt: &str, user_prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--mode".to_string(),
            "json".to_string(),
            "--no-session".to_string(),
            "--no-context-files".to_string(),
            "--model".to_string(),
            self.pi_model.clone(),
            "--system-prompt".to_string(),
            system_prompt.to_string(),
        ];

        if self.allowed_tools.is_empty() {
            args.push("--no-tools".to_string());
        } else {
            args.push("--tools".to_string());
            args.push(self.allowed_tools.join(","));
        }

        args.push(user_prompt.to_string());
        args
    }
}

#[async_trait]
impl ModelProvider for PiProvider {
    async fn send_message(
        &self,
        messages: &[Message],
        schema: Option<&str>,
    ) -> Result<String, ProviderError> {
        let (system_prompt, user_prompt) = process::extract_prompts(messages);
        let user_prompt = match schema {
            Some(schema) => format!(
                "{user_prompt}\n\nRespond with ONLY a JSON object matching this JSON Schema. \
                 Do not include markdown fences or explanatory text.\n\n```json\n{schema}\n```"
            ),
            None => user_prompt,
        };

        let args = self.build_args(&system_prompt, &user_prompt);
        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let env_vars = pi_env_vars();
        let env_var_refs: Vec<(&str, &str)> = env_vars
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();

        let mut parser = PiResponseParser::new(&self.model_id);
        process::spawn_cli_stream_lines(
            &self.binary_path,
            &args_refs,
            &env_var_refs,
            self.max_timeout,
            self.idle_timeout,
            &self.model_id,
            self.progress.clone(),
            |line| parser.observe_line(line),
        )
        .await?;

        parser.finish()
    }

    fn model_id(&self) -> &ModelId {
        &self.model_id
    }
}

fn pi_env_vars() -> Vec<(String, String)> {
    let mut env_vars = vec![
        ("PI_SKIP_VERSION_CHECK".to_string(), "1".to_string()),
        ("PI_TELEMETRY".to_string(), "0".to_string()),
    ];

    for key in PI_PASSTHROUGH_ENV {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                env_vars.push(((*key).to_string(), value));
            }
        }
    }

    env_vars
}

const PI_PASSTHROUGH_ENV: &[&str] = &[
    "HOME",
    "USERPROFILE",
    "PI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_SESSION_DIR",
    "PI_PACKAGE_DIR",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_OAUTH_TOKEN",
    "OPENAI_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "AZURE_OPENAI_RESOURCE_NAME",
    "AZURE_OPENAI_API_VERSION",
    "AZURE_OPENAI_DEPLOYMENT_NAME_MAP",
    "DEEPSEEK_API_KEY",
    "GEMINI_API_KEY",
    "GROQ_API_KEY",
    "CEREBRAS_API_KEY",
    "XAI_API_KEY",
    "FIREWORKS_API_KEY",
    "TOGETHER_API_KEY",
    "OPENROUTER_API_KEY",
    "AI_GATEWAY_API_KEY",
    "ZAI_API_KEY",
    "MISTRAL_API_KEY",
    "MINIMAX_API_KEY",
    "MOONSHOT_API_KEY",
    "OPENCODE_API_KEY",
    "KIMI_API_KEY",
    "CLOUDFLARE_API_KEY",
    "CLOUDFLARE_ACCOUNT_ID",
    "CLOUDFLARE_GATEWAY_ID",
    "XIAOMI_API_KEY",
    "XIAOMI_TOKEN_PLAN_CN_API_KEY",
    "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
    "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
    "AWS_PROFILE",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_REGION",
];

/// Extract the assistant response text from pi's `--mode json` JSONL stream.
pub fn extract_pi_response(jsonl: &str, model_id: &ModelId) -> Result<String, ProviderError> {
    let mut parser = PiResponseParser::new(model_id);
    for line in jsonl.lines() {
        parser.observe_line(line)?;
    }
    parser.finish()
}

struct PiResponseParser {
    model: ModelId,
    preview: String,
    latest_message_text: Option<String>,
    delta_text: String,
}

impl PiResponseParser {
    fn new(model_id: &ModelId) -> Self {
        Self {
            model: model_id.clone(),
            preview: String::new(),
            latest_message_text: None,
            delta_text: String::new(),
        }
    }

    fn observe_line(&mut self, line: &str) -> Result<(), ProviderError> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        if self.preview.len() < 200 {
            let remaining = 200 - self.preview.len();
            self.preview
                .push_str(&line.chars().take(remaining).collect::<String>());
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
            return Ok(());
        };

        if let Some(message) = error_message_from_event(&parsed) {
            return Err(ProviderError::ProcessFailed {
                model: self.model.clone(),
                message,
                exit_code: None,
            });
        }

        let event_type = parsed
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if event_type == "message_update" {
            if let Some(delta) = parsed
                .get("assistantMessageEvent")
                .and_then(|event| event.get("delta"))
                .and_then(|delta| delta.as_str())
            {
                if delta.starts_with(&self.delta_text) {
                    self.delta_text = delta.to_string();
                } else if !self.delta_text.ends_with(delta) {
                    self.delta_text.push_str(delta);
                }
            }
        }

        if matches!(event_type, "message_end" | "turn_end") {
            if let Some(text) = parsed.get("message").and_then(assistant_message_text) {
                self.latest_message_text = Some(text);
            }
        }

        Ok(())
    }

    fn finish(self) -> Result<String, ProviderError> {
        if let Some(text) = self
            .latest_message_text
            .filter(|text| !text.trim().is_empty())
        {
            return Ok(text);
        }
        if !self.delta_text.trim().is_empty() {
            return Ok(self.delta_text);
        }

        Err(ProviderError::InvalidJson {
            model: self.model,
            message: format!(
                "no assistant text found in pi JSON stream (raw: {})",
                self.preview
            ),
        })
    }
}

fn assistant_message_text(message: &serde_json::Value) -> Option<String> {
    let role = message.get("role").and_then(|role| role.as_str());
    if role != Some("assistant") {
        return None;
    }

    let content = message.get("content")?;
    text_from_content(content)
}

fn text_from_content(content: &serde_json::Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    let mut parts = Vec::new();
    for block in content.as_array()? {
        if block.get("type").and_then(|value| value.as_str()) == Some("text") {
            if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
                parts.push(text.to_string());
            }
        }
    }

    (!parts.is_empty()).then(|| parts.join(""))
}

fn error_message_from_event(event: &serde_json::Value) -> Option<String> {
    if event.get("type").and_then(|value| value.as_str()) == Some("error") {
        return event
            .get("message")
            .or_else(|| event.get("error").and_then(|error| error.get("message")))
            .and_then(|message| message.as_str())
            .map(str::to_string)
            .or_else(|| Some("pi reported an error".to_string()));
    }

    let message = event.get("message")?;
    let stop_reason = message
        .get("stopReason")
        .and_then(|reason| reason.as_str())
        .unwrap_or("");
    if stop_reason == "error" || stop_reason == "aborted" {
        return message
            .get("errorMessage")
            .and_then(|error| error.as_str())
            .map(str::to_string)
            .or_else(|| Some(format!("pi message ended with stopReason={stop_reason}")));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_disables_tools_by_default() {
        let provider = PiProvider {
            model_id: ModelId::from_parts("pi", "openai/gpt-5.4"),
            binary_path: PathBuf::from("/usr/local/bin/pi"),
            pi_model: "openai/gpt-5.4".to_string(),
            allowed_tools: vec![],
            max_timeout: Duration::from_secs(1800),
            idle_timeout: Duration::from_secs(120),
            progress: None,
        };

        let args = provider.build_args("system", "user");
        assert!(args.contains(&"--mode".to_string()));
        assert!(args.contains(&"json".to_string()));
        assert!(args.contains(&"--no-session".to_string()));
        assert!(args.contains(&"--no-context-files".to_string()));
        assert!(args.contains(&"--no-tools".to_string()));
        assert!(args.contains(&"openai/gpt-5.4".to_string()));
    }

    #[test]
    fn build_args_allows_selected_tools() {
        let provider = PiProvider {
            model_id: ModelId::from_parts("pi", "openai/gpt-5.4"),
            binary_path: PathBuf::from("/usr/local/bin/pi"),
            pi_model: "openai/gpt-5.4".to_string(),
            allowed_tools: vec!["read".to_string(), "bash".to_string()],
            max_timeout: Duration::from_secs(1800),
            idle_timeout: Duration::from_secs(120),
            progress: None,
        };

        let args = provider.build_args("system", "user");
        assert!(!args.contains(&"--no-tools".to_string()));
        assert!(args.contains(&"--tools".to_string()));
        assert!(args.contains(&"read,bash".to_string()));
    }

    #[test]
    fn extract_message_end_text() {
        let jsonl = r#"{"type":"session","version":3}
{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Hello"},{"type":"text","text":" world"}],"stopReason":"stop"}}
{"type":"agent_end","messages":[]}"#;

        let response =
            extract_pi_response(jsonl, &ModelId::from_parts("pi", "openai/test")).unwrap();
        assert_eq!(response, "Hello world");
    }

    #[test]
    fn extract_streaming_delta_fallback() {
        let jsonl = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello "}}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"world"}}"#;

        let response =
            extract_pi_response(jsonl, &ModelId::from_parts("pi", "openai/test")).unwrap();
        assert_eq!(response, "Hello world");
    }

    #[test]
    fn extract_accumulated_streaming_delta_fallback() {
        let jsonl = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello"}}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello world"}}"#;

        let response =
            extract_pi_response(jsonl, &ModelId::from_parts("pi", "openai/test")).unwrap();
        assert_eq!(response, "Hello world");
    }

    #[test]
    fn extract_error_event() {
        let jsonl = r#"{"type":"error","message":"auth failed"}"#;
        let err =
            extract_pi_response(jsonl, &ModelId::from_parts("pi", "openai/test")).unwrap_err();
        assert!(err.to_string().contains("auth failed"));
    }
}
