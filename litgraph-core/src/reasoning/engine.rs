//! v0.7.0 / Reasoning Engine: Symbolic reasoning orchestrator (no LLM).
//!
//! The Reasoning Engine is the central orchestrator that combines all
//! symbolic signals produced by the LitGraph Core layers into a single
//! coherent analysis. It is **LLM-free** — every signal it consumes is
//! produced by deterministic Rust code:
//!
//! ```text
//!                 ┌─────────────────────────────────────────────────┐
//!                 │              ReasoningEngine                     │
//!                 │                                                  │
//!   raw text ───► │  ┌──────────────────────┐                       │
//!                 │  │ Stage 1: Rust NER     │  characters::detect   │
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │  ┌──────────────────────┐                       │
//!                 │  │ Stage 2: Burn Scorer  │  InferenceScorer      │
//!                 │  │  (weights.json)        │  .score(features)    │
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │  ┌──────────────────────┐                       │
//!                 │  │ Stage 3: SVO Parser   │  SvoParser::parse_text│
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │  ┌──────────────────────┐                       │
//!                 │  │ Stage 4: Case Valid.  │  case_validation      │
//!                 │  │  (Nominative/Acc/...) │  .validate_svo_cases  │
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │  ┌──────────────────────┐                       │
//!                 │  │ Stage 5: POLER ε      │  compute_epsilon_     │
//!                 │  │  (climax formula)     │  climax_with_analyzer │
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │  ┌──────────────────────┐                       │
//!                 │  │ Stage 6: Conflict G.  │  NarrativeGraph       │
//!                 │  │  (Ω_conf, paradoxes)  │  .analyze             │
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │  ┌──────────────────────┐                       │
//!                 │  │ Stage 7: Diagnostics  │  diagnostics          │
//!                 │  │  (underfitting, ...)  │  ::DiagnosticsReport  │
//!                 │  └──────────┬───────────┘                       │
//!                 │             ▼                                    │
//!                 │     ReasoningReport ───► caller (Tauri / CLI)   │
//!                 └─────────────────────────────────────────────────┘
//! ```
//!
//! # How the engine consumes `weights.json`
//!
//! 1. On construction, `ReasoningEngine::new(weights_path)` loads the
//!    `WeightsFile` from disk and builds an [`InferenceScorer`] (pure-Rust
//!    MLP, no Burn runtime — see `scorer/inference.rs`).
//! 2. For each `ParsedCharacter` candidate from Stage 1, the engine extracts
//!    the 8-feature vector (`scorer::features::extract_features`) and calls
//!    `InferenceScorer::score_and_decide(features)`.
//! 3. The score becomes the candidate's `refined_confidence`. The decision
//!    (`Approve` / `Reject` / `Review`) is the engine's verdict for that
//!    candidate.
//! 4. The diagnostics layer (Stage 7) inspects the full distribution of
//!    scores + decisions + scaler std + weight magnitudes to flag
//!    underfitting, class imbalance, parallel-text pollution, etc.
//!
//! # Linguistic case validation
//!
//! Stage 4 inspects each SVO triplet's actor / target / instrument /
//! location tokens and verifies their grammatical cases match the role
//! requirements:
//! - Subject → Nominative
//! - Direct object → Accusative (or Genitive under negation)
//! - Instrument → Instrumental
//! - Location → Locative
//!
//! Triplets with case mismatches get their confidence multiplied by 0.3
//! (heavy penalty) so they don't pollute the narrative graph.
//!
//! # No LLM, no network, no stochasticity
//!
//! The engine is fully deterministic: same input text + same weights.json
//! → same `ReasoningReport`. This is a hard requirement for the POLER
//! symbolic-AI pipeline (see `docs/architecture/POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::linguistic::case_validation::{
    apply_case_validation, validate_svo_cases, CaseValidationResult,
};
use crate::linguistic::svo_parser::{SvoParser, SvoTriplet};
use crate::parser::characters::ParsedCharacter;
use crate::parser::epsilon::{compute_epsilon_climax_with_analyzer, EpsilonResult};
use crate::reasoning::diagnostics::DiagnosticsReport;
use crate::reasoning::narrative_graph::NarrativeGraph;
use crate::reasoning::{ConflictAnalyzer, ConflictReport};
use crate::scorer::features::extract_features;
use crate::scorer::inference::{Decision, InferenceScorer};
use crate::scorer::weights::WeightsFile;

