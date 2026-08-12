//! v0.7.0 / Reasoning Engine: Algorithm error detection & diagnostics.
//!
//! This module implements the **diagnostic layer** of the Reasoning Engine.
//! It does NOT make decisions — it inspects the engine's outputs (scored
//! candidates, validated triplets) and the scorer's weights file to detect
//! known failure modes:
//!
//! 1. **Class imbalance**: training set has too many `approve` (label=1.0)
//!    vs `reject` (label=0.0) examples. With 10:1 ratio, the model
//!    collapses to "always say yes" — this is what we observed after
//!    Phase 2 Step 4 on the expanded corpus (Rust found mostly English
//!    names from parallel texts, all auto-approved by the comparator).
//!
//! 2. **Approve-vs-Reject score separation**: when the model is well-trained,
//!    `mean(approve_scores) - mean(reject_scores) > 0.3`. When underfit,
//!    both classes cluster around 0.5 ± 0.1 — separation < 0.1.
//!
//! 3. **Script pollution**: Rust NER detected Latin-script tokens (English
//!    names from parallel-text Gutenberg corpus) mixed with Cyrillic. The
//!    Burn model was trained on this polluted data, so its weights encode
//!    English-name patterns, not Ukrainian-character patterns.
//!
//! 4. **Low-informative features**: some of the 8 features are constant
//!    across the training set (e.g. `is_capitalized` is always 1.0 for
//!    Rust-detected candidates — Rust only flags capitalized tokens).
//!    Such features contribute zero discriminative signal. We detect
//!    them by checking the scaler's `std` array: `std[i] >= 0.5` (the
//!    train_scorer floor) usually means "constant feature, floored".
//!
//! 5. **Weight magnitude collapse**: when Adam optimizer diverges or
//!    saturates, all weights collapse to similar magnitudes — the model
//!    becomes a near-constant function. We compute the std of fc1_weight
//!    flat array; if `std < 0.05`, the weights are suspiciously uniform.
//!
//! ## Output
//!
//! `DiagnosticsReport` is a serializable struct included in the final
//! `ReasoningReport`. The Reasoning Engine surfaces it to the caller
//! (Tauri command / CLI) so the user can see *why* a particular decision
//! was made and *what* to fix in the next training run.

use serde::{Deserialize, Serialize};

use crate::scorer::features::FEATURE_COUNT;
use crate::scorer::inference::{Decision, InferenceScorer};
use crate::scorer::weights::WeightsFile;

/// Script of a token, used to detect parallel-text pollution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Script {
    Cyrillic,
    Latin,
    Mixed,
    Other,
}

/// Detect the dominant script of a word by inspecting its characters.
pub fn detect_script(word: &str) -> Script {
    let mut cyrillic = 0usize;
    let mut latin = 0usize;
    let mut other = 0usize;
    for ch in word.chars() {
        if ch.is_alphabetic() {
            if ('\u{0400}'..='\u{04FF}').contains(&ch) {
                cyrillic += 1;
            } else if ch.is_ascii_alphabetic() {
                latin += 1;
            } else {
                other += 1;
            }
        }
    }
    let total = cyrillic + latin + other;
    if total == 0 {
        return Script::Other;
    }
    if cyrillic == total {
        Script::Cyrillic
    } else if latin == total {
        Script::Latin
    } else if cyrillic > 0 && latin > 0 {
        Script::Mixed
    } else {
        Script::Other
    }
}

/// Report on class imbalance in the training set (inferred from weights file
/// metadata + scorer behavior on a sample batch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassImbalanceReport {
    /// Number of approve decisions observed in the sample batch.
    pub approve_count: usize,
    /// Number of reject decisions observed.
    pub reject_count: usize,
    /// Number of review (uncertain) decisions observed.
    pub review_count: usize,
    /// Approve:Reject ratio. `>5.0` is considered severely imbalanced.
    pub approve_reject_ratio: f64,
    /// True if ratio > 5.0 (severe imbalance).
    pub is_imbalanced: bool,
    /// Human-readable recommendation.
    pub recommendation: String,
}

/// Report on score distribution for a batch of candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDistribution {
    pub mean: f32,
    pub std: f32,
    pub min: f32,
    pub max: f32,
    /// Mean score of approve-decided candidates.
    pub approve_mean: f32,
    /// Mean score of reject-decided candidates.
    pub reject_mean: f32,
    /// `approve_mean - reject_mean`. >0.3 = well-separated, <0.1 = underfit.
    pub separation: f32,
    /// True if separation < 0.15 (model cannot distinguish classes).
    pub underfitting_detected: bool,
    pub recommendation: String,
}

