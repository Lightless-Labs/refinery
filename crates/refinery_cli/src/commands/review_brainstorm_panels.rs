use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

use refinery_core::prompts::extract_json;
use refinery_core::scoring;
use refinery_core::types::ModelId;

use super::common::OutputFormat;

#[derive(Parser, Debug)]
pub struct ReviewBrainstormPanelsArgs {
    /// Brainstorm artifact run directories to turn into a blind panel review pack.
    #[arg(value_name = "RUN_DIR")]
    run_dirs: Vec<PathBuf>,

    /// Selector to use when building each reviewed panel.
    #[arg(long, default_value = "controversy-floor-7")]
    selector: PanelSelector,

    /// Only include these iteration strategies (comma-separated), e.g. score-only,own-reviews,full-visibility.
    #[arg(long, value_delimiter = ',')]
    strategies: Vec<String>,

    /// Prompt text mapping in the form `prompt_id=text`. Repeatable.
    #[arg(long = "prompt-text", value_name = "ID=TEXT")]
    prompt_texts: Vec<String>,

    /// Number of answers per panel [default: 3].
    #[arg(long, default_value = "3", value_parser = clap::value_parser!(u32).range(1..=20))]
    panel_size: u32,

    /// Output format [text|json]. Text emits Markdown.
    #[arg(short, long, default_value = "text")]
    output_format: OutputFormat,

    /// Write a JSON answer key mapping blind labels to strategies and model IDs.
    #[arg(long, value_name = "PATH")]
    key_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum PanelSelector {
    Mean,
    Stddev,
    Controversy,
    #[value(name = "controversy-floor-7", alias = "controversy_floor_7")]
    ControversyFloor7,
    QualityXLexdiv,
}

impl PanelSelector {
    fn as_str(self) -> &'static str {
        match self {
            Self::Mean => "mean",
            Self::Stddev => "stddev",
            Self::Controversy => "controversy",
            Self::ControversyFloor7 => "controversy_floor_7",
            Self::QualityXLexdiv => "quality_x_lexdiv",
        }
    }
}

#[derive(Clone, Debug)]
struct Candidate {
    model_id: ModelId,
    answer: String,
    mean_score: f64,
    stddev: f64,
    controversy_score: f64,
}

#[derive(Debug)]
struct LoadedRun {
    run_dir: PathBuf,
    prompt_id: String,
    iteration_strategy: String,
    panel: Vec<Candidate>,
}

#[derive(Debug, Serialize)]
struct ReviewPack {
    status: String,
    selector: String,
    prompts: Vec<PromptReview>,
}

#[derive(Debug, Serialize)]
struct PromptReview {
    prompt_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_text: Option<String>,
    panels: Vec<BlindPanel>,
}

#[derive(Debug, Serialize)]
struct BlindPanel {
    label: String,
    answers: Vec<BlindAnswer>,
}

#[derive(Debug, Serialize)]
struct BlindAnswer {
    label: String,
    answer: String,
}

#[derive(Debug, Serialize)]
struct ReviewAnswerKey {
    selector: String,
    prompts: Vec<PromptAnswerKey>,
}

#[derive(Debug, Serialize)]
struct PromptAnswerKey {
    prompt_id: String,
    panels: Vec<PanelAnswerKey>,
}

#[derive(Debug, Serialize)]
struct PanelAnswerKey {
    label: String,
    iteration_strategy: String,
    run_dir: String,
    answers: Vec<AnswerKeyEntry>,
}

#[derive(Debug, Serialize)]
struct AnswerKeyEntry {
    label: String,
    model_id: String,
    mean_score: f64,
    stddev: f64,
    controversy_score: f64,
}

#[derive(Debug, Deserialize)]
struct EvalArtifact {
    evaluator: String,
    evaluatee: String,
    score: f64,
}

#[derive(Debug, Deserialize)]
struct RunMetadata {
    iteration_strategy: Option<String>,
}

