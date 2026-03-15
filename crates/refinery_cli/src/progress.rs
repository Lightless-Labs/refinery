use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{IsTerminal as _, Write as _};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comfy_table::presets::NOTHING;
use comfy_table::{Cell, Color, Table};

use tundish_core::ModelId;

const TICK_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Simple frame-based progress display.
///
/// Each tick, clears the previous frame and redraws the current state.
/// No indicatif, no managed bars — just eprint the frame.
pub struct ProgressDisplay {
    inner: Arc<Mutex<Inner>>,
    hidden: bool,
}

struct Inner {
    /// Current round/total.
    round: u32,
    total: u32,
    /// Current phase.
    phase: String,
    /// Per-model status during propose.
    propose_status: Vec<ModelStatus>,
    /// Per-pair status during evaluate.
    eval_status: Vec<EvalStatus>,
    /// Convergence result (set after evaluate).
    convergence: Option<ConvergenceInfo>,
    /// Score table history.
    round_scores: Vec<HashMap<String, f64>>,
    /// Current round evals accumulator.
    current_evals: HashMap<String, Vec<f64>>,
    /// Models that proposed successfully this round.
    proposed_models: Vec<ModelId>,
    /// Permanently dropped models.
    dropped_models: Vec<ModelId>,
    /// Animation frame counter.
    tick: usize,
    /// Number of lines in the last rendered frame.
    last_frame_lines: usize,
}

#[derive(Clone)]
enum ModelStatus {
    Running { model: String, lines: usize, elapsed: Duration },
    Done { model: String, word_count: usize, preview: String },
    Failed { model: String, error: String },
}

#[derive(Clone)]
struct EvalStatus {
    key: String,
    done: bool,
    result: Option<String>,
}

#[derive(Clone)]
struct ConvergenceInfo {
    converged: bool,
    winner: Option<String>,
    best_score: f64,
    threshold: f64,
    stable_rounds: u32,
    required_stable: u32,
}

