//! Stub conflict analyzer for testing & prototyping.
//!
//! Returns a deterministic synthetic Ω_conf based on conflict-keyword density.
//! Useful for:
//! - Unit-testing Layer D's `compute_epsilon_climax_with_analyzer` without
//!   requiring the full SVO + characters pipeline.
//! - Quick smoke tests of the GUI rendering of ε_climax values.
//!
//! ## Algorithm
//!
//! Counts occurrences of conflict keywords ("війна", "битва", "зрада", etc.)
//! across all chapters. Returns Ω_conf = `weight × keyword_count`.
//!
//! This is NOT a substitute for [`NarrativeGraph`](super::narrative_graph::NarrativeGraph) —
//! it ignores character interactions entirely. It exists only as a
//! dependency-injection placeholder for tests.

use super::{ConflictAnalyzer, ConflictReport, ManuscriptAnalysis};

/// Ukrainian conflict keywords (lowercase).
const CONFLICT_KEYWORDS: &[&str] = &[
    "війна", "битва", "бій", "зрада", "зрадник",
    "вбивство", "кров", "смерть", "ненависть",
    "ворог", "вороги", "противник", "ворожнеча",
    "містити", "помста", "відплата",
    "напад", "облога", "атака",
];

/// Stub conflict analyzer: counts conflict keywords, returns Ω_conf = 0.5 × count.
#[derive(Debug, Default, Clone)]
pub struct StubConflictAnalyzer {
    /// Multiplier applied to keyword count. Default = 0.5 (gives Ω_conf ∈ [0, 50] for ~100 keywords).
    pub weight: f64,
}

impl StubConflictAnalyzer {
    /// Create a stub with default weight = 0.5.
    pub fn new() -> Self {
        Self { weight: 0.5 }
    }

    /// Create a stub with custom weight (useful for parameterized tests).
    pub fn with_weight(weight: f64) -> Self {
        Self { weight }
    }

    /// Count conflict keywords in a text (case-insensitive).
    pub fn count_keywords(&self, text: &str) -> usize {
        let lower = text.to_lowercase();
        CONFLICT_KEYWORDS
            .iter()
            .map(|kw| lower.matches(kw).count())
            .sum()
    }
}

impl ConflictAnalyzer for StubConflictAnalyzer {
    fn analyze(&self, manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport {
        let total_keywords: usize = manuscript
            .chapters
            .iter()
            .map(|ch| self.count_keywords(ch))
            .sum();
        let omega_conf = self.weight * total_keywords as f64;
        ConflictReport {
            omega_conf,
            spectral_radius: 0.0, // Stub doesn't compute ρ
            node_count: 0,
            edge_count: 0,
            paradoxes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linguistic::svo_parser::SvoTriplet;
    use crate::parser::characters::EntityType;

    fn make_manuscript(text: &str) -> ManuscriptAnalysis<'_> {
        ManuscriptAnalysis {
            chapters: vec![text],
            characters_per_chapter: vec![vec![]],
            triplets_per_chapter: vec![vec![]],
        }
    }

    #[test]
    fn test_empty_text_zero_omega() {
        let stub = StubConflictAnalyzer::new();
        let m = make_manuscript("");
        let r = stub.analyze(&m);
        assert_eq!(r.omega_conf, 0.0);
    }

    #[test]
    fn test_keyword_count_basic() {
        let stub = StubConflictAnalyzer::new();
        let count = stub.count_keywords("Війна і битва. Зрада ворога.");
        assert_eq!(count, 4, "війна + битва + зрада + ворог");
    }

    #[test]
    fn test_omega_conf_proportional_to_keywords() {
        let stub = StubConflictAnalyzer::with_weight(1.0);
        let m1 = make_manuscript("Звичайний текст без ключових слів.");
        let m2 = make_manuscript("Війна! Битва! Зрада!");
        let r1 = stub.analyze(&m1);
        let r2 = stub.analyze(&m2);
        assert_eq!(r1.omega_conf, 0.0);
        assert_eq!(r2.omega_conf, 3.0, "3 keywords × weight 1.0");
    }

    #[test]
    fn test_determinism_stub() {
        let stub = StubConflictAnalyzer::new();
        let m = make_manuscript("Війна, битва, зрада.");
        let r1 = stub.analyze(&m);
        let r2 = stub.analyze(&m);
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_case_insensitive_matching() {
        let stub = StubConflictAnalyzer::new();
        let count = stub.count_keywords("ВІЙНА Війна війна");
        assert_eq!(count, 3);
    }
}