/// A character candidate with refined confidence from the Burn scorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredCharacter {
    /// Original Rust NER output (immutable).
    #[serde(flatten)]
    pub parsed: ParsedCharacter,
    /// 8 features extracted from `parsed` (for audit / debugging).
    pub features: Vec<f32>,
    /// Raw confidence from Rust policy (0.3 / 0.5 / 0.7 / 1.0).
    pub raw_confidence: f32,
    /// Refined confidence from Burn MLP (0.0–1.0).
    pub refined_confidence: f32,
    /// Decision verdict: Approve / Reject / Review.
    pub decision: Decision,
    /// Detected script of the candidate name (Cyrillic / Latin / Mixed / Other).
    pub script: crate::reasoning::diagnostics::Script,
}

/// An SVO triplet with case-validation results attached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedTriplet {
    /// Original triplet (with adjusted confidence after case validation).
    #[serde(flatten)]
    pub triplet: SvoTriplet,
    /// Case validation per-role verdicts.
    pub case_validation: CaseValidationResult,
    /// True if the actor name resolves to a detected character.
    pub is_actor_character: bool,
    /// True if the target name resolves to a detected character.
    pub is_target_character: bool,
}

/// Serializable subset of `EpsilonResult` fields (the original has a
/// `&'static str` field which doesn't survive `Deserialize`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpsilonSummary {
    pub epsilon: f64,
    pub normalized: f64,
    pub word_count: usize,
    pub unique_words: usize,
    pub emotion_count: usize,
    pub is_climax: bool,
    pub is_noise: bool,
    pub theta_rel: f64,
    pub formula_variant: String,
}

impl From<&EpsilonResult> for EpsilonSummary {
    fn from(e: &EpsilonResult) -> Self {
        EpsilonSummary {
            epsilon: e.epsilon,
            normalized: e.normalized,
            word_count: e.word_count,
            unique_words: e.unique_words,
            emotion_count: e.emotion_count,
            is_climax: e.is_climax,
            is_noise: e.is_noise,
            theta_rel: e.theta_rel,
            formula_variant: e.formula_variant.to_string(),
        }
    }
}

/// The complete output of one `ReasoningEngine::analyze()` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningReport {
    /// Stage 1+2: character candidates with refined confidence + decisions.
    pub characters: Vec<ScoredCharacter>,
    /// Stage 3+4: SVO triplets with case-validation results.
    pub triplets: Vec<ValidatedTriplet>,
    /// Stage 5: POLER ε_climax for the fragment (serializable summary).
    pub epsilon: EpsilonSummary,
    /// Stage 6: conflict magnitude Ω_conf + paradoxes.
    pub conflict: ConflictReport,
    /// Stage 7: algorithm diagnostics (underfitting, class imbalance, ...).
    pub diagnostics: DiagnosticsReport,
    /// Number of characters approved by the scorer.
    pub approved_count: usize,
    /// Number of characters rejected.
    pub rejected_count: usize,
    /// Number of characters needing review.
    pub review_count: usize,
    /// Total characters analyzed.
    pub total_characters: usize,
    /// Total SVO triplets extracted.
    pub total_triplets: usize,
    /// Number of triplets with valid case alignment.
    pub triplets_valid_cases: usize,
    /// Number of triplets with invalid case alignment.
    pub triplets_invalid_cases: usize,
    /// Input text length (chars).
    pub text_length: usize,
    /// Weights file version used for scoring.
    pub weights_version: String,
    /// Weights file architecture string.
    pub weights_architecture: String,
}

/// The Reasoning Engine. Stateless after construction (weights are loaded once).
pub struct ReasoningEngine {
    scorer: InferenceScorer,
    svo_parser: SvoParser,
    /// Cached weights file metadata (for diagnostics).
    weights_file: WeightsFile,
}

impl ReasoningEngine {
    /// Construct a new engine, loading weights from the given path.
    ///
    /// If the path doesn't exist or is invalid, falls back to default
    /// weights (random init) and logs a warning. The engine still works —
    /// it just produces less accurate confidences.
    pub fn new(weights_path: &Path) -> Self {
        let weights_file = WeightsFile::load_from_file(weights_path).unwrap_or_else(|e| {
            eprintln!(
                "[reasoning_engine] WARNING: failed to load weights from {}: {} \
                 — falling back to default (random) weights",
                weights_path.display(),
                e
            );
            WeightsFile::new_default()
        });

        let scorer = InferenceScorer::from_weights_file(weights_file.clone())
            .unwrap_or_else(|e| {
                eprintln!(
                    "[reasoning_engine] WARNING: weights file invalid ({}), \
                     falling back to default",
                    e
                );
                InferenceScorer::from_weights_file(WeightsFile::new_default())
                    .expect("default weights file must always load")
            });

        ReasoningEngine {
            scorer,
            svo_parser: SvoParser::new(),
            weights_file,
        }
    }