pub fn run(args: &ReviewBrainstormPanelsArgs) -> ExitCode {
    if args.run_dirs.is_empty() {
        eprintln!("Error: at least one brainstorm run directory is required");
        return ExitCode::from(4);
    }

    let prompt_texts = match parse_prompt_texts(&args.prompt_texts) {
        Ok(texts) => texts,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(4);
        }
    };

    let strategy_filter: BTreeSet<String> = args.strategies.iter().cloned().collect();
    let mut runs = Vec::new();
    for run_dir in &args.run_dirs {
        match load_run(run_dir, args.selector, args.panel_size as usize) {
            Ok(run)
                if strategy_filter.is_empty()
                    || strategy_filter.contains(&run.iteration_strategy) =>
            {
                runs.push(run);
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error loading {}: {e}", run_dir.display());
                return ExitCode::from(1);
            }
        }
    }

    if runs.is_empty() {
        eprintln!("Error: no runs matched the requested strategy filter");
        return ExitCode::from(4);
    }

    let (pack, key) = build_review_pack(runs, args.selector.as_str(), &prompt_texts);

    if let Some(path) = &args.key_path {
        if let Err(e) = write_answer_key(path, &key) {
            eprintln!("Error writing answer key {}: {e}", path.display());
            return ExitCode::from(1);
        }
    }

    match args.output_format {
        OutputFormat::Json => match serde_json::to_string_pretty(&pack) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Failed to serialize review pack: {e}");
                ExitCode::from(1)
            }
        },
        OutputFormat::Text => {
            emit_markdown(&pack);
            ExitCode::SUCCESS
        }
    }
}

fn parse_prompt_texts(values: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut parsed = BTreeMap::new();
    for value in values {
        let (id, text) = value
            .split_once('=')
            .ok_or_else(|| format!("invalid --prompt-text '{value}', expected ID=TEXT"))?;
        if id.trim().is_empty() || text.trim().is_empty() {
            return Err(format!(
                "invalid --prompt-text '{value}', both ID and TEXT must be non-empty"
            ));
        }
        parsed.insert(id.trim().to_string(), text.trim().to_string());
    }
    Ok(parsed)
}

