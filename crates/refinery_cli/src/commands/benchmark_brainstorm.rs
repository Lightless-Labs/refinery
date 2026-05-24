use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use serde::{Deserialize, Serialize};

use refinery_core::scoring;
use refinery_core::types::ModelId;

use super::common::OutputFormat;

#[derive(Parser, Debug)]
pub struct BenchmarkBrainstormArgs {
    /// Brainstorm artifact run directories to analyze.
    #[arg(value_name = "RUN_DIR")]
    run_dirs: Vec<PathBuf>,

    /// Number of candidates each selector should return [default: 3].
    #[arg(long, default_value = "3", value_parser = clap::value_parser!(u32).range(1..=20))]
    panel_size: u32,

    /// Output format [text|json].
    #[arg(short, long, default_value = "json")]
    output_format: OutputFormat,
}

#[derive(Debug)]
struct Candidate {
    model_id: ModelId,
    answer: String,
    mean_score: f64,
    stddev: f64,
    controversy_score: f64,
    per_evaluator_scores: Vec<(ModelId, f64)>,
}

#[derive(Debug, Serialize)]
struct BenchmarkOutput {
    status: String,
    runs: Vec<RunBenchmarkOutput>,
}

#[derive(Debug, Serialize)]
struct RunBenchmarkOutput {
    run_dir: String,
    final_round: u32,
    candidate_count: usize,
    selectors: Vec<SelectorOutput>,
}

#[derive(Debug, Serialize)]
struct SelectorOutput {
    name: String,
    panel: Vec<CandidateOutput>,
    metrics: PanelMetrics,
}

#[derive(Debug, Serialize)]
struct CandidateOutput {
    model_id: String,
    mean_score: f64,
    stddev: f64,
    controversy_score: f64,
    evaluator_count: usize,
}

#[derive(Debug, Serialize)]
struct PanelMetrics {
    panel_mean_quality: f64,
    panel_min_quality: f64,
    panel_disagreement: f64,
    lexical_overlap: f64,
    meta_preamble_rate: f64,
}

#[derive(Debug, Deserialize)]
struct EvalArtifact {
    evaluator: String,
    evaluatee: String,
    score: f64,
}

pub fn run(args: &BenchmarkBrainstormArgs) -> ExitCode {
    if args.run_dirs.is_empty() {
        eprintln!("Error: at least one brainstorm run directory is required");
        return ExitCode::from(4);
    }

    let mut runs = Vec::new();
    for run_dir in &args.run_dirs {
        match analyze_run(run_dir, args.panel_size as usize) {
            Ok(run) => runs.push(run),
            Err(e) => {
                eprintln!("Error analyzing {}: {e}", run_dir.display());
                return ExitCode::from(1);
            }
        }
    }

    let output = BenchmarkOutput {
        status: "benchmarked".to_string(),
        runs,
    };

    match args.output_format {
        OutputFormat::Json => match serde_json::to_string_pretty(&output) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed to serialize benchmark output: {e}");
                ExitCode::from(1)
            }
        },
        OutputFormat::Text => {
            emit_text(&output);
            ExitCode::SUCCESS
        }
    }
}

fn analyze_run(run_dir: &Path, panel_size: usize) -> Result<RunBenchmarkOutput, String> {
    let final_round = find_final_round(run_dir)?;
    let round_dir = run_dir.join(format!("round-{final_round}"));
    let candidates = load_candidates(&round_dir)?;

    let selectors = vec![
        selector_output("mean", select_by_mean(&candidates, panel_size)),
        selector_output("stddev", select_by_stddev(&candidates, panel_size)),
        selector_output(
            "controversy",
            select_by_controversy(&candidates, panel_size),
        ),
        selector_output(
            "controversy_floor_7",
            select_by_controversy_with_quality_floor(&candidates, panel_size, 7.0),
        ),
        selector_output(
            "quality_x_lexdiv",
            select_by_quality_x_lexdiv(&candidates, panel_size),
        ),
    ];

    Ok(RunBenchmarkOutput {
        run_dir: run_dir.display().to_string(),
        final_round,
        candidate_count: candidates.len(),
        selectors,
    })
}