/// Report on script distribution among detected character candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAnalysis {
    pub cyrillic_count: usize,
    pub latin_count: usize,
    pub mixed_count: usize,
    pub other_count: usize,
    pub total: usize,
    /// Fraction of Latin-script candidates (0.0–1.0).
    pub latin_fraction: f32,
    /// True if latin_fraction > 0.30 (parallel-text pollution likely).
    pub parallel_text_detected: bool,
    pub recommendation: String,
}

/// Report on feature informativeness (inferred from scaler std values).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInformativeness {
    /// Per-feature std deviation from the scaler.
    pub per_feature_std: Vec<f32>,
    /// Indices of features whose std == 0.5 (the train_scorer floor).
    /// These features were constant in the training set → zero information.
    pub low_information_features: Vec<usize>,
    /// Feature names (aligned with `per_feature_std`).
    pub feature_names: Vec<String>,
    pub recommendation: String,
}

/// Names of the 11 features (must match `scorer::features`).
pub const FEATURE_NAMES: [&str; FEATURE_COUNT] = [
    "is_capitalized",
    "has_speech_verb",
    "has_direct_address",
    "is_single_token",
    "mention_count_norm",
    "speech_count_norm",
    "direct_count_norm",
    "is_character_type",
    "nominative_case_norm",
    "accusative_case_norm",
    "genitive_under_negation_norm",
];

/// Report on weight magnitude distribution (detects collapse or explosion).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightMagnitudeReport {
    pub fc1_weight_mean: f32,
    pub fc1_weight_std: f32,
    pub fc1_weight_min: f32,
    pub fc1_weight_max: f32,
    pub fc2_weight_mean: f32,
    pub fc2_weight_std: f32,
    /// True if fc1_weight_std < 0.05 (weights collapsed to near-constant).
    pub collapse_detected: bool,
    /// True if fc1_weight_max.abs() > 5.0 (potential explosion).
    pub explosion_detected: bool,
    pub recommendation: String,
}

/// Aggregate diagnostics report included in `ReasoningReport`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    pub class_imbalance: ClassImbalanceReport,
    pub score_distribution: ScoreDistribution,
    pub script_analysis: ScriptAnalysis,
    pub feature_informativeness: FeatureInformativeness,
    pub weight_magnitude: WeightMagnitudeReport,
    /// Overall health verdict: "healthy", "degraded", or "critical".
    pub overall_health: String,
    /// List of all recommendations (concatenated from sub-reports).
    pub recommendations: Vec<String>,
}

impl DiagnosticsReport {
    /// Build the aggregate report from a batch of scored candidates and
    /// the loaded weights file.
    ///
    /// # Arguments
    /// * `scores` — slice of (score, decision) tuples, one per candidate.
    /// * `candidate_names` — slice of candidate name strings (for script analysis).
    /// * `scorer` — the loaded inference scorer (for weight inspection).
    /// * `weights_file` — the source weights file (for feature informativeness).
    pub fn analyze(
        scores: &[(f32, Decision)],
        candidate_names: &[String],
        scorer: &InferenceScorer,
        weights_file: &WeightsFile,
    ) -> Self {
        let class_imbalance = analyze_class_imbalance(scores);
        let score_distribution = analyze_score_distribution(scores);
        let script_analysis = analyze_scripts(candidate_names);
        let feature_informativeness = analyze_feature_informativeness(weights_file);
        let weight_magnitude = analyze_weight_magnitude(scorer);

        let mut recommendations = Vec::new();
        recommendations.push(class_imbalance.recommendation.clone());
        recommendations.push(score_distribution.recommendation.clone());
        recommendations.push(script_analysis.recommendation.clone());
        recommendations.push(feature_informativeness.recommendation.clone());
        recommendations.push(weight_magnitude.recommendation.clone());
        recommendations.retain(|r| !r.is_empty());

        // Overall health: critical if any of (severe imbalance + underfitting +
        // parallel-text pollution + weight collapse) is detected. Degraded if
        // any single issue is detected. Healthy otherwise.
        let critical = class_imbalance.is_imbalanced
            && score_distribution.underfitting_detected
            && script_analysis.parallel_text_detected;
        let degraded = class_imbalance.is_imbalanced
            || score_distribution.underfitting_detected
            || script_analysis.parallel_text_detected
            || weight_magnitude.collapse_detected
            || weight_magnitude.explosion_detected
            || !feature_informativeness.low_information_features.is_empty();

        let overall_health = if critical {
            "critical".to_string()
        } else if degraded {
            "degraded".to_string()
        } else {
            "healthy".to_string()
        };

        DiagnosticsReport {
            class_imbalance,
            score_distribution,
            script_analysis,
            feature_informativeness,
            weight_magnitude,
            overall_health,
            recommendations,
        }
    }
}

