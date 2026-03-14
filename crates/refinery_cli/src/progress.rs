use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comfy_table::presets::NOTHING;
use comfy_table::{Cell, Color, Table};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use tundish_core::ModelId;

/// Spinner character sequence for in-progress models.
const TICK_STRINGS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "];

/// Shared progress display state for the CLI.
///
/// Wraps `indicatif::MultiProgress` for per-model spinners and `comfy-table`
/// for score tables. Thread-safe via internal `Mutex`.
pub struct ProgressDisplay {
    inner: Arc<Mutex<Inner>>,
    multi: MultiProgress,
}

struct Inner {
    /// Per-model progress bars, keyed by model display string.
    bars: HashMap<String, ProgressBar>,
    /// Per-round mean scores accumulated across all rounds.
    round_scores: Vec<HashMap<String, f64>>,
    /// Per-model evaluation scores for the current round (cleared each round).
    current_evals: HashMap<String, Vec<f64>>,
}

impl ProgressDisplay {
    /// Create a new progress display. If `hidden` is true (non-TTY or verbose mode),
    /// all spinners are invisible but the struct is still safe to use.
    pub fn new(hidden: bool) -> Self {
        let multi = if hidden {
            let m = MultiProgress::new();
            m.set_draw_target(ProgressDrawTarget::hidden());
            m
        } else {
            MultiProgress::new() // defaults to stderr
        };
        Self {
            inner: Arc::new(Mutex::new(Inner {
                bars: HashMap::new(),
                round_scores: Vec::new(),
                current_evals: HashMap::new(),
            })),
            multi,
        }
    }

    /// Handle a new round starting: clear previous spinners, print score table.
    pub fn round_started(&self, round: u32, total: u32) {
        let mut inner = self.inner.lock().unwrap();

        // Clear previous round's spinners
        for pb in inner.bars.values() {
            pb.finish_and_clear();
        }
        inner.bars.clear();
        inner.current_evals.clear();

        // Print score table for round 2+
        if !inner.round_scores.is_empty() {
            let table = render_score_table(&inner.round_scores, None);
            let _ = self.multi.println(table);
        }
        let _ = self.multi.println(format!("\n  Round {round}/{total}"));
    }

    /// Handle a phase starting: print phase header, create fresh spinners for models.
    pub fn phase_started(&self, phase: &str, models: &[ModelId]) {
        let mut inner = self.inner.lock().unwrap();

        // Clear any previous phase's spinners
        for pb in inner.bars.values() {
            pb.finish_and_clear();
        }
        inner.bars.clear();

        let _ = self.multi.println(format!("  ── {phase} ──"));

        // Create a spinner for each model
        let style = ProgressStyle::with_template("    {spinner:.dim} {wide_msg}")
            .unwrap()
            .tick_strings(TICK_STRINGS);

        for model in models {
            let pb = self.multi.add(ProgressBar::new_spinner());
            pb.set_style(style.clone());
            pb.set_message(format!("{model}"));
            pb.enable_steady_tick(Duration::from_millis(80));
            inner.bars.insert(model.to_string(), pb);
        }
    }

    /// Update a model's spinner with subprocess output progress.
    pub fn model_output(&self, model: &ModelId, lines: usize, elapsed: Duration) {
        let inner = self.inner.lock().unwrap();
        if let Some(pb) = inner.bars.get(&model.to_string()) {
            pb.set_message(format!("{model} — {lines} lines, {}s", elapsed.as_secs()));
        }
    }

    /// Mark a model's proposal as completed.
    pub fn model_proposed(&self, model: &ModelId, word_count: usize, preview: &str) {
        let inner = self.inner.lock().unwrap();
        let word_label = if word_count == 1 { "word" } else { "words" };
        if let Some(pb) = inner.bars.get(&model.to_string()) {
            pb.finish_with_message(format!(
                "\x1b[32m✓\x1b[0m {model} proposed ({word_count} {word_label}) — \"{preview}\""
            ));
        }
    }

    /// Mark a model's proposal as failed.
    pub fn model_propose_failed(&self, model: &ModelId, error: &str) {
        let inner = self.inner.lock().unwrap();
        if let Some(pb) = inner.bars.get(&model.to_string()) {
            pb.finish_with_message(format!("\x1b[31m✗\x1b[0m {model} failed — {error}"));
        }
    }

    /// Mark an evaluation as completed.
    #[allow(clippy::similar_names)]
    pub fn evaluation_completed(
        &self,
        reviewer: &ModelId,
        reviewee: &ModelId,
        score: f64,
        preview: &str,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .current_evals
            .entry(reviewee.to_string())
            .or_default()
            .push(score);

        // Find the reviewer's bar (or reviewee's if we're using model bars)
        let key = reviewer.to_string();
        if let Some(pb) = inner.bars.get(&key) {
            pb.finish_with_message(format!(
                "\x1b[32m✓\x1b[0m {reviewer} → {reviewee}: {score:.1} — \"{preview}\""
            ));
        }
    }

    /// Mark an evaluation as failed.
    #[allow(clippy::similar_names)]
    pub fn evaluation_failed(&self, reviewer: &ModelId, reviewee: &ModelId, error: &str) {
        let inner = self.inner.lock().unwrap();
        let key = reviewer.to_string();
        if let Some(pb) = inner.bars.get(&key) {
            pb.finish_with_message(format!(
                "\x1b[31m✗\x1b[0m {reviewer} → {reviewee} failed — {error}"
            ));
        }
    }

