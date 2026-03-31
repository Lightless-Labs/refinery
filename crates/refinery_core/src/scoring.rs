use crate::types::ModelId;

/// Compute the arithmetic mean of a slice of scores.
/// Returns 0.0 for an empty slice.
#[must_use]
pub fn mean(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)] // evaluator count will never approach 2^52
    let count = scores.len() as f64;
    scores.iter().sum::<f64>() / count
}

/// Compute the population standard deviation given scores and their mean.
/// Returns 0.0 for slices with fewer than 2 elements.
#[must_use]
pub fn stddev(scores: &[f64], mean: f64) -> f64 {
    if scores.len() < 2 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)] // evaluator count will never approach 2^52
    let count = scores.len() as f64;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / count;
    variance.sqrt()
}

/// Compute a controversy score from per-evaluator scores.
///
/// Controversy = mean * stddev (population). High-quality answers that evaluators
/// disagree about rank higher. Returns 0.0 if scores is empty or has one element.
#[must_use]
pub fn controversy_score(scores: &[f64]) -> f64 {
    let m = mean(scores);
    let s = stddev(scores, m);
    m * s
}

/// A candidate answer for panel selection, with score metadata.
#[derive(Debug, Clone)]
pub struct PanelCandidate {
    pub model_id: ModelId,
    pub answer: String,
    pub mean_score: f64,
    pub stddev: f64,
    pub controversy_score: f64,
    pub per_evaluator_scores: Vec<(ModelId, f64)>,
}

