//! v0.6.0 / Phase 2 Step 3: Feature extraction for Burn scorer.
//!
//! Converts a `ParsedCharacter` into a fixed-length numerical feature vector
//! that the Burn MLP can consume. The features are designed to be:
//!   - **cheap to compute** (no allocation in hot path)
//!   - **deterministic** (same input → same features)
//!   - **meaningful for character-vs-concept discrimination**
//!
//! # Feature inventory (8 features, all in [0.0, 1.0])
//!
//! | # | Feature | Source field | Range | Rationale |
//! |---|---------|--------------|-------|-----------|
//! | 0 | `is_capitalized` | `evidence_signals & SIGNAL_CAPITALIZED` | {0, 1} | Required signal — every Character has it |
//! | 1 | `has_speech_verb` | `evidence_signals & SIGNAL_SPEECH_VERB` | {0, 1} | Strong signal — character speaks |
//! | 2 | `has_direct_address` | `evidence_signals & SIGNAL_DIRECT_ADDRESS` | {0, 1} | Strong signal — addressed directly |
//! | 3 | `is_single_token` | `is_single_token()` | {0, 1} | Multi-token → likely FIO → Python |
//! | 4 | `mention_count_norm` | `mention_starts.len()` / max | [0, 1] | Frequency normalized to 20+ mentions |
//! | 5 | `speech_count_norm` | `speech_count` / max | [0, 1] | How often speaks, normalized to 10+ |
//! | 6 | `direct_count_norm` | `direct_count` / max | [0, 1] | How often addressed, normalized to 5+ |
//! | 7 | `is_character_type` | `entity_type == Character` | {0, 1} | What Rust parser thinks it is |
//!
//! # Future expansion
//!
//! Burn model dimensions are hardcoded to 8 for now. When we add features
//! (e.g. POS tags from Natasha, gender validation, FIO morphology), the MLP
//! architecture must be updated and weights retrained.

use crate::parser::characters::{EntityType, ParsedCharacter, SIGNAL_CAPITALIZED, SIGNAL_DIRECT_ADDRESS, SIGNAL_SPEECH_VERB};

/// Number of features extracted per `ParsedCharacter`.
/// MUST match the MLP input dimension in `model.rs`.
pub const FEATURE_COUNT: usize = 8;

/// Normalization constants — chosen so typical values map to ~[0.2, 0.8].
/// Mentions above these thresholds saturate to 1.0.
const MAX_MENTIONS: f32 = 20.0;
const MAX_SPEECH: f32 = 10.0;
const MAX_DIRECT: f32 = 5.0;

/// Fixed-length feature vector for Burn model input.
pub type FeatureVector = [f32; FEATURE_COUNT];

