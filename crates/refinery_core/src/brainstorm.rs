//! Brainstorm loop: score-only iteration with controversial panel selection.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::ModelProvider;
use crate::prompts;
use crate::scoring::{self, PanelCandidate};
use crate::types::{Message, ModelId, ScoreHistory};

/// Configuration for a brainstorm run.
pub struct BrainstormConfig {
    pub max_rounds: u32,
    pub panel_size: usize,
    pub max_concurrent: usize,
    pub timeout: Duration,
    /// If set, artifacts are saved per-round as each round completes.
    pub output_dir: Option<std::path::PathBuf>,
}

/// Per-round data from a brainstorm run.
#[derive(Debug, Clone)]
pub struct BrainstormRound {
    pub round: u32,
    /// Model ID → proposal text.
    pub proposals: HashMap<ModelId, String>,
    /// Evaluatee → Vec<(evaluator, score)>.
    pub eval_scores: HashMap<ModelId, Vec<(ModelId, f64)>>,
}

/// Result of a brainstorm run.
#[derive(Debug)]
pub struct BrainstormResult {
    pub panel: Vec<PanelCandidate>,
    pub total_calls: u32,
    pub rounds_completed: u32,
    pub rounds: Vec<BrainstormRound>,
}

/// Error from the brainstorm loop.
#[derive(Debug)]
pub struct BrainstormError {
    pub round: u32,
    pub message: String,
}

impl std::fmt::Display for BrainstormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "round {}: {}", self.round, self.message)
    }
}

impl std::error::Error for BrainstormError {}