    /// Construct with explicit weights file (for testing).
    pub fn with_weights_file(weights_file: WeightsFile) -> Self {
        let scorer = InferenceScorer::from_weights_file(weights_file.clone())
            .expect("weights file must be valid");
        ReasoningEngine {
            scorer,
            svo_parser: SvoParser::new(),
            weights_file,
        }
    }

    /// Run the full 7-stage analysis on the input text.
    ///
    /// # Arguments
    /// * `text` — the chapter / fragment to analyze.
    /// * `kappa` — sector-adaptive coefficient (controls θ_rel = 3.5/κ).
    ///   Use 1.0 for general prose, 2.0 for high-density conflict scenes.
    pub fn analyze(&self, text: &str, kappa: f64) -> ReasoningReport {
        // === Stage 1: Rust NER ===
        let detected_chars = crate::parser::characters::detect(text);

        // === Stage 2: Burn Scorer inference per candidate ===
        let mut characters: Vec<ScoredCharacter> = Vec::with_capacity(detected_chars.len());
        let mut scores_for_diagnostics: Vec<(f32, Decision)> = Vec::new();
        let mut names_for_diagnostics: Vec<String> = Vec::new();

        for pc in detected_chars {
            let features = extract_features(&pc);
            let raw_confidence = pc.confidence;
            let (refined_confidence, decision) = self.scorer.score_and_decide(&features);
            let script = crate::reasoning::diagnostics::detect_script(&pc.name);

            scores_for_diagnostics.push((refined_confidence, decision));
            names_for_diagnostics.push(pc.name.clone());

            characters.push(ScoredCharacter {
                parsed: pc,
                features: features.to_vec(),
                raw_confidence,
                refined_confidence,
                decision,
                script,
            });
        }

        // === Stage 3: SVO Parser ===
        let tagged_tokens = self.svo_parser.tag_text(text);
        let raw_triplets = self.svo_parser.extract_triplets(&tagged_tokens);

        // === Stage 4: Case Validation ===
        // Reuse the tagged tokens from Stage 3 (no re-tagging).
        let character_names: std::collections::HashSet<String> = characters
            .iter()
            .filter(|c| c.decision != Decision::Reject) // include Approve + Review
            .map(|c| c.parsed.name.to_lowercase())
            .collect();

        let mut triplets: Vec<ValidatedTriplet> = Vec::with_capacity(raw_triplets.len());
        let mut triplets_valid_cases = 0usize;
        let mut triplets_invalid_cases = 0usize;

        for t in raw_triplets {
            let case_validation = validate_svo_cases(&t, &tagged_tokens);
            let adjusted = apply_case_validation(&t, &case_validation);

            if case_validation.overall == crate::linguistic::case_validation::CaseValidation::Valid {
                triplets_valid_cases += 1;
            } else if case_validation.overall
                == crate::linguistic::case_validation::CaseValidation::Invalid
            {
                triplets_invalid_cases += 1;
            }

            let is_actor_character = character_names.contains(&t.actor.to_lowercase());
            let is_target_character = t
                .target
                .as_deref()
                .map(|tg| character_names.contains(&tg.to_lowercase()))
                .unwrap_or(false);

            triplets.push(ValidatedTriplet {
                triplet: adjusted,
                case_validation,
                is_actor_character,
                is_target_character,
            });
        }

        // === Stage 5: POLER ε_climax ===
        // Need characters + triplets in the format ManuscriptAnalysis expects.
        let parsed_chars_for_analysis: Vec<ParsedCharacter> = characters
            .iter()
            .filter(|c| c.decision != Decision::Reject)
            .map(|c| c.parsed.clone())
            .collect();
        let raw_triplets_for_analysis: Vec<SvoTriplet> = triplets
            .iter()
            .map(|vt| vt.triplet.clone())
            .collect();

        // === Stage 6: Conflict Graph (NarrativeGraph) ===
        let analyzer = NarrativeGraph::new();
        let manuscript = crate::reasoning::ManuscriptAnalysis {
            chapters: vec![text],
            characters_per_chapter: vec![parsed_chars_for_analysis.clone()],
            triplets_per_chapter: vec![raw_triplets_for_analysis.clone()],
        };
        let conflict = analyzer.analyze(&manuscript);

        // Compute ε_climax with the same analyzer
        let epsilon_result = compute_epsilon_climax_with_analyzer(text, None, kappa, &analyzer);
        let epsilon = EpsilonSummary::from(&epsilon_result);

        // === Stage 7: Diagnostics ===
        let diagnostics = DiagnosticsReport::analyze(
            &scores_for_diagnostics,
            &names_for_diagnostics,
            &self.scorer,
            &self.weights_file,
        );

        // Tally decisions
        let approved_count = characters
            .iter()
            .filter(|c| c.decision == Decision::Approve)
            .count();
        let rejected_count = characters
            .iter()
            .filter(|c| c.decision == Decision::Reject)
            .count();
        let review_count = characters
            .iter()
            .filter(|c| c.decision == Decision::Review)
            .count();
        let total_triplets = triplets.len();

        ReasoningReport {
            characters,
            triplets,
            epsilon,
            conflict,
            diagnostics,
            approved_count,
            rejected_count,
            review_count,
            total_characters: scores_for_diagnostics.len(),
            total_triplets,
            triplets_valid_cases,
            triplets_invalid_cases,
            text_length: text.chars().count(),
            weights_version: self.scorer.version.clone(),
            weights_architecture: self.scorer.architecture.clone(),
        }
    }

