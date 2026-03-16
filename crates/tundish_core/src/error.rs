use std::time::Duration;

use crate::types::ModelId;

/// Errors from individual provider backends.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("model {model} timed out after {elapsed:?}")]
    Timeout { model: ModelId, elapsed: Duration },

    #[error("model {model} idle timed out after {idle:?} (no output)")]
    IdleTimeout { model: ModelId, idle: Duration },

    #[error("model {model} returned invalid JSON: {message}")]
    InvalidJson { model: ModelId, message: String },

    #[error("model {model} process failed: {message} (exit code: {exit_code:?})")]
    ProcessFailed {
        model: ModelId,
        message: String,
        exit_code: Option<i32>,
    },

    #[error("model {model} response too large: {size} bytes (max: {max})")]
    ResponseTooLarge {
        model: ModelId,
        size: usize,
        max: usize,
    },

    #[error("model {model} JSON nesting too deep: {depth} levels (max: {max})")]
    JsonTooDeep {
        model: ModelId,
        depth: usize,
        max: usize,
    },

    #[error("missing credential: {var_name} not set for {provider}")]
    MissingCredential { provider: String, var_name: String },

    #[error("CLI binary not found: {binary_name}")]
    BinaryNotFound { binary_name: String },
}

impl ProviderError {
    /// Whether this error is permanent and the model should not be retried.
    ///
    /// Permanent errors: missing credentials, binary not found, process failures
    /// that indicate invalid models or auth issues.
    /// Transient errors: timeouts, idle timeouts, JSON parse failures.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        match self {
            Self::MissingCredential { .. } | Self::BinaryNotFound { .. } => true,
            Self::ProcessFailed { message, .. } => {
                // Only permanent if the message indicates a non-retryable error
                message.contains("not found")
                    || message.contains("not supported")
                    || message.contains("not exist")
                    || message.contains("issue with the selected model")
                    || message.contains("Authentication")
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_display() {
        let err = ProviderError::Timeout {
            model: ModelId::new("test/claude"),
            elapsed: Duration::from_secs(120),
        };
        assert!(err.to_string().contains("claude"));
        assert!(err.to_string().contains("120"));
    }
}