/// Extract 8 features from a `ParsedCharacter`.
///
/// All features are in [0.0, 1.0]. Binary signals are 0.0 or 1.0.
/// Continuous signals are normalized by typical maximums.
///
/// # Example
///
/// ```
/// use litgraph_core::parser::characters::ParsedCharacter;
/// use litgraph_core::scorer::extract_features;
///
/// let pc = ParsedCharacter {
///     name: "Борис".to_string(),
///     aliases: vec!["Борис".to_string()],
///     count: 2,
///     description: String::new(),
///     speech_count: 1,
///     direct_count: 0,
///     reason: String::new(),
///     entity_type: litgraph_core::parser::characters::EntityType::Character,
///     evidence_signals: 0b011, // cap + speech
///     confidence: 0.7,
///     mention_starts: vec![10, 50],
///     first_mention: Some(10),
/// };
///
/// let features = extract_features(&pc);
/// assert_eq!(features.len(), 8);
/// assert_eq!(features[0], 1.0); // is_capitalized
/// assert_eq!(features[1], 1.0); // has_speech_verb
/// assert_eq!(features[2], 0.0); // has_direct_address
/// ```
pub fn extract_features(pc: &ParsedCharacter) -> FeatureVector {
    let signals = pc.evidence_signals;
    let mention_count = pc.mention_starts.len() as f32;

    [
        // Feature 0: is_capitalized (bit 0 of evidence_signals)
        ((signals & SIGNAL_CAPITALIZED) != 0) as u8 as f32,
        // Feature 1: has_speech_verb (bit 1)
        ((signals & SIGNAL_SPEECH_VERB) != 0) as u8 as f32,
        // Feature 2: has_direct_address (bit 2)
        ((signals & SIGNAL_DIRECT_ADDRESS) != 0) as u8 as f32,
        // Feature 3: is_single_token
        pc.is_single_token() as u8 as f32,
        // Feature 4: mention_count normalized
        (mention_count / MAX_MENTIONS).min(1.0),
        // Feature 5: speech_count normalized
        (pc.speech_count as f32 / MAX_SPEECH).min(1.0),
        // Feature 6: direct_count normalized
        (pc.direct_count as f32 / MAX_DIRECT).min(1.0),
        // Feature 7: is_character_type
        (pc.entity_type == EntityType::Character) as u8 as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::characters::{EntityType, ParsedCharacter};

    fn make_pc(signals: u8, speech: usize, direct: usize, mentions: usize, single: bool) -> ParsedCharacter {
        let name = if single { "Борис".to_string() } else { "Иван Петров".to_string() };
        ParsedCharacter {
            name,
            aliases: vec!["Борис".to_string()],
            count: mentions,
            description: String::new(),
            speech_count: speech,
            direct_count: direct,
            reason: String::new(),
            entity_type: EntityType::Character,
            evidence_signals: signals,
            confidence: 0.7,
            mention_starts: (0..mentions).map(|i| i * 10).collect(),
            first_mention: if mentions > 0 { Some(0) } else { None },
        }
    }

    #[test]
    fn test_feature_count_matches_const() {
        let pc = make_pc(0b011, 1, 0, 1, true);
        let features = extract_features(&pc);
        assert_eq!(features.len(), FEATURE_COUNT);
        assert_eq!(FEATURE_COUNT, 8);
    }

    #[test]
    fn test_binary_features_set_correctly() {
        // signals = cap (1) + speech (2) + direct (4) = 0b111 = 7
        let pc = make_pc(0b111, 1, 1, 1, true);
        let features = extract_features(&pc);

        assert_eq!(features[0], 1.0, "is_capitalized");
        assert_eq!(features[1], 1.0, "has_speech_verb");
        assert_eq!(features[2], 1.0, "has_direct_address");
        assert_eq!(features[3], 1.0, "is_single_token (single)");
        assert_eq!(features[7], 1.0, "is_character_type");
    }

    #[test]
    fn test_binary_features_clear_when_signals_absent() {
        // signals = 0 → no signals set
        let pc = make_pc(0b000, 0, 0, 1, true);
        let features = extract_features(&pc);

        assert_eq!(features[0], 0.0, "is_capitalized");
        assert_eq!(features[1], 0.0, "has_speech_verb");
        assert_eq!(features[2], 0.0, "has_direct_address");
    }

    #[test]
    fn test_single_token_vs_multi_token() {
        let single = make_pc(0b011, 1, 0, 1, true);
        let multi = make_pc(0b011, 1, 0, 1, false);

        assert_eq!(extract_features(&single)[3], 1.0, "single-token → 1.0");
        assert_eq!(extract_features(&multi)[3], 0.0, "multi-token → 0.0");
    }

    #[test]
    fn test_mention_count_normalization() {
        // 1 mention → 1/20 = 0.05
        let pc1 = make_pc(0b001, 0, 0, 1, true);
        assert!((extract_features(&pc1)[4] - 0.05).abs() < 1e-6);

        // 20 mentions → 20/20 = 1.0
        let pc20 = make_pc(0b001, 0, 0, 20, true);
        assert!((extract_features(&pc20)[4] - 1.0).abs() < 1e-6);

        // 50 mentions → saturates to 1.0
        let pc50 = make_pc(0b001, 0, 0, 50, true);
        assert!((extract_features(&pc50)[4] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_speech_count_normalization() {
        // 1 speech → 1/10 = 0.1
        let pc = make_pc(0b011, 1, 0, 1, true);
        assert!((extract_features(&pc)[5] - 0.1).abs() < 1e-6);

        // 10+ speech → 1.0
        let pc10 = make_pc(0b011, 15, 0, 1, true);
        assert!((extract_features(&pc10)[5] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_all_features_in_unit_range() {
        // Edge cases: empty, max signals, max counts
        let cases = vec![
            make_pc(0b000, 0, 0, 0, true),
            make_pc(0b111, 100, 100, 100, false),
            make_pc(0b001, 5, 3, 15, true),
        ];

        for pc in cases {
            let features = extract_features(&pc);
            for (i, &f) in features.iter().enumerate() {
                assert!(f >= 0.0 && f <= 1.0,
                    "Feature {} out of [0,1]: {} for pc {:?}", i, f, pc.name);
            }
        }
    }

    #[test]
    fn test_concept_vs_character_feature() {
        let mut char_pc = make_pc(0b011, 1, 0, 1, true);
        let mut concept_pc = make_pc(0b001, 0, 0, 5, true);
        concept_pc.entity_type = EntityType::Concept;

        assert_eq!(extract_features(&char_pc)[7], 1.0, "Character → 1.0");
        assert_eq!(extract_features(&concept_pc)[7], 0.0, "Concept → 0.0");

        // Sanity: unused mut warning suppression
        char_pc.count = 1;
    }
}
