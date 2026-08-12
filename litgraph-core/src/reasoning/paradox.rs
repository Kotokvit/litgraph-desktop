//! Temporal Paradox Detector (Layer E.2).
//!
//! Identifies narrative inconsistencies across chapters:
//!
//! - **Dead-Speaking Paradox**: A character marked as deceased in chapter N
//!   acts or speaks in chapter N+k (k > 0).
//! - **Spatial Teleportation Paradox**: A character appears in two non-adjacent
//!   locations in consecutive chapters without a transit event.
//!
//! ## Detection signals
//!
//! - **Death markers**: keywords `"помер"`, `"загинув"`, `"вбитий"`, `"померла"`,
//!   `"смерть"` immediately after a character's name.
//! - **Speech markers**: `"сказав"`, `"відповів"`, `"промовив"`, `"крикнув"`
//!   with the character as subject.
//! - **Action markers**: any SVO triplet where the deceased character is the
//!   `actor` after the death chapter.
//!
//! ## Limitations
//!
//! This is a heuristic detector — false positives are possible (e.g., flashbacks,
//! dream sequences, named-after descendants). False positives are NOT bugs:
//! they are *signals* that Layer G (LLM Reasoning) should resolve by proposing
//! a hypothesis (flashback / dream / resurrection / disguise).

use crate::linguistic::svo_parser::SvoTriplet;
use crate::parser::characters::ParsedCharacter;
use crate::reasoning::ManuscriptAnalysis;

/// Type of temporal paradox detected.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParadoxKind {
    /// Character acts/speaks after being marked as deceased.
    DeadSpeaking,
    /// Character appears in two non-adjacent locations without transit.
    SpatialTeleportation,
}

/// A single detected temporal paradox.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Paradox {
    /// Type of paradox.
    pub kind: ParadoxKind,
    /// Character name involved.
    pub character: String,
    /// Chapter index where the paradox manifests (e.g., where dead character speaks).
    pub chapter_idx: usize,
    /// Chapter index where the death was recorded (for DeadSpeaking), or the
    /// previous location chapter (for SpatialTeleportation).
    pub origin_chapter_idx: usize,
    /// Human-readable explanation.
    pub explanation: String,
}

/// Ukrainian death-marker verbs (past tense, masculine + feminine).
const DEATH_MARKERS: &[&str] = &[
    "помер", "померла", "померли",
    "загинув", "загинула", "загинули",
    "вбитий", "вбита", "вбиті",
    "померла", "помер",
    "загинув", "загинула",
    "стратили", "стратила", "стратив",
    "поховано", "похований", "похована",
];

/// Ukrainian speech-verb lemmas (any form) — used to detect speaking after death.
const SPEECH_MARKERS: &[&str] = &[
    "сказати", "сказав", "сказала",
    "відповісти", "відповів", "відповіла",
    "промовити", "промовив", "промовила",
    "крикнути", "крикнув", "крикнула",
    "спитати", "спитав", "спитала",
    "запитати", "запитав", "запитала",
    "шепнути", "шепнув", "шепнула",
    "вигукнути", "вигукнув", "вигукнула",
];

/// Detect temporal paradoxes in a manuscript.
///
/// ## Algorithm (Dead-Speaking)
///
/// For each chapter N and each character C mentioned in N:
/// 1. Scan text for `<Name> <death_marker>` patterns (case-insensitive).
/// 2. If found, record `(C, N)` as a death event.
/// 3. For all chapters M > N, check if C is the actor of any SVO triplet
///    or appears with a speech marker.
/// 4. If yes, emit a [`Paradox`] of kind [`DeadSpeaking`](ParadoxKind::DeadSpeaking).
///
/// ## Algorithm (Spatial Teleportation) — *placeholder*
///
/// Not yet implemented. Requires Layer F location normalization to detect
/// non-adjacent location pairs.
#[derive(Debug, Default)]
pub struct ParadoxDetector {
    /// Recorded death events: (character_name, chapter_idx) pairs.
    deaths: Vec<(String, usize)>,
}