impl ProgressDisplay {
    pub fn new(hidden: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                round: 0,
                total: 0,
                phase: String::new(),
                propose_status: Vec::new(),
                eval_status: Vec::new(),
                convergence: None,
                round_scores: Vec::new(),
                current_evals: HashMap::new(),
                proposed_models: Vec::new(),
                dropped_models: Vec::new(),
                tick: 0,
                last_frame_lines: 0,
            })),
            hidden,
        }
    }

    fn redraw(&self) {
        if self.hidden {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.tick += 1;

        let frame = Self::render_frame(&inner);

        // Get terminal width to truncate lines (prevents wrapping which breaks cursor math)
        let term_width = terminal_width();
        let lines: Vec<&str> = frame.lines().collect();
        let new_lines = lines.len();

        // Move cursor up to erase previous frame
        if inner.last_frame_lines > 0 {
            eprint!("\x1b[{}A", inner.last_frame_lines);
        }
        // Clear each line and print new frame, truncated to terminal width
        #[allow(clippy::print_with_newline)]
        for line in &lines {
            // Strip ANSI codes when measuring visible width
            let visible_len = strip_ansi_len(line);
            if visible_len > term_width {
                // Truncate at byte boundary near term_width visible chars
                let truncated = truncate_to_width(line, term_width.saturating_sub(3));
                eprint!("\x1b[2K{truncated}...\n");
            } else {
                eprint!("\x1b[2K{line}\n");
            }
        }
        let _ = std::io::stderr().flush();
        inner.last_frame_lines = new_lines;
    }

    fn render_frame(inner: &Inner) -> String {
        let mut out = String::new();

        // Score table from previous rounds
        if !inner.round_scores.is_empty() {
            let winner = inner.convergence.as_ref().and_then(|c| c.winner.as_deref());
            let table = render_score_table(&inner.round_scores, winner);
            out.push_str(&table);
            out.push('\n');
        }

        if inner.round == 0 {
            return out;
        }

        // Round header
        let _ = writeln!(out, "\n  Round {}/{}", inner.round, inner.total);

        // Propose phase
        if inner.phase == "propose" || !inner.propose_status.is_empty() {
            let _ = writeln!(out, "  ── propose ──");
            for status in &inner.propose_status {
                match status {
                    ModelStatus::Running { model, lines, elapsed } => {
                        let spin = TICK_CHARS[inner.tick % TICK_CHARS.len()];
                        if *lines > 0 {
                            let _ = writeln!(out, "    {spin} {model} — {lines} lines, {}s", elapsed.as_secs());
                        } else {
                            let _ = writeln!(out, "    {spin} {model}");
                        }
                    }
                    ModelStatus::Done { model, word_count, preview } => {
                        let word_label = if *word_count == 1 { "word" } else { "words" };
                        let _ = writeln!(out, "    \x1b[32m✓\x1b[0m {model} proposed ({word_count} {word_label}) — \"{preview}\"");
                    }
                    ModelStatus::Failed { model, error } => {
                        let _ = writeln!(out, "    \x1b[31m✗\x1b[0m {model} failed — {error}");
                    }
                }
            }
        }

        // Evaluate phase
        if inner.phase == "evaluate" || !inner.eval_status.is_empty() {
            let _ = writeln!(out, "  ── evaluate ──");
            for status in &inner.eval_status {
                if status.done {
                    if let Some(ref result) = status.result {
                        let _ = writeln!(out, "    {result}");
                    }
                } else {
                    let spin = TICK_CHARS[inner.tick % TICK_CHARS.len()];
                    let _ = writeln!(out, "    {spin} {}", status.key);
                }
            }
        }

        // Convergence
        if let Some(ref conv) = inner.convergence {
            if conv.converged {
                let w = conv.winner.as_deref().unwrap_or("?");
                let _ = writeln!(out,
                    "  \x1b[32m→ Converged!\x1b[0m Winner: {w} ({:.1} ≥ {:.1}, stable {}/{})",
                    conv.best_score, conv.threshold, conv.stable_rounds, conv.required_stable
                );
            } else {
                let _ = writeln!(out,
                    "  → Not converged ({:.1}/{:.1}, stable {}/{})",
                    conv.best_score, conv.threshold, conv.stable_rounds, conv.required_stable
                );
            }
        }

        out
    }

    pub fn round_started(&self, round: u32, total: u32) {
        let mut inner = self.inner.lock().unwrap();
        // Erase previous frame before resetting state
        if inner.last_frame_lines > 0 {
            eprint!("\x1b[{}A", inner.last_frame_lines);
            #[allow(clippy::print_with_newline)]
            for _ in 0..inner.last_frame_lines {
                eprint!("\x1b[2K\n");
            }
            eprint!("\x1b[{}A", inner.last_frame_lines);
            let _ = std::io::stderr().flush();
            inner.last_frame_lines = 0;
        }
        inner.round = round;
        inner.total = total;
        inner.phase.clear();
        inner.propose_status.clear();
        inner.eval_status.clear();
        inner.convergence = None;
        inner.current_evals.clear();
        inner.proposed_models.clear();
        drop(inner);
        self.redraw();
    }

    pub fn phase_started(&self, phase: &str, models: &[ModelId]) {
        let mut inner = self.inner.lock().unwrap();
        inner.phase = phase.to_string();

        if phase == "propose" {
            inner.propose_status = models
                .iter()
                .filter(|m| !inner.dropped_models.contains(m))
                .map(|m| ModelStatus::Running {
                    model: m.to_string(),
                    lines: 0,
                    elapsed: Duration::ZERO,
                })
                .collect();
        } else if phase == "evaluate" {
            let active = inner.proposed_models.clone();
            inner.eval_status = active
                .iter()
                .flat_map(|reviewer| {
                    active.iter().filter(move |reviewee| *reviewee != reviewer).map(move |reviewee| {
                        EvalStatus {
                            key: format!("{reviewer} → {reviewee}"),
                            done: false,
                            result: None,
                        }
                    })
                })
                .collect();
        }
        drop(inner);
        self.redraw();
    }

    pub fn model_output(&self, model: &ModelId, lines: usize, elapsed: Duration) {
        let mut inner = self.inner.lock().unwrap();
        if inner.phase != "propose" {
            return;
        }
        let key = model.to_string();
        for status in &mut inner.propose_status {
            if let ModelStatus::Running { model: m, .. } = status {
                if *m == key {
                    *status = ModelStatus::Running { model: key.clone(), lines, elapsed };
                    break;
                }
            }
        }
        drop(inner);
        self.redraw();
    }

    pub fn model_proposed(&self, model: &ModelId, word_count: usize, preview: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.proposed_models.push(model.clone());
        let key = model.to_string();
        for status in &mut inner.propose_status {
            if matches!(status, ModelStatus::Running { model: m, .. } if *m == key) {
                *status = ModelStatus::Done {
                    model: key.clone(),
                    word_count,
                    preview: preview.to_string(),
                };
                break;
            }
        }
        drop(inner);
        self.redraw();
    }

    pub fn model_propose_failed(&self, model: &ModelId, error: &str) {
        let mut inner = self.inner.lock().unwrap();
        let is_permanent = error.contains("process failed")
            || error.contains("not found")
            || error.contains("not supported")
            || error.contains("credential");
        if is_permanent && !inner.dropped_models.contains(model) {
            inner.dropped_models.push(model.clone());
        }
        let key = model.to_string();
        for status in &mut inner.propose_status {
            if matches!(status, ModelStatus::Running { model: m, .. } if *m == key) {
                *status = ModelStatus::Failed {
                    model: key.clone(),
                    error: error.to_string(),
                };
                break;
            }
        }
        drop(inner);
        self.redraw();
    }

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
        let key = format!("{reviewer} → {reviewee}");
        for status in &mut inner.eval_status {
            if status.key == key {
                status.done = true;
                status.result = Some(format!(
                    "\x1b[32m✓\x1b[0m {reviewer} → {reviewee}: {score:.1} — \"{preview}\""
                ));
                break;
            }
        }
        drop(inner);
        self.redraw();
    }

    #[allow(clippy::similar_names)]
    pub fn evaluation_failed(&self, reviewer: &ModelId, reviewee: &ModelId, error: &str) {
        let mut inner = self.inner.lock().unwrap();
        let key = format!("{reviewer} → {reviewee}");
        for status in &mut inner.eval_status {
            if status.key == key {
                status.done = true;
                status.result = Some(format!(
                    "\x1b[31m✗\x1b[0m {reviewer} → {reviewee} failed — {error}"
                ));
                break;
            }
        }
        drop(inner);
        self.redraw();
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
        let mut inner = self.inner.lock().unwrap();
        let winner_name = winner.map(std::string::ToString::to_string);

        inner.convergence = Some(ConvergenceInfo {
            converged,
            winner: winner_name.clone(),
            best_score,
            threshold,
            stable_rounds,
            required_stable,
        });

        // Finalize scores
        if !inner.current_evals.is_empty() {
            let mut means: HashMap<String, f64> = HashMap::new();
            for (model, scores) in &inner.current_evals {
                #[allow(clippy::cast_precision_loss)]
                let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                means.insert(model.clone(), mean);
            }
            inner.round_scores.push(means);
        }
        drop(inner);
        self.redraw();
    }

    pub fn finish(&self) {
        if self.hidden {
            return;
        }
        // Print final frame without clearing (so it stays in scrollback)
        let inner = self.inner.lock().unwrap();
        if inner.last_frame_lines > 0 {
            // Already displayed — leave it
        }
    }

    /// Start the background tick task for spinner animation.
    pub fn start_tick(&self) -> Option<tokio::task::JoinHandle<()>> {
        if self.hidden || !std::io::stderr().is_terminal() {
            return None;
        }
        let display = self.clone_shared();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(80)).await;
                display.redraw();
            }
        }))
    }

    pub fn tundish_callback(&self) -> tundish_core::ProgressFn {
        let display = self.clone_shared();
        Arc::new(move |model: &ModelId, lines: usize, elapsed: Duration| {
            display.model_output(model, lines, elapsed);
        })
    }

    pub fn consensus_callback(&self, models: Vec<ModelId>) -> refinery_core::ProgressFn {
        let display = self.clone_shared();
        Arc::new(move |event| {
            use refinery_core::ProgressEvent;
            match event {
                ProgressEvent::RoundStarted { round, total } => display.round_started(round, total),
                ProgressEvent::PhaseStarted { phase, .. } => {
                    display.phase_started(&phase.to_string(), &models);
                }
                ProgressEvent::ModelProposed { model, word_count, preview } => {
                    display.model_proposed(&model, word_count, &preview);
                }
                ProgressEvent::ModelProposeFailed { model, error } => {
                    display.model_propose_failed(&model, &error);
                }
                ProgressEvent::EvaluationCompleted { reviewer, reviewee, score, preview } => {
                    display.evaluation_completed(&reviewer, &reviewee, score, &preview);
                }
                ProgressEvent::EvaluationFailed { reviewer, reviewee, error } => {
                    display.evaluation_failed(&reviewer, &reviewee, &error);
                }
                ProgressEvent::ConvergenceCheck {
                    converged, winner, best_score, threshold, stable_rounds, required_stable, ..
                } => {
                    display.convergence_check(
                        converged, winner.as_ref(), best_score, threshold, stable_rounds, required_stable,
                    );
                }
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

/// Get terminal width, defaulting to 120 if unknown.
fn terminal_width() -> usize {
    // Use libc ioctl to get terminal width
    #[cfg(unix)]
    {
        #[allow(unsafe_code)]
        unsafe {
            let mut winsize: libc::winsize = std::mem::zeroed();
            if libc::ioctl(2, libc::TIOCGWINSZ, &raw mut winsize) == 0 && winsize.ws_col > 0 {
                return winsize.ws_col as usize;
            }
        }
    }
    120
}

/// Count visible characters (excluding ANSI escape sequences).
fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            len += 1;
        }
    }
    len
}

/// Truncate a string to approximately `max_visible` visible characters,
/// preserving ANSI escape sequences.
fn truncate_to_width(s: &str, max_visible: usize) -> &str {
    let mut visible = 0;
    let mut in_escape = false;
    for (i, c) in s.char_indices() {
        if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            visible += 1;
            if visible >= max_visible {
                return &s[..i + c.len_utf8()];
            }
        }
    }
    s
}

fn render_score_table(round_scores: &[HashMap<String, f64>], winner: Option<&str>) -> String {
    let Some(latest) = round_scores.last() else {
        return String::new();
    };

    let mut models: Vec<&String> = latest.keys().collect();
    models.sort_by(|a, b| {
        latest
            .get(*b)
            .partial_cmp(&latest.get(*a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.enforce_styling();

    let mut header = vec![Cell::new("")];
    for r in 1..=round_scores.len() {
        header.push(Cell::new(format!("R{r}")).fg(Color::DarkGrey));
    }
    table.set_header(header);

    for name in &models {
        let is_winner = winner == Some(name.as_str());
        let color = if is_winner { Color::Green } else { Color::Reset };

        let mut row = vec![Cell::new(format!("    {name}")).fg(color)];
        for round in round_scores {
            match round.get(*name) {
                Some(score) => row.push(Cell::new(format!("{score:>4.1}")).fg(color)),
                None => row.push(Cell::new("   -").fg(Color::DarkGrey)),
            }
        }
        if is_winner {
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

    table.trim_fmt()
}