fn find_final_round(run_dir: &Path) -> Result<u32, String> {
    let entries = std::fs::read_dir(run_dir).map_err(|e| e.to_string())?;
    let mut max_round = None;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("failed to read {} entry: {e}", run_dir.display()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(round) = name
            .strip_prefix("round-")
            .and_then(|suffix| suffix.parse::<u32>().ok())
        {
            max_round = Some(max_round.map_or(round, |current: u32| current.max(round)));
        }
    }
    max_round.ok_or_else(|| "no round-* directories found".to_string())
}

fn load_candidates(round_dir: &Path) -> Result<Vec<Candidate>, String> {
    let mut scores: BTreeMap<ModelId, Vec<(ModelId, f64)>> = BTreeMap::new();

    for entry in std::fs::read_dir(round_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("evaluate-") || !name.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let eval: EvalArtifact = serde_json::from_str(&content)
            .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
        scores
            .entry(ModelId::new(&eval.evaluatee))
            .or_default()
            .push((ModelId::new(&eval.evaluator), eval.score));
    }

    if scores.is_empty() {
        return Err("no evaluation artifacts found".to_string());
    }

    scores
        .into_iter()
        .map(|(model_id, per_evaluator_scores)| {
            let safe_id = model_id.to_string().replace('/', "_");
            let proposal_path = round_dir.join(format!("propose-{safe_id}.md"));
            let answer = std::fs::read_to_string(&proposal_path).map_err(|e| {
                format!(
                    "failed to read proposal for {model_id} at {}: {e}",
                    proposal_path.display()
                )
            })?;
            let score_values: Vec<f64> = per_evaluator_scores.iter().map(|(_, s)| *s).collect();
            let mean_score = scoring::mean(&score_values);
            let stddev = scoring::stddev(&score_values, mean_score);
            let controversy_score = scoring::controversy_score(&score_values);
            Ok(Candidate {
                model_id,
                answer,
                mean_score,
                stddev,
                controversy_score,
                per_evaluator_scores,
            })
        })
        .collect()
}

fn select_by_mean(candidates: &[Candidate], panel_size: usize) -> Vec<&Candidate> {
    let mut selected: Vec<&Candidate> = candidates.iter().collect();
    selected.sort_by(|a, b| {
        b.mean_score
            .total_cmp(&a.mean_score)
            .then_with(|| b.controversy_score.total_cmp(&a.controversy_score))
            .then_with(|| a.model_id.to_string().cmp(&b.model_id.to_string()))
    });
    selected.truncate(panel_size);
    selected
}

fn select_by_stddev(candidates: &[Candidate], panel_size: usize) -> Vec<&Candidate> {
    let mut selected: Vec<&Candidate> = candidates.iter().collect();
    selected.sort_by(|a, b| {
        b.stddev
            .total_cmp(&a.stddev)
            .then_with(|| b.mean_score.total_cmp(&a.mean_score))
            .then_with(|| a.model_id.to_string().cmp(&b.model_id.to_string()))
    });
    selected.truncate(panel_size);
    selected
}

fn select_by_controversy(candidates: &[Candidate], panel_size: usize) -> Vec<&Candidate> {
    let mut selected: Vec<&Candidate> = candidates.iter().collect();
    selected.sort_by(|a, b| {
        b.controversy_score
            .total_cmp(&a.controversy_score)
            .then_with(|| b.mean_score.total_cmp(&a.mean_score))
            .then_with(|| a.model_id.to_string().cmp(&b.model_id.to_string()))
    });
    selected.truncate(panel_size);
    selected
}

fn select_by_controversy_with_quality_floor(
    candidates: &[Candidate],
    panel_size: usize,
    quality_floor: f64,
) -> Vec<&Candidate> {
    let mut selected: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| candidate.mean_score >= quality_floor)
        .collect();
    selected.sort_by(|a, b| {
        b.controversy_score
            .total_cmp(&a.controversy_score)
            .then_with(|| b.mean_score.total_cmp(&a.mean_score))
            .then_with(|| a.model_id.to_string().cmp(&b.model_id.to_string()))
    });

    if selected.len() < panel_size {
        let selected_ids: BTreeSet<ModelId> = selected
            .iter()
            .map(|candidate| candidate.model_id.clone())
            .collect();
        let mut backfill: Vec<&Candidate> = candidates
            .iter()
            .filter(|candidate| !selected_ids.contains(&candidate.model_id))
            .collect();
        backfill.sort_by(|a, b| {
            b.controversy_score
                .total_cmp(&a.controversy_score)
                .then_with(|| b.mean_score.total_cmp(&a.mean_score))
                .then_with(|| a.model_id.to_string().cmp(&b.model_id.to_string()))
        });
        selected.extend(backfill);
    }

    selected.truncate(panel_size);
    selected
}