    /// Print convergence check result and finalize score table.
    #[allow(clippy::too_many_arguments)]
    pub fn convergence_check(
        &self,
        converged: bool,
        winner: Option<&ModelId>,
        best_score: f64,
        threshold: f64,
        stable_rounds: u32,
        required_stable: u32,
    ) {
        let mut inner = self.inner.lock().unwrap();

        let winner_name = winner.map(std::string::ToString::to_string);

        if converged {
            let w = winner_name.as_deref().unwrap_or("?");
            let _ = self.multi.println(format!(
                "  \x1b[32m→ Converged!\x1b[0m Winner: {w} ({best_score:.1} ≥ {threshold:.1}, stable {stable_rounds}/{required_stable})"
            ));
        } else {
            let _ = self.multi.println(format!(
                "  → Not converged ({best_score:.1}/{threshold:.1}, stable {stable_rounds}/{required_stable})"
            ));
        }

        // Finalize current round means into history
        if !inner.current_evals.is_empty() {
            let mut means: HashMap<String, f64> = HashMap::new();
            for (model, scores) in &inner.current_evals {
                #[allow(clippy::cast_precision_loss)]
                let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                means.insert(model.clone(), mean);
            }
            inner.round_scores.push(means);
        }

        // Render score table
        if !inner.round_scores.is_empty() {
            let table = render_score_table(&inner.round_scores, winner_name.as_deref());
            let _ = self.multi.println(table);
        }
    }

    /// Clear all spinners (for final cleanup).
    pub fn finish(&self) {
        let inner = self.inner.lock().unwrap();
        for pb in inner.bars.values() {
            pb.finish_and_clear();
        }
        let _ = self.multi.clear();
    }

    /// Build a tundish progress callback that updates per-model spinners.
    pub fn tundish_callback(&self) -> tundish_core::ProgressFn {
        let display = self.clone_shared();
        Arc::new(move |model: &ModelId, lines: usize, elapsed: Duration| {
            display.model_output(model, lines, elapsed);
        })
    }

    /// Build a consensus progress callback that handles phase-level events.
    pub fn consensus_callback(&self, models: Vec<ModelId>) -> refinery_core::ProgressFn {
        let display = self.clone_shared();
        Arc::new(move |event| {
            use refinery_core::ProgressEvent;
            match event {
                ProgressEvent::RoundStarted { round, total } => {
                    display.round_started(round, total);
                }
                ProgressEvent::PhaseStarted { phase, .. } => {
                    display.phase_started(&phase.to_string(), &models);
                }
                ProgressEvent::ModelProposed {
                    model,
                    word_count,
                    preview,
                } => {
                    display.model_proposed(&model, word_count, &preview);
                }
                ProgressEvent::ModelProposeFailed { model, error } => {
                    display.model_propose_failed(&model, &error);
                }
                ProgressEvent::EvaluationCompleted {
                    reviewer,
                    reviewee,
                    score,
                    preview,
                } => {
                    display.evaluation_completed(&reviewer, &reviewee, score, &preview);
                }
                ProgressEvent::EvaluationFailed {
                    reviewer,
                    reviewee,
                    error,
                } => {
                    display.evaluation_failed(&reviewer, &reviewee, &error);
                }
                ProgressEvent::ConvergenceCheck {
                    converged,
                    winner,
                    best_score,
                    threshold,
                    stable_rounds,
                    required_stable,
                    ..
                } => {
                    display.convergence_check(
                        converged,
                        winner.as_ref(),
                        best_score,
                        threshold,
                        stable_rounds,
                        required_stable,
                    );
                }
            }
        })
    }

    /// Clone the shared state for use in callbacks.
    fn clone_shared(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            multi: self.multi.clone(),
        }
    }
}

/// Render the progressive score table using comfy-table.
fn render_score_table(round_scores: &[HashMap<String, f64>], winner: Option<&str>) -> String {
    let Some(latest) = round_scores.last() else {
        return String::new();
    };

    // Collect all models, sorted by latest round score desc
    let mut models: Vec<&String> = latest.keys().collect();
    models.sort_by(|a, b| {
        latest
            .get(*b)
            .partial_cmp(&latest.get(*a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut table = Table::new();
    table.load_preset(NOTHING);

    // Header: empty cell + R1, R2, ...
    let mut header = vec![Cell::new("")];
    for r in 1..=round_scores.len() {
        header.push(Cell::new(format!("R{r}")).fg(Color::DarkGrey));
    }
    table.set_header(header);

    // One row per model
    for name in &models {
        let is_winner = winner == Some(name.as_str());
        let color = if is_winner {
            Color::Green
        } else {
            Color::Reset
        };

        let mut row = vec![Cell::new(format!("    {name}")).fg(color)];
        for round in round_scores {
            match round.get(*name) {
                Some(score) => {
                    row.push(Cell::new(format!("{score:>4.1}")).fg(color));
                }
                None => {
                    row.push(Cell::new("   -").fg(Color::DarkGrey));
                }
            }
        }
        if is_winner {
            // Append star to last cell
            if let Some(last) = row.last_mut() {
                let mut s = String::new();
                if let Some(score) = latest.get(*name) {
                    write!(s, "{score:>4.1} ★").unwrap();
                }
                *last = Cell::new(s).fg(Color::Green);
            }
        }
        table.add_row(row);
    }

    format!("{table}")
}
