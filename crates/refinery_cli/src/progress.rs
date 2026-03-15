use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::IsTerminal as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tundish_core::ModelId;

const SPIN: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Simple progress display. Prints events to stderr as they happen.
/// A background tick task animates the spinner for the current in-progress model.
pub struct ProgressDisplay {
    inner: Arc<Mutex<Inner>>,
    hidden: bool,
}

struct Inner {
    label: Option<String>,
    frame: usize,
    current_evals: HashMap<String, Vec<f64>>,
    round_scores: Vec<HashMap<String, f64>>,
    proposed: Vec<ModelId>,
    dropped: Vec<ModelId>,
}

impl ProgressDisplay {
    pub fn new(hidden: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                label: None,
                frame: 0,
                current_evals: HashMap::new(),
                round_scores: Vec::new(),
                proposed: Vec::new(),
                dropped: Vec::new(),
            })),
            hidden,
        }
    }

    pub fn start_tick(&self) -> Option<tokio::task::JoinHandle<()>> {
        if self.hidden || !std::io::stderr().is_terminal() {
            return None;
        }
        let inner = self.inner.clone();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(80)).await;
                let mut s = inner.lock().unwrap();
                if let Some(ref label) = s.label {
                    let spin = SPIN[s.frame % SPIN.len()];
                    eprint!("\r\x1b[2K    {spin} {label}");
                    s.frame += 1;
                }
            }
        }))
    }

    pub fn round_started(&self, round: u32, total: u32) {
        let mut s = self.inner.lock().unwrap();
        s.label = None;
        s.current_evals.clear();
        s.proposed.clear();
        eprint!("\r\x1b[2K");
        eprintln!("\n  Round {round}/{total}");
    }

    pub fn phase_started(&self, phase: &str, _models: &[ModelId]) {
        let mut s = self.inner.lock().unwrap();
        s.label = None;
        eprint!("\r\x1b[2K");
        eprintln!("  ── {phase} ──");
    }

    pub fn model_proposed(&self, model: &ModelId, word_count: usize, preview: &str) {
        let mut s = self.inner.lock().unwrap();
        s.proposed.push(model.clone());
        s.label = None;
        let w = if word_count == 1 { "word" } else { "words" };
        eprintln!(
            "\r\x1b[2K    \x1b[32m✓\x1b[0m {model} proposed ({word_count} {w}) — \"{preview}\""
        );
    }

    pub fn model_propose_failed(&self, model: &ModelId, error: &str) {
        let mut s = self.inner.lock().unwrap();
        let permanent = error.contains("process failed")
            || error.contains("not found")
            || error.contains("not supported")
            || error.contains("credential");
        if permanent && !s.dropped.contains(model) {
            s.dropped.push(model.clone());
        }
        s.label = None;
        eprintln!("\r\x1b[2K    \x1b[31m✗\x1b[0m {model} failed — {error}");
    }

    #[allow(clippy::similar_names)]
    pub fn evaluation_completed(
        &self,
        reviewer: &ModelId,
        reviewee: &ModelId,
        score: f64,
        preview: &str,
    ) {
        let mut s = self.inner.lock().unwrap();
        s.current_evals
            .entry(reviewee.to_string())
            .or_default()
            .push(score);
        s.label = None;
        eprintln!(
            "\r\x1b[2K    \x1b[32m✓\x1b[0m {reviewer} → {reviewee}: {score:.1} — \"{preview}\""
        );
    }

    #[allow(clippy::similar_names)]
    pub fn evaluation_failed(&self, reviewer: &ModelId, reviewee: &ModelId, error: &str) {
        let mut s = self.inner.lock().unwrap();
        s.label = None;
        eprintln!("\r\x1b[2K    \x1b[31m✗\x1b[0m {reviewer} → {reviewee} failed — {error}");
    }

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
        let mut s = self.inner.lock().unwrap();
        s.label = None;
        eprint!("\r\x1b[2K");

        let winner_name = winner.map(std::string::ToString::to_string);
        if converged {
            let w = winner_name.as_deref().unwrap_or("?");
            eprintln!(
                "  \x1b[32m→ Converged!\x1b[0m Winner: {w} ({best_score:.1} ≥ {threshold:.1}, stable {stable_rounds}/{required_stable})"
            );
        } else {
            eprintln!(
                "  → Not converged ({best_score:.1}/{threshold:.1}, stable {stable_rounds}/{required_stable})"
            );
        }

        // Finalize current round means
        if !s.current_evals.is_empty() {
            let mut means: HashMap<String, f64> = HashMap::new();
            for (model, scores) in &s.current_evals {
                #[allow(clippy::cast_precision_loss)]
                let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                means.insert(model.clone(), mean);
            }
            s.round_scores.push(means);
        }

        // Render score table
        if !s.round_scores.is_empty() {
            let latest = s.round_scores.last().unwrap();
            let mut models: Vec<&String> = latest.keys().collect();
            models.sort_by(|a, b| {
                latest
                    .get(*b)
                    .partial_cmp(&latest.get(*a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let name_w = models.iter().map(|n| n.len()).max().unwrap_or(0);
            let num_rounds = s.round_scores.len();

            let mut header = format!("    {:<name_w$}", "");
            for r in 1..=num_rounds {
                let _ = write!(header, "  R{r:<3}");
            }
            eprintln!("\x1b[2m{header}\x1b[0m");

            for name in &models {
                let is_winner = winner_name.as_deref() == Some(name.as_str());
                let mut row = if is_winner {
                    format!("    \x1b[32m{name:<name_w$}")
                } else {
                    format!("    {name:<name_w$}")
                };
                for round in &s.round_scores {
                    match round.get(*name) {
                        Some(score) => {
                            let _ = write!(row, "  {score:>4.1}");
                        }
                        None => row.push_str("     -"),
                    }
                }
                if is_winner {
                    row.push_str(" ★\x1b[0m");
                }
                eprintln!("{row}");
            }
        }
    }

    pub fn finish(&self) {
        let mut s = self.inner.lock().unwrap();
        s.label = None;
        eprint!("\r\x1b[2K");
    }

    pub fn tundish_callback(&self) -> tundish_core::ProgressFn {
        let inner = self.inner.clone();
        let hidden = self.hidden;
        Arc::new(move |model: &ModelId, lines: usize, elapsed: Duration| {
            if hidden {
                return;
            }
            let mut s = inner.lock().unwrap();
            if s.label.as_deref().is_none_or(|l| l.starts_with(&model.to_string())) {
                s.label = Some(format!("{model} — {lines} lines, {}s", elapsed.as_secs()));
            }
        })
    }

    pub fn consensus_callback(&self, models: Vec<ModelId>) -> refinery_core::ProgressFn {
        let d = self.clone_shared();
        Arc::new(move |ev| {
            use refinery_core::ProgressEvent;
            match ev {
                ProgressEvent::RoundStarted { round, total } => d.round_started(round, total),
                ProgressEvent::PhaseStarted { phase, .. } => {
                    d.phase_started(&phase.to_string(), &models);
                }
                ProgressEvent::ModelProposed {
                    model,
                    word_count,
                    preview,
                } => d.model_proposed(&model, word_count, &preview),
                ProgressEvent::ModelProposeFailed { model, error } => {
                    d.model_propose_failed(&model, &error);
                }
                ProgressEvent::EvaluationCompleted {
                    reviewer,
                    reviewee,
                    score,
                    preview,
                } => d.evaluation_completed(&reviewer, &reviewee, score, &preview),
                ProgressEvent::EvaluationFailed {
                    reviewer,
                    reviewee,
                    error,
                } => d.evaluation_failed(&reviewer, &reviewee, &error),
                ProgressEvent::ConvergenceCheck {
                    converged,
                    winner,
                    best_score,
                    threshold,
                    stable_rounds,
                    required_stable,
                    ..
                } => d.convergence_check(
                    converged,
                    winner.as_ref(),
                    best_score,
                    threshold,
                    stable_rounds,
                    required_stable,
                ),
            }
        })
    }

    fn clone_shared(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            hidden: self.hidden,
        }
    }
}