impl ParadoxDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run paradox detection on a manuscript analysis.
    pub fn detect(&mut self, manuscript: &ManuscriptAnalysis<'_>) -> Vec<Paradox> {
        self.deaths.clear();
        let mut paradoxes = Vec::new();

        // Phase 1: scan for death events
        for (ch_idx, chapter_text) in manuscript.chapters.iter().enumerate() {
            for c in &manuscript.characters_per_chapter[ch_idx] {
                if self.has_death_marker(chapter_text, &c.name) {
                    self.deaths.push((c.name.clone(), ch_idx));
                }
            }
        }

        // Phase 2: scan post-death chapters for dead-speaking
        for (ch_idx, chapter_text) in manuscript.chapters.iter().enumerate() {
            for triplets in &manuscript.triplets_per_chapter[ch_idx..=ch_idx] {
                for t in triplets {
                    if let Some(death_ch) = self.find_death_before(&t.actor, ch_idx) {
                        paradoxes.push(Paradox {
                            kind: ParadoxKind::DeadSpeaking,
                            character: t.actor.clone(),
                            chapter_idx: ch_idx,
                            origin_chapter_idx: death_ch,
                            explanation: format!(
                                "Character '{}' acts in chapter {} but died in chapter {}",
                                t.actor, ch_idx, death_ch
                            ),
                        });
                        break; // one paradox per (character, chapter) pair
                    }
                }
            }
            // Also scan text for speech markers after death
            for (dead_char, death_ch) in &self.deaths {
                if *death_ch < ch_idx && self.has_speech_marker(chapter_text, dead_char) {
                    // Avoid duplicate if already added via triplet check
                    let already = paradoxes.iter().any(|p| {
                        p.character == *dead_char && p.chapter_idx == ch_idx
                    });
                    if !already {
                        paradoxes.push(Paradox {
                            kind: ParadoxKind::DeadSpeaking,
                            character: dead_char.clone(),
                            chapter_idx: ch_idx,
                            origin_chapter_idx: *death_ch,
                            explanation: format!(
                                "Character '{}' speaks in chapter {} but died in chapter {}",
                                dead_char, ch_idx, death_ch
                            ),
                        });
                    }
                }
            }
        }

        paradoxes
    }

    /// Check if `text` contains a death marker immediately after `name`.
    fn has_death_marker(&self, text: &str, name: &str) -> bool {
        let lower = text.to_lowercase();
        let name_lower = name.to_lowercase();
        for marker in DEATH_MARKERS {
            // Look for patterns: "<name> <marker>" within ~40 chars after name.
            if let Some(name_pos) = lower.find(&name_lower) {
                let after_name = &lower[name_pos + name_lower.len()..];
                let window: String = after_name.chars().take(40).collect();
                // Strip trailing punctuation so "помер." matches "помер".
                let matches = window
                    .split_whitespace()
                    .any(|w| w.trim_end_matches(|c: char| !c.is_alphanumeric()) == *marker);
                if matches {
                    return true;
                }
            }
        }
        false
    }

    /// Check if `text` contains a speech marker immediately after `name`.
    fn has_speech_marker(&self, text: &str, name: &str) -> bool {
        let lower = text.to_lowercase();
        let name_lower = name.to_lowercase();
        for marker in SPEECH_MARKERS {
            if let Some(name_pos) = lower.find(&name_lower) {
                let after_name = &lower[name_pos + name_lower.len()..];
                let window: String = after_name.chars().take(40).collect();
                let matches = window
                    .split_whitespace()
                    .any(|w| w.trim_end_matches(|c: char| !c.is_alphanumeric()) == *marker);
                if matches {
                    return true;
                }
            }
        }
        false
    }

    /// Find the most recent death chapter for `character` before chapter `current_ch`.
    fn find_death_before(&self, character: &str, current_ch: usize) -> Option<usize> {
        self.deaths
            .iter()
            .filter(|(name, ch)| name.eq_ignore_ascii_case(character) && *ch < current_ch)
            .map(|(_, ch)| *ch)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::characters::EntityType;

    fn char(name: &str) -> ParsedCharacter {
        use crate::parser::characters::{SIGNAL_CAPITALIZED, SIGNAL_SPEECH_VERB};
        // Test helper: 2 signals (cap + speech) → confidence 0.7 (single-token)
        let signals = SIGNAL_CAPITALIZED | SIGNAL_SPEECH_VERB;
        ParsedCharacter {
            name: name.to_string(),
            aliases: vec![],
            count: 1,
            description: String::new(),
            speech_count: 1,
            direct_count: 0,
            reason: "test".to_string(),
            entity_type: EntityType::Character,
            evidence_signals: signals,
            confidence: ParsedCharacter::confidence_from_signals(signals, true),
            mention_starts: vec![],
            first_mention: None,
            nominative_count: 0,
            accusative_count: 0,
            genitive_negated_count: 0,
        }
    }

    fn triplet(actor: &str, verb: &str, target: Option<&str>) -> SvoTriplet {
        SvoTriplet {
            actor: actor.to_string(),
            verb: verb.to_string(),
            target: target.map(|s| s.to_string()),
            instrument: None,
            location: None,
            polarity: true,
            confidence: 0.9,
        }
    }

    #[test]
    fn test_no_paradox_when_no_death() {
        let mut det = ParadoxDetector::new();
        let manuscript = ManuscriptAnalysis {
            chapters: vec!["Петро вбив ворога."],
            characters_per_chapter: vec![vec![char("Петро"), char("ворог")]],
            triplets_per_chapter: vec![vec![triplet("Петро", "вбити", Some("ворога"))]],
        };
        let paradoxes = det.detect(&manuscript);
        assert!(paradoxes.is_empty(), "No death ⇒ no paradox");
    }

    #[test]
    fn test_dead_speaking_paradox_detected() {
        let mut det = ParadoxDetector::new();
        let manuscript = ManuscriptAnalysis {
            chapters: vec![
                "Петро помер у бою.",
                "Петро сказав слова прощання.",
            ],
            characters_per_chapter: vec![
                vec![char("Петро")],
                vec![char("Петро")],
            ],
            triplets_per_chapter: vec![vec![], vec![]],
        };
        let paradoxes = det.detect(&manuscript);
        assert_eq!(paradoxes.len(), 1, "Should detect dead-speaking paradox");
        assert_eq!(paradoxes[0].kind, ParadoxKind::DeadSpeaking);
        assert_eq!(paradoxes[0].character, "Петро");
        assert_eq!(paradoxes[0].chapter_idx, 1);
        assert_eq!(paradoxes[0].origin_chapter_idx, 0);
    }

    #[test]
    fn test_dead_acting_via_svo_triplet() {
        let mut det = ParadoxDetector::new();
        let manuscript = ManuscriptAnalysis {
            chapters: vec![
                "Петро загинув у бою.",
                "Петро вбив нового ворога.",
            ],
            characters_per_chapter: vec![
                vec![char("Петро")],
                vec![char("Петро"), char("ворог")],
            ],
            triplets_per_chapter: vec![
                vec![],
                vec![triplet("Петро", "вбити", Some("ворога"))],
            ],
        };
        let paradoxes = det.detect(&manuscript);
        assert_eq!(paradoxes.len(), 1);
        assert_eq!(paradoxes[0].kind, ParadoxKind::DeadSpeaking);
    }

    #[test]
    fn test_no_paradox_when_character_alive_in_same_chapter() {
        // Death and action in the same chapter is NOT a paradox (could be
        // sequential: died later in the chapter).
        let mut det = ParadoxDetector::new();
        let manuscript = ManuscriptAnalysis {
            chapters: vec!["Петро помер. Петро сказав останнє слово."],
            characters_per_chapter: vec![vec![char("Петро")]],
            triplets_per_chapter: vec![vec![]],
        };
        let paradoxes = det.detect(&manuscript);
        assert!(paradoxes.is_empty(), "Same-chapter death + speech is not a paradox");
    }

    #[test]
    fn test_multiple_deaths_track_separately() {
        let mut det = ParadoxDetector::new();
        let manuscript = ManuscriptAnalysis {
            chapters: vec![
                "Петро помер. Марта загинула.",
                "Петро сказав. Марта відповіла.",
            ],
            characters_per_chapter: vec![
                vec![char("Петро"), char("Марта")],
                vec![char("Петро"), char("Марта")],
            ],
            triplets_per_chapter: vec![vec![], vec![]],
        };
        let paradoxes = det.detect(&manuscript);
        assert_eq!(paradoxes.len(), 2, "Both characters should produce paradoxes");
    }

    #[test]
    fn test_feminine_death_marker() {
        let mut det = ParadoxDetector::new();
        let manuscript = ManuscriptAnalysis {
            chapters: vec![
                "Марта померла вночі.",
                "Марта сказала прощання.",
            ],
            characters_per_chapter: vec![vec![char("Марта")], vec![char("Марта")]],
            triplets_per_chapter: vec![vec![], vec![]],
        };
        let paradoxes = det.detect(&manuscript);
        assert_eq!(paradoxes.len(), 1, "Feminine 'померла' must be detected");
    }
}
