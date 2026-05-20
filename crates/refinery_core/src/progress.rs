use std::sync::Arc;

use crate::types::{ModelId, Phase};

/// Progress events emitted during a consensus run.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// A new round has started.
    RoundStarted { round: u32, total: u32 },

    /// A phase within the current round has started.
    PhaseStarted { round: u32, phase: Phase },

    /// A model successfully produced a proposal.
    ModelProposed {
        model: ModelId,
        word_count: usize,
        preview: String,
    },

    /// A model failed to produce a proposal.
    ModelProposeFailed { model: ModelId, error: String },

    /// An evaluation was completed.
    EvaluationCompleted {
        reviewer: ModelId,
        reviewee: ModelId,
        score: f64,
        preview: String,
    },

    /// An evaluation failed.
    EvaluationFailed {
        reviewer: ModelId,
        reviewee: ModelId,
        error: String,
    },

    /// Convergence check result after the close phase.
    ConvergenceCheck {
        round: u32,
        converged: bool,
        winner: Option<ModelId>,
        best_score: f64,
        threshold: f64,
        stable_rounds: u32,
        required_stable: u32,
    },
}

/// Callback for consensus progress events.
pub type ProgressFn = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

/// Truncate text to `max_chars` with an ellipsis suffix.
///
/// Collapses newlines into spaces so the preview is always single-line.
#[must_use]
pub fn preview(text: &str, max_chars: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed: String = collapsed.chars().take(max_chars).collect();
    if collapsed.chars().count() > max_chars {
        format!("{trimmed}...")
    } else {
        trimmed
    }
}