/// Run the brainstorm loop: score-only iteration + controversial panel selection.
///
/// Each round: all models propose (with score-only history), then all models
/// evaluate each other's proposals using the brainstorm rubric. After all rounds,
/// select the most controversial answers for the panel.
#[allow(clippy::too_many_lines)]
pub async fn run(
    providers: &[Arc<dyn ModelProvider>],
    prompt: &str,
    config: &BrainstormConfig,
) -> Result<BrainstormResult, BrainstormError> {
    let permits = if config.max_concurrent == 0 {
        providers.len().pow(2).max(1)
    } else {
        config.max_concurrent
    };
    let semaphore = Arc::new(Semaphore::new(permits));

    let timeout = config.timeout;

    let mut score_histories: HashMap<ModelId, ScoreHistory> = HashMap::new();
    let mut latest_answers: HashMap<ModelId, String> = HashMap::new();
    let mut last_round_eval_scores: HashMap<ModelId, Vec<(ModelId, f64)>> = HashMap::new();
    let mut total_calls: u32 = 0;
    let mut round_data: Vec<BrainstormRound> = Vec::new();

    for round in 1..=config.max_rounds {
        // ── Propose ─────────────────────────────────────────────────────

        let mut propose_handles = tokio::task::JoinSet::new();

        for provider in providers {
            let model_id = provider.model_id().clone();
            let sem = semaphore.clone();
            let p = provider.clone();

            let history = score_histories.get(&model_id);
            let empty = Vec::new();
            let user_content =
                prompts::propose_with_score_history_prompt(prompt, history.unwrap_or(&empty));

            let messages = vec![
                Message::system(prompts::brainstorm_system_prompt()),
                Message::user(user_content),
            ];

            propose_handles.spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                let result = tokio::time::timeout(
                    timeout,
                    p.send_message(&messages, Some(prompts::ANSWER_SCHEMA)),
                )
                .await;
                (model_id, result)
            });
        }

        let mut round_proposals: HashMap<ModelId, String> = HashMap::new();
        let mut propose_count: u32 = 0;

        while let Some(result) = propose_handles.join_next().await {
            match result {
                Ok((model_id, Ok(Ok(response)))) => {
                    propose_count += 1;
                    let answer = serde_json::from_str::<serde_json::Value>(&response)
                        .ok()
                        .and_then(|v| v.get("answer").and_then(|a| a.as_str()).map(String::from))
                        .unwrap_or(response);
                    if !answer.trim().is_empty() {
                        round_proposals.insert(model_id, answer);
                    }
                }
                Ok((_, Ok(Err(_)) | Err(_))) | Err(_) => {
                    propose_count += 1;
                }
            }
        }

        total_calls += propose_count;

        if round_proposals.is_empty() {
            return Err(BrainstormError {
                round,
                message: "All models failed to propose.".to_string(),
            });
        }

        // ── Evaluate ────────────────────────────────────────────────────

        // Single model: skip evaluation (no self-eval).
        if round_proposals.len() == 1 {
            for (model_id, answer) in &round_proposals {
                latest_answers.insert(model_id.clone(), answer.clone());
            }
            let rd = BrainstormRound {
                round,
                proposals: round_proposals,
                eval_scores: HashMap::new(),
            };
            if let Some(ref dir) = config.output_dir {
                if let Err(e) = save_round_artifacts(dir, &rd) {
                    eprintln!("Warning: failed to save round {round} artifacts: {e}");
                }
            }
            round_data.push(rd);
            last_round_eval_scores.clear();
            continue;
        }

        let nonce = prompts::generate_nonce();
        let proposed_ids: Vec<ModelId> = round_proposals.keys().cloned().collect();
        let labels = prompts::shuffled_labels(proposed_ids.len());
        let label_map: HashMap<ModelId, String> = proposed_ids
            .iter()
            .zip(labels.iter())
            .map(|(id, label)| (id.clone(), label.clone()))
            .collect();

        let mut eval_handles = tokio::task::JoinSet::new();

        for evaluator_provider in providers {
            let evaluator_id = evaluator_provider.model_id().clone();
            if !round_proposals.contains_key(&evaluator_id) {
                continue;
            }

            for (evaluatee_id, answer_text) in &round_proposals {
                if *evaluatee_id == evaluator_id {
                    continue;
                }

                let eval_label = label_map.get(evaluatee_id).cloned().unwrap_or_default();
                let eval_prompt_text = prompts::brainstorm::brainstorm_evaluate_prompt(
                    prompt,
                    answer_text,
                    &eval_label,
                    &nonce,
                );

                let messages = vec![
                    Message::system(prompts::system_prompt()),
                    Message::user(eval_prompt_text),
                ];

                let sem = semaphore.clone();
                let provider = evaluator_provider.clone();
                let evaluator = evaluator_id.clone();
                let evaluatee = evaluatee_id.clone();

                eval_handles.spawn(async move {
                    let _permit = sem.acquire().await.expect("semaphore closed");
                    let result = tokio::time::timeout(
                        timeout,
                        provider.send_message(
                            &messages,
                            Some(prompts::brainstorm::BRAINSTORM_EVAL_SCHEMA),
                        ),
                    )
                    .await;
                    (evaluator, evaluatee, result)
                });
            }
        }

        let mut round_scores: HashMap<ModelId, Vec<(ModelId, f64)>> = HashMap::new();
        let mut eval_count: u32 = 0;

        while let Some(result) = eval_handles.join_next().await {
            match result {
                Ok((from, to, Ok(Ok(response)))) => {
                    eval_count += 1;
                    let parsed: serde_json::Value =
                        serde_json::from_str(&response).unwrap_or_default();
                    #[allow(clippy::cast_precision_loss)]
                    let score_val = parsed
                        .get("score")
                        .and_then(|v| v.as_u64().map(|u| u as f64).or_else(|| v.as_f64()))
                        .filter(|s| (1.0..=10.0).contains(s));
                    if let Some(score) = score_val {
                        round_scores.entry(to).or_default().push((from, score));
                    }
                }
                Ok((_, _, Ok(Err(_)) | Err(_))) | Err(_) => {
                    eval_count += 1;
                }
            }
        }

        total_calls += eval_count;

        // Update score histories
        for (model_id, answer) in &round_proposals {
            let scores: Vec<f64> = round_scores
                .get(model_id)
                .map(|s| s.iter().map(|(_, score)| *score).collect())
                .unwrap_or_default();
            let mean = scoring::mean(&scores);

            score_histories
                .entry(model_id.clone())
                .or_default()
                .push((answer.clone(), mean));

            latest_answers.insert(model_id.clone(), answer.clone());
        }

        last_round_eval_scores = round_scores.clone();

        // Save artifacts immediately as this round completes
        if let Some(ref dir) = config.output_dir {
            let rd = BrainstormRound {
                round,
                proposals: round_proposals.clone(),
                eval_scores: round_scores,
            };
            if let Err(e) = save_round_artifacts(dir, &rd) {
                eprintln!("Warning: failed to save round {round} artifacts: {e}");
            }
        }
        round_data.push(BrainstormRound {
            round,
            proposals: round_proposals,
            eval_scores: last_round_eval_scores.clone(),
        });
    }

    // ── Panel selection ─────────────────────────────────────────────────

    let mut candidates: Vec<PanelCandidate> = latest_answers
        .iter()
        .map(|(model_id, answer)| {
            let per_evaluator: Vec<(ModelId, f64)> = last_round_eval_scores
                .get(model_id)
                .cloned()
                .unwrap_or_default();
            let scores: Vec<f64> = per_evaluator.iter().map(|(_, s)| *s).collect();
            let m = scoring::mean(&scores);
            let s = scoring::stddev(&scores, m);
            let c = scoring::controversy_score(&scores);

            PanelCandidate {
                model_id: model_id.clone(),
                answer: answer.clone(),
                mean_score: m,
                stddev: s,
                controversy_score: c,
                per_evaluator_scores: per_evaluator,
            }
        })
        .collect();

    let panel = scoring::select_panel(&mut candidates, config.panel_size);

    // Save panel summary
    if let Some(ref dir) = config.output_dir {
        if let Err(e) = save_panel_summary(dir, &panel) {
            eprintln!("Warning: failed to save panel summary: {e}");
        }
    }

    Ok(BrainstormResult {
        panel,
        total_calls,
        rounds_completed: config.max_rounds,
        rounds: round_data,
    })
}