/// Select the top `panel_size` candidates by controversy score (descending),
/// with mean score as a tiebreaker (also descending).
///
/// Returns all candidates if `panel_size >= candidates.len()`.
#[must_use]
pub fn select_panel(candidates: &mut [PanelCandidate], panel_size: usize) -> Vec<PanelCandidate> {
    // Sort descending by (controversy_score, mean_score) — NaN-safe via total_cmp.
    candidates.sort_by(|a, b| {
        b.controversy_score
            .total_cmp(&a.controversy_score)
            .then_with(|| b.mean_score.total_cmp(&a.mean_score))
    });
    candidates.iter().take(panel_size).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controversy_score_empty_scores() {
        assert!((controversy_score(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn controversy_score_single_evaluator() {
        // Single score → stddev = 0 → controversy = 0
        assert!((controversy_score(&[7.0]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn high_disagreement_ranks_above_low_disagreement() {
        // Same mean (6.75), but very different stddevs
        let high_variance = [3.0, 5.0, 9.0, 10.0]; // mean=6.75, high stddev
        let low_variance = [6.0, 7.0, 7.0, 7.0]; // mean=6.75, low stddev

        let high_cs = controversy_score(&high_variance);
        let low_cs = controversy_score(&low_variance);

        assert!(
            high_cs > low_cs,
            "high_variance controversy ({high_cs}) should exceed low_variance ({low_cs})"
        );
    }

    #[test]
    fn select_panel_returns_top_n_by_controversy() {
        let mut candidates = vec![
            PanelCandidate {
                model_id: ModelId::new("test/low"),
                answer: "low controversy".into(),
                mean_score: 6.75,
                stddev: stddev(&[6.0, 7.0, 7.0, 7.0], 6.75),
                controversy_score: controversy_score(&[6.0, 7.0, 7.0, 7.0]),
                per_evaluator_scores: vec![],
            },
            PanelCandidate {
                model_id: ModelId::new("test/high"),
                answer: "high controversy".into(),
                mean_score: 6.75,
                stddev: stddev(&[3.0, 5.0, 9.0, 10.0], 6.75),
                controversy_score: controversy_score(&[3.0, 5.0, 9.0, 10.0]),
                per_evaluator_scores: vec![],
            },
            PanelCandidate {
                model_id: ModelId::new("test/mid"),
                answer: "mid controversy".into(),
                mean_score: 5.0,
                stddev: stddev(&[3.0, 7.0], 5.0),
                controversy_score: controversy_score(&[3.0, 7.0]),
                per_evaluator_scores: vec![],
            },
        ];

        let panel = select_panel(&mut candidates, 2);
        assert_eq!(panel.len(), 2);
        assert_eq!(panel[0].model_id, ModelId::new("test/high"));
    }

    #[test]
    fn select_panel_identical_scores_tiebreak_by_mean() {
        // All have identical scores → stddev=0 → controversy=0 for all.
        // Tiebreaker is mean descending.
        let mut candidates = vec![
            PanelCandidate {
                model_id: ModelId::new("test/low_mean"),
                answer: "low mean".into(),
                mean_score: 5.0,
                stddev: 0.0,
                controversy_score: 0.0,
                per_evaluator_scores: vec![],
            },
            PanelCandidate {
                model_id: ModelId::new("test/high_mean"),
                answer: "high mean".into(),
                mean_score: 9.0,
                stddev: 0.0,
                controversy_score: 0.0,
                per_evaluator_scores: vec![],
            },
            PanelCandidate {
                model_id: ModelId::new("test/mid_mean"),
                answer: "mid mean".into(),
                mean_score: 7.0,
                stddev: 0.0,
                controversy_score: 0.0,
                per_evaluator_scores: vec![],
            },
        ];

        let panel = select_panel(&mut candidates, 2);
        assert_eq!(panel.len(), 2);
        assert_eq!(panel[0].model_id, ModelId::new("test/high_mean"));
        assert_eq!(panel[1].model_id, ModelId::new("test/mid_mean"));
    }

    #[test]
    fn select_panel_larger_than_candidates_returns_all() {
        let mut candidates = vec![PanelCandidate {
            model_id: ModelId::new("test/only"),
            answer: "solo".into(),
            mean_score: 8.0,
            stddev: 0.0,
            controversy_score: 0.0,
            per_evaluator_scores: vec![],
        }];

        let panel = select_panel(&mut candidates, 10);
        assert_eq!(panel.len(), 1);
        assert_eq!(panel[0].model_id, ModelId::new("test/only"));
    }

    #[test]
    fn mean_and_stddev_correctness() {
        let scores = [3.0, 5.0, 9.0, 10.0];
        let m = mean(&scores);
        // (3+5+9+10)/4 = 6.75
        assert!((m - 6.75).abs() < f64::EPSILON);

        let s = stddev(&scores, m);
        // variance = ((3-6.75)^2 + (5-6.75)^2 + (9-6.75)^2 + (10-6.75)^2) / 4
        //          = (14.0625 + 3.0625 + 5.0625 + 10.5625) / 4
        //          = 32.75 / 4 = 8.1875
        // stddev = sqrt(8.1875) ≈ 2.8614
        let expected_stddev = (8.1875_f64).sqrt();
        assert!(
            (s - expected_stddev).abs() < 1e-10,
            "stddev {s} != expected {expected_stddev}"
        );

        // controversy = mean * stddev = 6.75 * 2.8614... ≈ 19.315
        let cs = controversy_score(&scores);
        assert!((cs - m * expected_stddev).abs() < 1e-10);
    }

    #[test]
    fn happy_path_high_stddev_beats_low_stddev_same_mean() {
        // From the plan: mean=7.0, scores [3,5,9,10] vs mean=7.0, scores [6,7,7,8]
        // Adjusting: [3,5,9,10] has mean 6.75, [6,7,7,8] has mean 7.0
        // Using the exact plan scenario with matching means:
        let high_var = [4.0, 6.0, 8.0, 10.0]; // mean=7.0, stddev=sqrt(5)≈2.236
        let low_var = [6.0, 7.0, 7.0, 8.0]; // mean=7.0, stddev=sqrt(0.5)≈0.707

        assert!((mean(&high_var) - 7.0).abs() < f64::EPSILON);
        assert!((mean(&low_var) - 7.0).abs() < f64::EPSILON);

        let high_cs = controversy_score(&high_var);
        let low_cs = controversy_score(&low_var);
        assert!(
            high_cs > low_cs,
            "high_var ({high_cs}) should beat low_var ({low_cs})"
        );
    }

    #[test]
    fn select_panel_top_3() {
        let mut candidates: Vec<PanelCandidate> = (0..5)
            .map(|i| {
                let scores: Vec<f64> = vec![i as f64, (10 - i) as f64];
                PanelCandidate {
                    model_id: ModelId::new(&format!("test/model_{i}")),
                    answer: format!("answer {i}"),
                    mean_score: mean(&scores),
                    stddev: stddev(&scores, mean(&scores)),
                    controversy_score: controversy_score(&scores),
                    per_evaluator_scores: vec![],
                }
            })
            .collect();

        let panel = select_panel(&mut candidates, 3);
        assert_eq!(panel.len(), 3);

        // Verify descending controversy order
        for window in panel.windows(2) {
            assert!(
                window[0].controversy_score >= window[1].controversy_score,
                "panel not sorted descending: {} >= {}",
                window[0].controversy_score,
                window[1].controversy_score
            );
        }
    }
}