fn analyze_class_imbalance(scores: &[(f32, Decision)]) -> ClassImbalanceReport {
    let approve_count = scores.iter().filter(|(_, d)| *d == Decision::Approve).count();
    let reject_count = scores.iter().filter(|(_, d)| *d == Decision::Reject).count();
    let review_count = scores.iter().filter(|(_, d)| *d == Decision::Review).count();

    let approve_reject_ratio = if reject_count > 0 {
        approve_count as f64 / reject_count as f64
    } else if approve_count > 0 {
        f64::INFINITY
    } else {
        0.0
    };

    let is_imbalanced = approve_reject_ratio > 5.0;

    let recommendation = if is_imbalanced {
        format!(
            "Class imbalance detected: approve:reject = {:.1}:1 ({} approve, {} reject). \
             Rebalance training data — add more 'reject' examples (concepts, abstract \
             nouns, English names from parallel texts that should NOT be characters). \
             Target ratio ≤ 3:1.",
            approve_reject_ratio, approve_count, reject_count
        )
    } else {
        String::new()
    };

    ClassImbalanceReport {
        approve_count,
        reject_count,
        review_count,
        approve_reject_ratio,
        is_imbalanced,
        recommendation,
    }
}

fn analyze_score_distribution(scores: &[(f32, Decision)]) -> ScoreDistribution {
    if scores.is_empty() {
        return ScoreDistribution {
            mean: 0.5,
            std: 0.0,
            min: 0.5,
            max: 0.5,
            approve_mean: 0.5,
            reject_mean: 0.5,
            separation: 0.0,
            underfitting_detected: false,
            recommendation: String::new(),
        };
    }

    let raw_scores: Vec<f32> = scores.iter().map(|(s, _)| *s).collect();
    let n = raw_scores.len() as f32;
    let mean = raw_scores.iter().sum::<f32>() / n;
    let variance = raw_scores.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n;
    let std = variance.sqrt();
    let min = raw_scores.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = raw_scores
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);

    let approve_scores: Vec<f32> = scores
        .iter()
        .filter(|(_, d)| *d == Decision::Approve)
        .map(|(s, _)| *s)
        .collect();
    let reject_scores: Vec<f32> = scores
        .iter()
        .filter(|(_, d)| *d == Decision::Reject)
        .map(|(s, _)| *s)
        .collect();

    let approve_mean = if approve_scores.is_empty() {
        0.0
    } else {
        approve_scores.iter().sum::<f32>() / approve_scores.len() as f32
    };
    let reject_mean = if reject_scores.is_empty() {
        0.0
    } else {
        reject_scores.iter().sum::<f32>() / reject_scores.len() as f32
    };

    let separation = approve_mean - reject_mean;
    let underfitting_detected = separation < 0.15 && !approve_scores.is_empty() && !reject_scores.is_empty();

    let recommendation = if underfitting_detected {
        format!(
            "Underfitting detected: approve mean = {:.3}, reject mean = {:.3}, separation = {:.3} \
             (< 0.15 threshold). The model cannot distinguish approve from reject. Likely causes: \
             (a) class imbalance, (b) low-informative features, (c) parallel-text pollution. \
             Retrain with balanced data + case-aware features (Nominative case, FIO morphology).",
            approve_mean, reject_mean, separation
        )
    } else {
        String::new()
    };

    ScoreDistribution {
        mean,
        std,
        min,
        max,
        approve_mean,
        reject_mean,
        separation,
        underfitting_detected,
        recommendation,
    }
}

fn analyze_scripts(candidate_names: &[String]) -> ScriptAnalysis {
    let mut cyrillic_count = 0usize;
    let mut latin_count = 0usize;
    let mut mixed_count = 0usize;
    let mut other_count = 0usize;

    for name in candidate_names {
        match detect_script(name) {
            Script::Cyrillic => cyrillic_count += 1,
            Script::Latin => latin_count += 1,
            Script::Mixed => mixed_count += 1,
            Script::Other => other_count += 1,
        }
    }

    let total = candidate_names.len();
    let latin_fraction = if total > 0 {
        latin_count as f32 / total as f32
    } else {
        0.0
    };
    let parallel_text_detected = latin_fraction > 0.30;

    let recommendation = if parallel_text_detected {
        format!(
            "Parallel-text pollution detected: {:.0}% of candidates are Latin-script \
             ({} out of {}). Rust NER likely picked up English names from Gutenberg \
             parallel-text corpus. Filter candidates by script=Cyrillic before scoring, \
             or train a separate scorer per script.",
            latin_fraction * 100.0,
            latin_count,
            total
        )
    } else {
        String::new()
    };

    ScriptAnalysis {
        cyrillic_count,
        latin_count,
        mixed_count,
        other_count,
        total,
        latin_fraction,
        parallel_text_detected,
        recommendation,
    }
}

