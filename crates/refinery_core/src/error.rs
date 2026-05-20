use crate::types::{ModelId, Phase};

// Re-export from tundish_core
pub use tundish_core::ProviderError;

/// Errors from the consensus engine.
#[derive(Debug, thiserror::Error)]
pub enum ConvergeError {
    #[error("phase {phase} failed for model {model}: {source}")]
    PhaseFailure {
        phase: Phase,
        model: ModelId,
        source: ProviderError,
    },

    #[error("insufficient models in round {round}: {remaining} remaining, {minimum} required")]
    InsufficientModels {
        round: u32,
        remaining: usize,
        minimum: usize,
    },

    #[error("invalid config: {field} = {value} ({constraint})")]
    ConfigInvalid {
        field: &'static str,
        value: String,
        constraint: String,
    },

    #[error("consensus run cancelled")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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

    #[test]
    fn converge_error_display() {
        let err = ConvergeError::InsufficientModels {
            round: 3,
            remaining: 1,
            minimum: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("round 3"));
        assert!(msg.contains("1 remaining"));
        assert!(msg.contains("2 required"));
    }

    #[test]
    fn config_invalid_carries_structured_info() {
        let err = ConvergeError::ConfigInvalid {
            field: "max_rounds",
            value: "25".to_string(),
            constraint: "must be 1-20".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("max_rounds"));
        assert!(msg.contains("25"));
        assert!(msg.contains("must be 1-20"));
    }
}
