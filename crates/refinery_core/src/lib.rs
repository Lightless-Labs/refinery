pub mod brainstorm;
pub mod engine;
pub mod error;
pub mod phases;
pub mod progress;
pub mod prompts;
pub mod scoring;
pub mod strategy;
pub mod types;

pub use engine::{Engine, Session};
pub use error::{ConvergeError, ProviderError};
pub use progress::{ProgressEvent, ProgressFn};
pub use strategy::{ClosingDecision, ClosingStrategy, VoteThreshold};
pub use types::{
    ConsensusOutcome, ConvergenceStatus, CostEstimate, EngineConfig, Message, ModelAnswer, ModelId,
    Role, RoundOutcome, RoundOverrides,
};

// Re-export the ModelProvider trait from tundish_core
pub use tundish_core::ModelProvider;

/// Testing utilities for mock providers and strategies.
#[cfg(any(test, feature = "testing"))]
pub mod testing;