fn select_by_quality_x_lexdiv(candidates: &[Candidate], panel_size: usize) -> Vec<&Candidate> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut remaining: Vec<&Candidate> = candidates.iter().collect();
    remaining.sort_by(|a, b| {
        b.mean_score
            .total_cmp(&a.mean_score)
            .then_with(|| a.model_id.to_string().cmp(&b.model_id.to_string()))
    });
    selected.push(remaining.remove(0));

    while selected.len() < panel_size && !remaining.is_empty() {
        let best_index = remaining
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                quality_x_lexdiv_score(a, &selected)
                    .total_cmp(&quality_x_lexdiv_score(b, &selected))
                    .then_with(|| a.mean_score.total_cmp(&b.mean_score))
                    .then_with(|| b.model_id.to_string().cmp(&a.model_id.to_string()))
            })
            .map_or(0, |(i, _)| i);
        selected.push(remaining.remove(best_index));
    }

    selected
}

fn quality_x_lexdiv_score(candidate: &Candidate, selected: &[&Candidate]) -> f64 {
    let max_similarity = selected
        .iter()
        .map(|other| lexical_similarity(&candidate.answer, &other.answer))
        .fold(0.0, f64::max);
    (candidate.mean_score / 10.0) * (1.0 - max_similarity)
}

fn selector_output(name: &str, panel: Vec<&Candidate>) -> SelectorOutput {
    SelectorOutput {
        name: name.to_string(),
        metrics: panel_metrics(&panel),
        panel: panel.into_iter().map(candidate_output).collect(),
    }
}

fn candidate_output(candidate: &Candidate) -> CandidateOutput {
    CandidateOutput {
        model_id: candidate.model_id.to_string(),
        mean_score: candidate.mean_score,
        stddev: candidate.stddev,
        controversy_score: candidate.controversy_score,
        evaluator_count: candidate.per_evaluator_scores.len(),
    }
}

fn panel_metrics(panel: &[&Candidate]) -> PanelMetrics {
    if panel.is_empty() {
        return PanelMetrics {
            panel_mean_quality: 0.0,
            panel_min_quality: 0.0,
            panel_disagreement: 0.0,
            lexical_overlap: 0.0,
            meta_preamble_rate: 0.0,
        };
    }

    #[allow(clippy::cast_precision_loss)]
    let count = panel.len() as f64;
    let panel_mean_quality = panel.iter().map(|c| c.mean_score).sum::<f64>() / count;
    let panel_min_quality = panel
        .iter()
        .map(|c| c.mean_score)
        .fold(f64::INFINITY, f64::min);
    let panel_disagreement = panel.iter().map(|c| c.stddev).sum::<f64>() / count;
    let lexical_overlap = average_pairwise_lexical_overlap(panel);
    let meta_count = panel
        .iter()
        .filter(|candidate| has_meta_preamble(&candidate.answer))
        .count();
    #[allow(clippy::cast_precision_loss)]
    let meta_preamble_rate = meta_count as f64 / count;

    PanelMetrics {
        panel_mean_quality,
        panel_min_quality,
        panel_disagreement,
        lexical_overlap,
        meta_preamble_rate,
    }
}

fn average_pairwise_lexical_overlap(panel: &[&Candidate]) -> f64 {
    if panel.len() < 2 {
        return 0.0;
    }

    let mut total = 0.0;
    let mut pair_count = 0_usize;
    for i in 0..panel.len() {
        for j in (i + 1)..panel.len() {
            total += lexical_similarity(&panel[i].answer, &panel[j].answer);
            pair_count += 1;
        }
    }

    #[allow(clippy::cast_precision_loss)]
    {
        total / pair_count as f64
    }
}