fn save_round_artifacts(
    base_dir: &std::path::Path,
    round: &BrainstormRound,
) -> Result<(), Box<dyn std::error::Error>> {
    let round_dir = base_dir.join(format!("round-{}", round.round));
    std::fs::create_dir_all(&round_dir)?;

    for (model_id, text) in &round.proposals {
        let safe_id = model_id.to_string().replace('/', "_");
        std::fs::write(round_dir.join(format!("propose-{safe_id}.md")), text)?;
    }

    for (evaluatee, scores) in &round.eval_scores {
        let safe_evaluatee = evaluatee.to_string().replace('/', "_");
        for (evaluator, score) in scores {
            let safe_evaluator = evaluator.to_string().replace('/', "_");
            let content = serde_json::json!({
                "evaluator": evaluator.to_string(),
                "evaluatee": evaluatee.to_string(),
                "score": score,
            });
            std::fs::write(
                round_dir.join(format!("evaluate-{safe_evaluator}-{safe_evaluatee}.json")),
                serde_json::to_string_pretty(&content)?,
            )?;
        }
    }

    Ok(())
}

fn save_panel_summary(
    base_dir: &std::path::Path,
    panel: &[scoring::PanelCandidate],
) -> Result<(), Box<dyn std::error::Error>> {
    let panel_json: Vec<serde_json::Value> = panel
        .iter()
        .map(|c| {
            serde_json::json!({
                "model_id": c.model_id.to_string(),
                "mean_score": c.mean_score,
                "stddev": c.stddev,
                "controversy_score": c.controversy_score,
                "per_evaluator_scores": c.per_evaluator_scores.iter()
                    .map(|(id, s)| serde_json::json!({"evaluator": id.to_string(), "score": s}))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    std::fs::write(
        base_dir.join("panel.json"),
        serde_json::to_string_pretty(&panel_json)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{EchoProvider, FailingProvider};

    fn eval_json(score: u8) -> String {
        format!(
            r#"{{"originality": {score}, "insight": {score}, "depth": {score}, "feasibility": {score}, "rationale": "test", "score": {score}}}"#
        )
    }

    fn default_config(max_rounds: u32, panel_size: usize) -> BrainstormConfig {
        BrainstormConfig {
            max_rounds,
            panel_size,
            max_concurrent: 0,
            timeout: Duration::from_secs(120),
            output_dir: None,
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn single_model_returns_answer_directly() {
        // Single model, 3 rounds: 1 propose per round, 0 evals (no self-eval)
        // Queue order per model: [propose_r1, propose_r2, propose_r3]
        let p = EchoProvider::new("test/solo");
        p.queue_response(r#"{"answer": "solo r1"}"#.to_string());
        p.queue_response(r#"{"answer": "solo r2"}"#.to_string());
        p.queue_response(r#"{"answer": "solo r3"}"#.to_string());
        let provider = Arc::new(p) as Arc<dyn ModelProvider>;

        let config = default_config(3, 1);
        let result = run(&[provider], "test prompt", &config).await.unwrap();

        assert_eq!(result.panel.len(), 1);
        // Final-round answer
        assert_eq!(result.panel[0].answer, "solo r3");
        assert_eq!(result.panel[0].model_id, ModelId::new("test/solo"));
        // 3 rounds * 1 propose = 3 calls (no evals for single model)
        assert_eq!(result.total_calls, 3);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn round_loop_accumulates_across_rounds() {
        // 2 models, 2 rounds.
        // Queue order per model: [propose_r1, eval_r1, propose_r2, eval_r2]
        let pa = EchoProvider::new("test/a");
        pa.queue_response(r#"{"answer": "a round 1"}"#.to_string());
        pa.queue_response(eval_json(9)); // a evals b in round 1
        pa.queue_response(r#"{"answer": "a round 2"}"#.to_string());
        pa.queue_response(eval_json(9)); // a evals b in round 2

        let pb = EchoProvider::new("test/b");
        pb.queue_response(r#"{"answer": "b round 1"}"#.to_string());
        pb.queue_response(eval_json(5)); // b evals a in round 1
        pb.queue_response(r#"{"answer": "b round 2"}"#.to_string());
        pb.queue_response(eval_json(5)); // b evals a in round 2

        let providers: Vec<Arc<dyn ModelProvider>> = vec![Arc::new(pa), Arc::new(pb)];
        let config = default_config(2, 2);
        let result = run(&providers, "test prompt", &config).await.unwrap();

        assert_eq!(result.panel.len(), 2);
        // 2 rounds * (2 propose + 2 eval) = 8
        assert_eq!(result.total_calls, 8);

        // Panel should contain final-round answers
        let answers: Vec<&str> = result.panel.iter().map(|c| c.answer.as_str()).collect();
        assert!(answers.contains(&"a round 2"));
        assert!(answers.contains(&"b round 2"));
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn controversial_answer_ranks_higher() {
        // 3 models, 1 round.
        // Queue order per model: [propose, eval_of_other1, eval_of_other2]
        //
        // Model A: propose "controversial take"
        //   A evals B → score 7, A evals C → score 7
        // Model B: propose "safe answer"
        //   B evals A → score 3 (dislikes A), B evals C → score 5
        // Model C: propose "another safe"
        //   C evals A → score 9 (loves A), C evals B → score 7
        //
        // A gives 7, B gives 3, C gives 9 (uniform per evaluator, no self-eval):
        // A receives from B(3)+C(9)=[3,9] → mean=6, stddev=3, controversy=18
        // B receives from A(7)+C(9)=[7,9] → mean=8, stddev=1, controversy=8
        // C receives from A(7)+B(3)=[7,3] → mean=5, stddev=2, controversy=10
        // Panel size 1 → model A wins (highest controversy)

        let pa = EchoProvider::new("test/a");
        pa.queue_response(r#"{"answer": "controversial take"}"#.to_string());
        pa.queue_response(eval_json(7)); // A evals B
        pa.queue_response(eval_json(7)); // A evals C

        // B always scores 3 — dislikes everything
        let pb = EchoProvider::new("test/b");
        pb.queue_response(r#"{"answer": "safe answer"}"#.to_string());
        pb.queue_response(eval_json(3)); // B evals someone
        pb.queue_response(eval_json(3)); // B evals someone else

        // C always scores 9 — loves everything
        let pc = EchoProvider::new("test/c");
        pc.queue_response(r#"{"answer": "another safe"}"#.to_string());
        pc.queue_response(eval_json(9)); // C evals someone
        pc.queue_response(eval_json(9)); // C evals someone else

        let providers: Vec<Arc<dyn ModelProvider>> = vec![Arc::new(pa), Arc::new(pb), Arc::new(pc)];
        let config = default_config(1, 1);
        let result = run(&providers, "test prompt", &config).await.unwrap();

        assert_eq!(result.panel.len(), 1);
        assert_eq!(result.panel[0].answer, "controversial take");
        // A received [3, 9] → stddev = 3.0
        assert!(
            result.panel[0].stddev > 2.0,
            "stddev should be high: got {}",
            result.panel[0].stddev
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn all_models_fail_returns_error() {
        let providers: Vec<Arc<dyn ModelProvider>> = vec![
            Arc::new(FailingProvider::new("test/fail_a")),
            Arc::new(FailingProvider::new("test/fail_b")),
        ];

        let config = default_config(3, 2);
        let result = run(&providers, "test prompt", &config).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.round, 1);
        assert!(err.message.contains("All models failed"));
    }
}
