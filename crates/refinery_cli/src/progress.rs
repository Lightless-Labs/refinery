use std::collections::HashMap;
use std::io::{self, Stderr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use ratatui::{Terminal, TerminalOptions, Viewport};

use tundish_core::ModelId;

const SPIN: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct ProgressDisplay {
    s: Arc<Mutex<St>>,
    t: Arc<Mutex<Option<Terminal<CrosstermBackend<Stderr>>>>>,
    hidden: bool,
}

/// All mutable state, updated by callbacks, read by renderer.
struct St {
    round: u32,
    total: u32,
    phase: String,
    models: Vec<(String, Status)>, // propose phase
    evals: Vec<(String, Status)>,  // evaluate phase
    scores: Vec<HashMap<String, f64>>,
    cur_evals: HashMap<String, Vec<f64>>,
    proposed: Vec<ModelId>,
    dropped: Vec<ModelId>,
    conv: Option<(bool, Option<String>, f64, f64, u32, u32)>,
    tick: usize,
}

#[derive(Clone)]
enum Status {
    Run(usize, Duration),                   // lines, elapsed
    Ok(usize, String),                      // word_count, preview
    Fail(String),                           // error
    EvalOk(f64, String),                    // score, preview
    EvalFail(String),                       // error
}

impl ProgressDisplay {
    pub fn new(hidden: bool) -> Self {
        let t = if hidden { None } else {
            enable_raw_mode().ok().and_then(|()| {
                Terminal::with_options(
                    CrosstermBackend::new(io::stderr()),
                    TerminalOptions { viewport: Viewport::Inline(1) },
                ).ok()
            })
        };
        Self {
            s: Arc::new(Mutex::new(St {
                round: 0, total: 0, phase: String::new(),
                models: vec![], evals: vec![], scores: vec![],
                cur_evals: HashMap::new(), proposed: vec![],
                dropped: vec![], conv: None, tick: 0,
            })),
            t: Arc::new(Mutex::new(t)),
            hidden,
        }
    }

    fn draw(&self) {
        let s = self.s.lock().unwrap();
        let mut tg = self.t.lock().unwrap();
        let Some(t) = tg.as_mut() else { return };
        let lines = build(&s);
        #[allow(clippy::cast_possible_truncation)]
        let h = lines.len().max(1) as u16;
        if let Ok(sz) = t.size() { if sz.height != h { let _ = t.resize(Rect::new(0, 0, sz.width, h)); } }
        let _ = t.draw(|f| {
            let text: Text = lines.into_iter().map(Line::from).collect::<Vec<_>>().into();
            f.render_widget(Paragraph::new(text), f.area());
        });
    }

    fn flush(&self) {
        let s = self.s.lock().unwrap();
        let mut tg = self.t.lock().unwrap();
        let Some(t) = tg.as_mut() else { return };
        let lines = build(&s);
        for spans in &lines {
            let _ = t.insert_before(1, |buf| {
                Paragraph::new(Line::from(spans.clone())).render(buf.area, buf);
            });
        }
        let w = t.size().map(|sz| sz.width).unwrap_or(80);
        let _ = t.resize(Rect::new(0, 0, w, 1));
        let _ = t.draw(|f| f.render_widget(Paragraph::new(""), f.area()));
    }

    // ── events ──

    pub fn round_started(&self, round: u32, total: u32) {
        self.flush();
        let mut s = self.s.lock().unwrap();
        s.round = round; s.total = total; s.phase.clear();
        s.models.clear(); s.evals.clear(); s.conv = None;
        s.cur_evals.clear(); s.proposed.clear();
        drop(s); self.draw();
    }

    pub fn phase_started(&self, phase: &str, models: &[ModelId]) {
        let mut s = self.s.lock().unwrap();
        s.phase = phase.into();
        if phase == "propose" {
            s.models = models.iter()
                .filter(|m| !s.dropped.contains(m))
                .map(|m| (m.to_string(), Status::Run(0, Duration::ZERO)))
                .collect();
        } else if phase == "evaluate" {
            let a = s.proposed.clone();
            s.evals = a.iter().flat_map(|r| a.iter().filter(move |e| *e != r)
                .map(move |e| (format!("{r} → {e}"), Status::Run(0, Duration::ZERO))))
                .collect();
        }
        drop(s); self.draw();
    }

    pub fn model_output(&self, model: &ModelId, lines: usize, elapsed: Duration) {
        let mut s = self.s.lock().unwrap();
        if s.phase != "propose" { return; }
        let k = model.to_string();
        if let Some(m) = s.models.iter_mut().find(|(n, _)| *n == k) {
            if matches!(m.1, Status::Run(..)) { m.1 = Status::Run(lines, elapsed); }
        }
    }

    pub fn model_proposed(&self, model: &ModelId, wc: usize, preview: &str) {
        let mut s = self.s.lock().unwrap();
        s.proposed.push(model.clone());
        let k = model.to_string();
        if let Some(m) = s.models.iter_mut().find(|(n, _)| *n == k) {
            m.1 = Status::Ok(wc, preview.into());
        }
        drop(s); self.draw();
    }

    pub fn model_propose_failed(&self, model: &ModelId, error: &str) {
        let mut s = self.s.lock().unwrap();
        if (error.contains("process failed") || error.contains("not found")
            || error.contains("not supported")) && !s.dropped.contains(model) {
            s.dropped.push(model.clone());
        }
        let k = model.to_string();
        if let Some(m) = s.models.iter_mut().find(|(n, _)| *n == k) {
            m.1 = Status::Fail(error.into());
        }
        drop(s); self.draw();
    }

    #[allow(clippy::similar_names)]
    pub fn evaluation_completed(&self, reviewer: &ModelId, reviewee: &ModelId, score: f64, preview: &str) {
        let mut s = self.s.lock().unwrap();
        s.cur_evals.entry(reviewee.to_string()).or_default().push(score);
        let k = format!("{reviewer} → {reviewee}");
        if let Some(e) = s.evals.iter_mut().find(|(p, _)| *p == k) {
            e.1 = Status::EvalOk(score, preview.into());
        }
        drop(s); self.draw();
    }

    #[allow(clippy::similar_names)]
    pub fn evaluation_failed(&self, reviewer: &ModelId, reviewee: &ModelId, error: &str) {
        let mut s = self.s.lock().unwrap();
        let k = format!("{reviewer} → {reviewee}");
        if let Some(e) = s.evals.iter_mut().find(|(p, _)| *p == k) {
            e.1 = Status::EvalFail(format!("{k} failed — {error}"));
        }
        drop(s); self.draw();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn convergence_check(&self, converged: bool, winner: Option<&ModelId>, bs: f64, th: f64, sr: u32, rr: u32) {
        let mut s = self.s.lock().unwrap();
        s.conv = Some((converged, winner.map(std::string::ToString::to_string), bs, th, sr, rr));
        if !s.cur_evals.is_empty() {
            #[allow(clippy::cast_precision_loss)]
            let means: HashMap<String, f64> = s.cur_evals.iter()
                .map(|(m, sc)| (m.clone(), sc.iter().sum::<f64>() / sc.len() as f64))
                .collect();
            s.scores.push(means);
        }
        drop(s); self.draw();
    }

    pub fn finish(&self) { self.flush(); let mut tg = self.t.lock().unwrap(); *tg = None; let _ = disable_raw_mode(); }
    pub fn start_tick(&self) -> Option<tokio::task::JoinHandle<()>> {
        if self.hidden { return None; }
        let d = self.clone_shared();
        Some(tokio::spawn(async move { loop {
            tokio::time::sleep(Duration::from_millis(80)).await;
            d.s.lock().unwrap().tick += 1;
            d.draw();
        }}))
    }
    pub fn tundish_callback(&self) -> tundish_core::ProgressFn {
        let d = self.clone_shared();
        Arc::new(move |m: &ModelId, l: usize, e: Duration| d.model_output(m, l, e))
    }
    pub fn consensus_callback(&self, models: Vec<ModelId>) -> refinery_core::ProgressFn {
        let d = self.clone_shared();
        Arc::new(move |ev| { use refinery_core::ProgressEvent; match ev {
            ProgressEvent::RoundStarted { round, total } => d.round_started(round, total),
            ProgressEvent::PhaseStarted { phase, .. } => d.phase_started(&phase.to_string(), &models),
            ProgressEvent::ModelProposed { model, word_count, preview } => d.model_proposed(&model, word_count, &preview),
            ProgressEvent::ModelProposeFailed { model, error } => d.model_propose_failed(&model, &error),
            ProgressEvent::EvaluationCompleted { reviewer, reviewee, score, preview } => d.evaluation_completed(&reviewer, &reviewee, score, &preview),
            ProgressEvent::EvaluationFailed { reviewer, reviewee, error } => d.evaluation_failed(&reviewer, &reviewee, &error),
            ProgressEvent::ConvergenceCheck { converged, winner, best_score, threshold, stable_rounds, required_stable, .. } =>
                d.convergence_check(converged, winner.as_ref(), best_score, threshold, stable_rounds, required_stable),
        }})
    }
    fn clone_shared(&self) -> Self { Self { s: self.s.clone(), t: self.t.clone(), hidden: self.hidden } }
}

// ── frame builder ──

#[allow(clippy::cast_possible_truncation)]
fn build(s: &St) -> Vec<Vec<Span<'static>>> {
    let mut out: Vec<Vec<Span>> = vec![];
    if s.round == 0 { out.push(vec![Span::raw("")]); return out; }

    // Score table
    if !s.scores.is_empty() {
        let w = s.conv.as_ref().and_then(|c| c.1.as_deref());
        let latest = s.scores.last().unwrap();
        let mut ms: Vec<&String> = latest.keys().collect();
        ms.sort_by(|a, b| latest.get(*b).partial_cmp(&latest.get(*a)).unwrap_or(std::cmp::Ordering::Equal));
        let mut hdr = vec![Span::styled(format!("{:<40}", ""), Style::new().dark_gray())];
        for r in 1..=s.scores.len() { hdr.push(Span::styled(format!("{:>6}", format!("R{r}")), Style::new().dark_gray())); }
        out.push(hdr);
        for name in &ms {
            let win = w == Some(name.as_str());
            let st = if win { Style::new().green() } else { Style::new() };
            let mut row = vec![Span::styled(format!("    {name:<36}"), st)];
            for rd in &s.scores {
                match rd.get(*name) {
                    Some(sc) => row.push(Span::styled(format!("{sc:>6.1}"), st)),
                    None => row.push(Span::styled(format!("{:>6}", "-"), Style::new().dark_gray())),
                }
            }
            if win { row.push(Span::styled(" ★", Style::new().green())); }
            out.push(row);
        }
    }

    out.push(vec![Span::raw(format!("\n  Round {}/{}", s.round, s.total))]);

    if !s.models.is_empty() {
        out.push(vec![Span::raw("  ── propose ──")]);
        for (name, st) in &s.models {
            out.push(match st {
                Status::Run(l, e) => { let sp = SPIN[s.tick % SPIN.len()]; vec![Span::styled(
                    if *l > 0 { format!("    {sp} {name} — {l} lines, {}s", e.as_secs()) }
                    else { format!("    {sp} {name}") }, Style::new().dim())] },
                Status::Ok(wc, pv) => { let w = if *wc == 1 { "word" } else { "words" };
                    vec![Span::styled("    ✓ ", Style::new().green()), Span::raw(format!("{name} proposed ({wc} {w}) — \"{pv}\""))] },
                Status::Fail(err) => vec![Span::styled("    ✗ ", Style::new().red()), Span::raw(format!("{name} failed — {err}"))],
                _ => vec![],
            });
        }
    }

    if !s.evals.is_empty() {
        out.push(vec![Span::raw("  ── evaluate ──")]);
        for (pair, st) in &s.evals {
            out.push(match st {
                Status::Run(..) => { let sp = SPIN[s.tick % SPIN.len()];
                    vec![Span::styled(format!("    {sp} {pair}"), Style::new().dim())] },
                Status::EvalOk(sc, pv) => vec![Span::styled("    ✓ ", Style::new().green()), Span::raw(format!("{pair}: {sc:.1} — \"{pv}\""))],
                Status::EvalFail(msg) => vec![Span::styled("    ✗ ", Style::new().red()), Span::raw(msg.clone())],
                _ => vec![],
            });
        }
    }

    if let Some((conv, ref w, bs, th, sr, rr)) = s.conv {
        if conv {
            let wn = w.as_deref().unwrap_or("?");
            out.push(vec![Span::styled("  → Converged! ", Style::new().green()), Span::raw(format!("Winner: {wn} ({bs:.1} ≥ {th:.1}, stable {sr}/{rr})"))]);
        } else {
            out.push(vec![Span::raw(format!("  → Not converged ({bs:.1}/{th:.1}, stable {sr}/{rr})"))]);
        }
    }

    out
}