fn lexical_similarity(a: &str, b: &str) -> f64 {
    let a_tokens = tokens(a);
    let b_tokens = tokens(b);
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }
    let intersection = a_tokens.intersection(&b_tokens).count();
    let union = a_tokens.union(&b_tokens).count();
    #[allow(clippy::cast_precision_loss)]
    {
        intersection as f64 / union as f64
    }
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '\'')
        .filter_map(|token| {
            let token = token.to_lowercase();
            (token.len() >= 3 && !STOPWORDS.contains(&token.as_str())).then_some(token)
        })
        .collect()
}

fn has_meta_preamble(answer: &str) -> bool {
    let lower = answer.to_lowercase();
    [
        "round 1",
        "previous proposal",
        "previous answer",
        "my previous",
        "prior answer",
        "score of",
        "based on my",
        "feedback suggests",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn emit_text(output: &BenchmarkOutput) {
    println!("Status: {}", output.status);
    for run in &output.runs {
        println!("\nRun: {}", run.run_dir);
        println!("Final round: {}", run.final_round);
        println!("Candidates: {}", run.candidate_count);
        for selector in &run.selectors {
            println!("\n  Selector: {}", selector.name);
            println!(
                "    quality mean/min: {:.2}/{:.2}",
                selector.metrics.panel_mean_quality, selector.metrics.panel_min_quality
            );
            println!(
                "    disagreement: {:.2}, lexical overlap: {:.3}, meta preamble rate: {:.2}",
                selector.metrics.panel_disagreement,
                selector.metrics.lexical_overlap,
                selector.metrics.meta_preamble_rate
            );
            for (rank, candidate) in selector.panel.iter().enumerate() {
                println!(
                    "    {}. {} (mean {:.2}, stddev {:.2}, controversy {:.2})",
                    rank + 1,
                    candidate.model_id,
                    candidate.mean_score,
                    candidate.stddev,
                    candidate.controversy_score
                );
            }
        }
    }
}

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "from", "into", "onto", "over", "under", "would",
    "could", "should", "there", "their", "about", "after", "before", "while", "where", "which",
    "what", "when", "then", "than", "they", "them", "these", "those", "your", "ours", "were",
    "was", "are", "been", "being", "have", "has", "had", "not", "but", "can", "may", "might",
    "will", "just", "only", "also", "very", "more", "most",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_similarity_uses_jaccard_overlap() {
        let similarity = lexical_similarity("alpha beta gamma", "alpha beta delta");
        assert!((similarity - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn meta_preamble_detection_finds_score_history_mentions() {
        assert!(has_meta_preamble(
            "Based on my Round 1 score, here is a better answer."
        ));
        assert!(!has_meta_preamble("Here is a privacy-first product idea."));
    }

    #[test]
    fn controversy_floor_excludes_low_quality_when_possible() {
        let candidates = vec![
            candidate("test/high_disagreement_low_quality", "alpha", 5.5, 2.0),
            candidate("test/solid", "beta", 8.0, 0.0),
            candidate("test/good", "gamma", 7.5, 0.5),
        ];

        let selected = select_by_controversy_with_quality_floor(&candidates, 2, 7.0);
        let selected_ids: Vec<&ModelId> = selected
            .iter()
            .map(|candidate| &candidate.model_id)
            .collect();
        assert!(!selected_ids.contains(&&ModelId::new("test/high_disagreement_low_quality")));
    }

    #[test]
    fn quality_x_lexdiv_prefers_different_answers_after_first_pick() {
        let candidates = vec![
            candidate("test/a", "alpha beta gamma", 9.0, 0.0),
            candidate("test/b", "alpha beta gamma delta", 8.0, 0.0),
            candidate("test/c", "orange banana pear", 7.5, 0.0),
        ];

        let selected = select_by_quality_x_lexdiv(&candidates, 2);
        assert_eq!(selected[0].model_id, ModelId::new("test/a"));
        assert_eq!(selected[1].model_id, ModelId::new("test/c"));
    }

    fn candidate(model_id: &str, answer: &str, mean_score: f64, stddev: f64) -> Candidate {
        Candidate {
            model_id: ModelId::new(model_id),
            answer: answer.to_string(),
            mean_score,
            stddev,
            controversy_score: mean_score * stddev,
            per_evaluator_scores: Vec::new(),
        }
    }
}