fn write_answer_key(path: &Path, key: &ReviewAnswerKey) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(key)
        .map_err(|e| format!("failed to serialize answer key: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn load_run(
    run_dir: &Path,
    selector: PanelSelector,
    panel_size: usize,
) -> Result<LoadedRun, String> {
    let final_round = find_final_round(run_dir)?;
    let round_dir = run_dir.join(format!("round-{final_round}"));
    let candidates = load_candidates(&round_dir)?;
    let panel = select_panel(&candidates, selector, panel_size)
        .into_iter()
        .cloned()
        .collect();
    let metadata = load_run_metadata(run_dir)?;
    let iteration_strategy = metadata
        .and_then(|metadata| metadata.iteration_strategy)
        .ok_or_else(|| "run metadata missing iteration_strategy".to_string())?;
    let prompt_id = prompt_id_from_run_dir(run_dir);

    Ok(LoadedRun {
        run_dir: run_dir.to_path_buf(),
        prompt_id,
        iteration_strategy,
        panel,
    })
}

fn load_run_metadata(run_dir: &Path) -> Result<Option<RunMetadata>, String> {
    let path = run_dir.join("metadata.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let metadata = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
    Ok(Some(metadata))
}

fn prompt_id_from_run_dir(run_dir: &Path) -> String {
    run_dir.parent().and_then(Path::file_name).map_or_else(
        || run_dir.display().to_string(),
        |name| name.to_string_lossy().to_string(),
    )
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
    let mut scores: BTreeMap<ModelId, Vec<f64>> = BTreeMap::new();

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
        if eval.evaluator != eval.evaluatee {
            scores
                .entry(ModelId::new(&eval.evaluatee))
                .or_default()
                .push(eval.score);
        }
    }

    if scores.is_empty() {
        return Err("no evaluation artifacts found".to_string());
    }

    scores
        .into_iter()
        .map(|(model_id, score_values)| {
            let safe_id = model_id.to_string().replace('/', "_");
            let proposal_path = round_dir.join(format!("propose-{safe_id}.md"));
            let proposal = std::fs::read_to_string(&proposal_path).map_err(|e| {
                format!(
                    "failed to read proposal for {model_id} at {}: {e}",
                    proposal_path.display()
                )
            })?;
            let answer = proposal_answer_text(&proposal);
            let mean_score = scoring::mean(&score_values);
            let stddev = scoring::stddev(&score_values, mean_score);
            let controversy_score = scoring::controversy_score(&score_values);
            Ok(Candidate {
                model_id,
                answer,
                mean_score,
                stddev,
                controversy_score,
            })
        })
        .collect()
}

fn proposal_answer_text(proposal: &str) -> String {
    let candidates = proposal_json_candidates(proposal);
    candidates
        .iter()
        .find_map(|json| {
            serde_json::from_str::<serde_json::Value>(json)
                .ok()
                .and_then(|value| {
                    value
                        .get("answer")
                        .and_then(|answer| answer.as_str())
                        .map(str::to_string)
                })
        })
        .or_else(|| {
            candidates
                .iter()
                .find_map(|candidate| malformed_answer_wrapper(candidate))
        })
        .unwrap_or_else(|| proposal.to_string())
}

fn malformed_answer_wrapper(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    let marker_start = trimmed.rfind("\"answer\"")?;
    let after_marker = trimmed[marker_start + "\"answer\"".len()..].trim_start();
    let after_colon = after_marker.strip_prefix(':')?.trim_start();
    let inner = after_colon
        .strip_prefix('"')?
        .trim_end()
        .strip_suffix('}')?
        .trim_end()
        .strip_suffix('"')?;
    Some(
        inner
            .replace("\\n", "\n")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\"),
    )
}

fn proposal_json_candidates(proposal: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let trimmed = proposal.trim();

    // Pi responses can wrap a JSON object in a markdown JSON fence while the
    // JSON string itself contains markdown fences. The shared extract_json()
    // helper intentionally stops at the first closing fence, so first try a
    // last-fence interpretation for proposal artifacts.
    if let Some(after_opening_fence) = trimmed.strip_prefix("```json") {
        let after_opening_fence = after_opening_fence.trim_start();
        if let Some(end) = after_opening_fence.rfind("```") {
            candidates.push(after_opening_fence[..end].trim());
        }
    }

    if let Some(json) = extract_json(proposal) {
        candidates.push(json);
    }
    candidates.push(trimmed);
    candidates
}

fn select_panel(
    candidates: &[Candidate],
    selector: PanelSelector,
    panel_size: usize,
) -> Vec<&Candidate> {
    match selector {
        PanelSelector::Mean => select_by_mean(candidates, panel_size),
        PanelSelector::Stddev => select_by_stddev(candidates, panel_size),
        PanelSelector::Controversy => select_by_controversy(candidates, panel_size),
        PanelSelector::ControversyFloor7 => {
            select_by_controversy_with_quality_floor(candidates, panel_size, 7.0)
        }
        PanelSelector::QualityXLexdiv => select_by_quality_x_lexdiv(candidates, panel_size),
    }
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

fn build_review_pack(
    runs: Vec<LoadedRun>,
    selector: &str,
    prompt_texts: &BTreeMap<String, String>,
) -> (ReviewPack, ReviewAnswerKey) {
    let mut grouped: BTreeMap<String, Vec<LoadedRun>> = BTreeMap::new();
    for run in runs {
        grouped.entry(run.prompt_id.clone()).or_default().push(run);
    }

    let mut prompt_reviews = Vec::new();
    let mut prompt_keys = Vec::new();

    for (prompt_id, mut prompt_runs) in grouped {
        prompt_runs.sort_by(|a, b| {
            blind_sort_key(&prompt_id, &a.iteration_strategy)
                .cmp(&blind_sort_key(&prompt_id, &b.iteration_strategy))
                .then_with(|| a.iteration_strategy.cmp(&b.iteration_strategy))
        });

        let mut panels = Vec::new();
        let mut key_panels = Vec::new();
        for (panel_index, run) in prompt_runs.into_iter().enumerate() {
            let panel_label = panel_label(panel_index);
            let answers: Vec<BlindAnswer> = run
                .panel
                .iter()
                .enumerate()
                .map(|(answer_index, candidate)| BlindAnswer {
                    label: answer_label(answer_index),
                    answer: candidate.answer.clone(),
                })
                .collect();
            let key_answers = run
                .panel
                .iter()
                .enumerate()
                .map(|(answer_index, candidate)| AnswerKeyEntry {
                    label: answer_label(answer_index),
                    model_id: candidate.model_id.to_string(),
                    mean_score: candidate.mean_score,
                    stddev: candidate.stddev,
                    controversy_score: candidate.controversy_score,
                })
                .collect();
            panels.push(BlindPanel {
                label: panel_label.clone(),
                answers,
            });
            key_panels.push(PanelAnswerKey {
                label: panel_label,
                iteration_strategy: run.iteration_strategy,
                run_dir: run.run_dir.display().to_string(),
                answers: key_answers,
            });
        }

        prompt_reviews.push(PromptReview {
            prompt_text: prompt_texts.get(&prompt_id).cloned(),
            prompt_id: prompt_id.clone(),
            panels,
        });
        prompt_keys.push(PromptAnswerKey {
            prompt_id,
            panels: key_panels,
        });
    }

    (
        ReviewPack {
            status: "review_pack".to_string(),
            selector: selector.to_string(),
            prompts: prompt_reviews,
        },
        ReviewAnswerKey {
            selector: selector.to_string(),
            prompts: prompt_keys,
        },
    )
}

fn blind_sort_key(prompt_id: &str, strategy: &str) -> u64 {
    // FNV-1a for deterministic label shuffling without adding a dependency.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in prompt_id.bytes().chain([b'|']).chain(strategy.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn panel_label(index: usize) -> String {
    let letter = char::from(b'A' + u8::try_from(index % 26).expect("panel label index fits"));
    if index < 26 {
        letter.to_string()
    } else {
        format!("{letter}{}", index / 26)
    }
}

fn answer_label(index: usize) -> String {
    format!("Answer {}", index + 1)
}

fn emit_markdown(pack: &ReviewPack) {
    println!("# Brainstorm Panel Review Pack");
    println!();
    println!("Selector: `{}`", pack.selector);
    println!();
    println!(
        "Review each panel as a complete user-facing brainstorm result. The panel labels are blind: they do not reveal the iteration strategy or model IDs."
    );
    println!();
    println!(
        "Use a 1-5 scale for: useful diversity, non-overlap, novelty, actionability, coverage, and overall panel value. Also note best-answer regret: whether the panel appears to omit an obviously stronger direction."
    );

    for prompt in &pack.prompts {
        println!();
        println!("## Prompt: {}", prompt.prompt_id);
        if let Some(text) = &prompt.prompt_text {
            println!();
            println!("{text}");
        }

        for panel in &prompt.panels {
            println!();
            println!("### Panel {}", panel.label);
            for answer in &panel.answers {
                println!();
                println!("#### {}", answer.label);
                println!();
                println!("{}", answer.answer.trim());
            }
            println!();
            println!("#### Review notes for Panel {}", panel.label);
            println!();
            println!("- Useful diversity (1-5): ");
            println!("- Non-overlap (1-5): ");
            println!("- Novelty (1-5): ");
            println!("- Actionability (1-5): ");
            println!("- Coverage (1-5): ");
            println!("- Overall panel value (1-5): ");
            println!("- Best-answer regret / omissions: ");
            println!("- Notes: ");
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
    fn parse_prompt_texts_requires_id_text_pairs() {
        let parsed = parse_prompt_texts(&["product=Generate ideas".to_string()]).unwrap();
        assert_eq!(parsed.get("product"), Some(&"Generate ideas".to_string()));
        assert!(parse_prompt_texts(&["missing separator".to_string()]).is_err());
    }

    #[test]
    fn prompt_id_comes_from_run_parent_directory() {
        let path = Path::new("target/bench/score-only/product/20260530_run");
        assert_eq!(prompt_id_from_run_dir(path), "product");
    }

    #[test]
    fn proposal_answer_text_extracts_fenced_json_answer() {
        let proposal = "```json\n{\"answer\":\"Actual answer\"}\n```";
        assert_eq!(proposal_answer_text(proposal), "Actual answer");
    }

    #[test]
    fn proposal_answer_text_handles_nested_markdown_fences_inside_json_string() {
        let proposal =
            "```json\n{\"answer\":\"Use this snippet:\\n\\n```rust\\nfn main() {}\\n```\"}\n```";
        assert_eq!(
            proposal_answer_text(proposal),
            "Use this snippet:\n\n```rust\nfn main() {}\n```"
        );
    }

    #[test]
    fn proposal_answer_text_handles_malformed_multiline_answer_wrapper() {
        let proposal = "```json\n{\"answer\":\"First line\nSecond line with \\\"quote\\\"\"}\n```";
        assert_eq!(
            proposal_answer_text(proposal),
            "First line\nSecond line with \"quote\""
        );
    }

    #[test]
    fn review_pack_hides_strategy_but_key_reveals_it() {
        let runs = vec![
            loaded_run("product", "score-only", "score answer"),
            loaded_run("product", "own-reviews", "review answer"),
        ];
        let (pack, key) = build_review_pack(runs, "controversy_floor_7", &BTreeMap::new());

        assert_eq!(pack.prompts.len(), 1);
        assert_eq!(pack.prompts[0].panels.len(), 2);
        let json = serde_json::to_string(&pack).unwrap();
        assert!(!json.contains("score-only"));
        assert!(!json.contains("own-reviews"));

        let key_json = serde_json::to_string(&key).unwrap();
        assert!(key_json.contains("score-only"));
        assert!(key_json.contains("own-reviews"));
    }

    fn loaded_run(prompt_id: &str, strategy: &str, answer: &str) -> LoadedRun {
        LoadedRun {
            run_dir: PathBuf::from(format!("target/{strategy}/{prompt_id}/run")),
            prompt_id: prompt_id.to_string(),
            iteration_strategy: strategy.to_string(),
            panel: vec![Candidate {
                model_id: ModelId::new("test/model"),
                answer: answer.to_string(),
                mean_score: 8.0,
                stddev: 0.5,
                controversy_score: 4.0,
            }],
        }
    }
}