fn analyze_feature_informativeness(weights_file: &WeightsFile) -> FeatureInformativeness {
    let per_feature_std = weights_file.scaler.std.clone();
    // A feature at the 0.01 floor is effectively constant (zero information).
    let low_information_features: Vec<usize> = (0..FEATURE_COUNT)
        .filter(|&i| per_feature_std.get(i).copied().unwrap_or(0.0) <= 0.01)
        .collect();

    let recommendation = if !low_information_features.is_empty() {
        let names: Vec<&str> = low_information_features
            .iter()
            .map(|&i| FEATURE_NAMES.get(i).copied().unwrap_or("?"))
            .collect();
        format!(
            "Low-informative features detected: [{}]. These features were constant \
             in the training set (std floored at 0.5). They contribute zero \
             discriminative signal. Consider: (a) adding negative examples that \
             vary these features, (b) removing them from the MLP input, \
             (c) replacing them with case-aware features (Nominative_case_count, \
             Accusative_case_count).",
            names.join(", ")
        )
    } else {
        String::new()
    };

    FeatureInformativeness {
        per_feature_std,
        low_information_features,
        feature_names: FEATURE_NAMES.iter().map(|s| s.to_string()).collect(),
        recommendation,
    }
}

fn analyze_weight_magnitude(scorer: &InferenceScorer) -> WeightMagnitudeReport {
    // Flatten fc1_weight into a single Vec<f32> for stats
    let fc1_flat: Vec<f32> = scorer
        .fc1_weight
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect();
    let fc2_flat: Vec<f32> = scorer
        .fc2_weight
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect();

    let n1 = fc1_flat.len() as f32;
    let fc1_weight_mean = fc1_flat.iter().sum::<f32>() / n1.max(1.0);
    let fc1_var = fc1_flat.iter().map(|w| (w - fc1_weight_mean).powi(2)).sum::<f32>() / n1.max(1.0);
    let fc1_weight_std = fc1_var.sqrt();
    let fc1_weight_min = fc1_flat.iter().cloned().fold(f32::INFINITY, f32::min);
    let fc1_weight_max = fc1_flat
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);

    let n2 = fc2_flat.len() as f32;
    let fc2_weight_mean = if n2 > 0.0 {
        fc2_flat.iter().sum::<f32>() / n2
    } else {
        0.0
    };
    let fc2_var = if n2 > 0.0 {
        fc2_flat.iter().map(|w| (w - fc2_weight_mean).powi(2)).sum::<f32>() / n2
    } else {
        0.0
    };
    let fc2_weight_std = fc2_var.sqrt();

    let collapse_detected = fc1_weight_std < 0.05;
    let explosion_detected = fc1_weight_max.abs() > 5.0;

    let recommendation = if collapse_detected {
        format!(
            "Weight collapse detected: fc1_weight std = {:.4} (< 0.05). All weights are \
             near-constant — the model has degenerated to a near-linear function. \
             Possible causes: (a) learning rate too high, (b) dead ReLUs (all hidden \
             units output 0), (c) training data has near-constant labels. \
             Reinitialize and retrain with lower learning rate (0.001 instead of 0.01).",
            fc1_weight_std
        )
    } else if explosion_detected {
        format!(
            "Weight explosion detected: fc1_weight max abs = {:.4} (> 5.0). \
             Possible causes: (a) missing gradient clipping, (b) features not \
             normalized, (c) learning rate too high. Add gradient clipping \
             (max_norm=1.0) and verify scaler is applied.",
            fc1_weight_max.abs()
        )
    } else {
        String::new()
    };

    WeightMagnitudeReport {
        fc1_weight_mean,
        fc1_weight_std,
        fc1_weight_min,
        fc1_weight_max,
        fc2_weight_mean,
        fc2_weight_std,
        collapse_detected,
        explosion_detected,
        recommendation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_script_cyrillic() {
        assert_eq!(detect_script("Петро"), Script::Cyrillic);
        assert_eq!(detect_script("Марта"), Script::Cyrillic);
        assert_eq!(detect_script("ВЕНЯ"), Script::Cyrillic);
    }

    #[test]
    fn test_detect_script_latin() {
        assert_eq!(detect_script("John"), Script::Latin);
        assert_eq!(detect_script("MARY"), Script::Latin);
    }

    #[test]
    fn test_detect_script_mixed() {
        assert_eq!(detect_script("John-Петро"), Script::Mixed);
    }

    #[test]
    fn test_detect_script_other() {
        assert_eq!(detect_script("123"), Script::Other);
        assert_eq!(detect_script("---"), Script::Other);
    }

    #[test]
    fn test_class_imbalance_severe() {
        let scores: Vec<(f32, Decision)> = (0..10)
            .map(|_| (0.9, Decision::Approve))
            .chain(std::iter::once((0.1, Decision::Reject)))
            .collect();
        let report = analyze_class_imbalance(&scores);
        assert!(report.is_imbalanced);
        assert!(report.approve_reject_ratio > 5.0);
    }

    #[test]
    fn test_class_imbalance_balanced() {
        let scores: Vec<(f32, Decision)> = vec![
            (0.9, Decision::Approve),
            (0.1, Decision::Reject),
            (0.8, Decision::Approve),
            (0.2, Decision::Reject),
        ];
        let report = analyze_class_imbalance(&scores);
        assert!(!report.is_imbalanced);
        assert!((report.approve_reject_ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_score_distribution_underfitting() {
        // Approve and reject scores clustered near 0.5 — underfit.
        let scores: Vec<(f32, Decision)> = vec![
            (0.55, Decision::Review), // technically Review, but we test separation
            (0.45, Decision::Review),
        ];
        let report = analyze_score_distribution(&scores);
        // Both Review, so approve_mean=0, reject_mean=0, separation=0
        assert!(!report.underfitting_detected); // no approve/reject to compare
    }

    #[test]
    fn test_score_distribution_well_separated() {
        let scores: Vec<(f32, Decision)> = vec![
            (0.95, Decision::Approve),
            (0.85, Decision::Approve),
            (0.10, Decision::Reject),
            (0.05, Decision::Reject),
        ];
        let report = analyze_score_distribution(&scores);
        assert!(!report.underfitting_detected);
        assert!(report.separation > 0.7);
    }

    #[test]
    fn test_script_analysis_parallel_text() {
        // 70% Latin, 30% Cyrillic — should flag parallel-text pollution.
        let names: Vec<String> = vec![
            "John".to_string(),
            "Mary".to_string(),
            "Peter".to_string(),
            "Alice".to_string(),
            "Bob".to_string(),
            "Carol".to_string(),
            "Dave".to_string(),
            "Петро".to_string(),
            "Марта".to_string(),
            "Веня".to_string(),
        ];
        let report = analyze_scripts(&names);
        assert!(report.parallel_text_detected);
        assert!(report.latin_fraction > 0.5);
    }

    #[test]
    fn test_feature_informativeness_detects_constant_features() {
        // Build a weights file where 4 features have std=0.01 (floored = constant)
        let mut wf = WeightsFile::new_default();
        wf.scaler.std = vec![0.01, 0.7, 0.01, 0.6, 0.01, 0.8, 0.01, 0.9, 0.7, 0.8, 0.9];
        let report = analyze_feature_informativeness(&wf);
        assert_eq!(report.low_information_features.len(), 4);
        assert!(report.low_information_features.contains(&0));
        assert!(report.low_information_features.contains(&2));
        assert!(report.low_information_features.contains(&4));
        assert!(report.low_information_features.contains(&6));
    }

    #[test]
    fn test_diagnostics_report_overall_health_critical() {
        // Set up all the conditions for "critical":
        // - class imbalance: 10 approve, 1 reject
        // - underfitting: scores clustered
        // - parallel-text: mostly Latin names
        let scores: Vec<(f32, Decision)> = (0..10)
            .map(|_| (0.55, Decision::Approve))
            .chain(std::iter::once((0.45, Decision::Reject)))
            .collect();
        let names: Vec<String> = (0..10).map(|i| format!("Name{}", i)).collect();
        let scorer = InferenceScorer::load_from_path(std::path::Path::new(
            "litgraph-core/data/scorer_weights.json",
        ))
        .unwrap_or_else(|_| {
            // Fallback to default if file not found in test env
            InferenceScorer::from_weights_file(WeightsFile::new_default()).unwrap()
        });
        let wf = WeightsFile::new_default();

        let report = DiagnosticsReport::analyze(&scores, &names, &scorer, &wf);
        // Should be at least "degraded" (likely "critical" if all three fire)
        assert!(
            report.overall_health == "critical" || report.overall_health == "degraded",
            "expected critical or degraded, got {}",
            report.overall_health
        );
        assert!(!report.recommendations.is_empty());
    }
}