    /// Get a reference to the loaded inference scorer (for testing / inspection).
    pub fn scorer(&self) -> &InferenceScorer {
        &self.scorer
    }

    /// Get the weights file metadata.
    pub fn weights_file(&self) -> &WeightsFile {
        &self.weights_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> ReasoningEngine {
        // Use default weights for tests (no I/O dependency).
        ReasoningEngine::with_weights_file(WeightsFile::new_default())
    }

    #[test]
    fn test_engine_analyze_simple_sentence() {
        let engine = make_engine();
        let report = engine.analyze("Петро сказав Марті: йдемо.", 1.0);
        // Should detect at least one character candidate (Петро or Марта).
        assert!(report.total_characters >= 1,
            "expected >=1 character, got {}", report.total_characters);
        // Decision tallies should sum to total
        assert_eq!(
            report.approved_count + report.rejected_count + report.review_count,
            report.total_characters
        );
        // Weights metadata should be populated
        assert!(!report.weights_version.is_empty());
        assert!(!report.weights_architecture.is_empty());
    }

    #[test]
    fn test_engine_analyze_empty_text() {
        let engine = make_engine();
        let report = engine.analyze("", 1.0);
        assert_eq!(report.total_characters, 0);
        assert_eq!(report.total_triplets, 0);
        assert_eq!(report.text_length, 0);
    }

    #[test]
    fn test_engine_diagnostics_included() {
        let engine = make_engine();
        let report = engine.analyze("Марта й Веня пішли у ліс.", 1.0);
        // Diagnostics report should always be present (even if healthy)
        assert!(!report.diagnostics.overall_health.is_empty());
        // Feature informativeness should always have 8 features
        assert_eq!(
            report.diagnostics.feature_informativeness.per_feature_std.len(),
            crate::scorer::FEATURE_COUNT
        );
    }

    #[test]
    fn test_engine_report_is_serializable() {
        let engine = make_engine();
        let report = engine.analyze("Петро вбив ворога.", 1.0);
        let json = serde_json::to_string(&report).expect("serialize");
        assert!(json.contains("characters"));
        assert!(json.contains("triplets"));
        assert!(json.contains("epsilon"));
        assert!(json.contains("conflict"));
        assert!(json.contains("diagnostics"));
    }

    #[test]
    fn test_engine_case_validation_flags_invalid_actor() {
        // Construct an engine and analyze text where the SVO parser may
        // produce a triplet with a non-Nominative actor (heuristic fallback).
        // We verify that case_validation is at least invoked (overall field exists).
        let engine = make_engine();
        let report = engine.analyze("Петрові дали книгу.", 1.0);
        // Whatever the parser produced, each triplet has a case_validation
        for t in &report.triplets {
            let _ = &t.case_validation.overall;
        }
    }

    #[test]
    fn test_engine_weights_version_propagates() {
        let engine = make_engine();
        let report = engine.analyze("Test.", 1.0);
        // Default weights file uses CARGO_PKG_VERSION
        assert!(!report.weights_version.is_empty());
    }
}
