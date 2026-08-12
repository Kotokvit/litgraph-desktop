//! v0.7.0 / Reasoning Engine: SVO role validation via Ukrainian grammatical cases.
//!
//! The SVO parser (`linguistic::svo_parser`) extracts triplets from tagged
//! tokens, but it does not verify that the grammatical case of each role
//! matches what that role requires. For example:
//!
//! - Subject (Actor) should be in **Nominative** (`v_naz`).
//! - Direct Object (Target) should be in **Accusative** (`v_zna`) or, under
//!   negation, **Genitive** (`v_rod`).
//! - Instrument should be in **Instrumental** (`v_oru`).
//! - Location should be in **Locative** (`v_mis`) — usually with a preposition.
//!
//! When the parser falls back to heuristics (e.g. when the POS tagger is
//! unsure), it may pick a token whose case contradicts its assigned role.
//! This module flags such mismatches so the Reasoning Engine can:
//!
//! 1. Down-weight the triplet's `confidence` (don't propagate bad evidence).
//! 2. Surface the mismatch as a diagnostic for the user / reviewer.
//! 3. Optionally retry the parse with stricter POS disambiguation.
//!
//! # Algorithm
//!
//! For each `SvoTriplet`, look up the actor / target / instrument / location
//! tokens in the original `Vec<TaggedToken>` (passed by reference from the
//! SVO parser). If found, compare the token's `PosTag.case` against the role's
//! required case set. Emit a [`CaseValidation`] verdict.
//!
//! When the token is not found (e.g. triplet came from heuristic fallback),
//! emit `CaseValidation::Unknown` — we cannot validate, but we also cannot
//! reject. The Reasoning Engine treats `Unknown` as "neutral, neither boost
//! nor penalty".

use serde::{Deserialize, Serialize};

use crate::linguistic::pos_tagger::{GrammaticalCase, TaggedToken};
use crate::linguistic::svo_parser::SvoTriplet;

/// Verdict for a single SVO role's case check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseValidation {
    /// Case matches the role's requirement (e.g. Actor in Nominative).
    Valid,
    /// Case contradicts the role (e.g. Actor in Dative — cannot be subject).
    Invalid,
    /// Token not found in tagged input, or POS tag has no case info.
    /// Used when the parser heuristic produced a triplet but the token
    /// wasn't found in the original tagged slice (defensive).
    Unknown,
}

impl CaseValidation {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaseValidation::Valid => "valid",
            CaseValidation::Invalid => "invalid",
            CaseValidation::Unknown => "unknown",
        }
    }

    /// Confidence multiplier applied to the triplet's confidence.
    /// `Valid` → 1.0 (no change), `Invalid` → 0.3 (heavy penalty),
    /// `Unknown` → 0.8 (small penalty, since we can't confirm correctness).
    pub fn confidence_multiplier(&self) -> f64 {
        match self {
            CaseValidation::Valid => 1.0,
            CaseValidation::Invalid => 0.3,
            CaseValidation::Unknown => 0.8,
        }
    }
}

/// Full validation result for one SVO triplet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseValidationResult {
    /// Verdict for the Actor (subject) role.
    pub actor: CaseValidation,
    /// Detected case of the actor token, if any.
    pub actor_case: Option<GrammaticalCase>,
    /// Verdict for the Target (direct object) role, if present.
    pub target: Option<CaseValidation>,
    /// Detected case of the target token, if any.
    pub target_case: Option<GrammaticalCase>,
    /// Verdict for the Instrument role, if present.
    pub instrument: Option<CaseValidation>,
    /// Verdict for the Location role, if present.
    pub location: Option<CaseValidation>,
    /// Overall verdict: `Valid` only if all present roles are `Valid`.
    /// `Invalid` if any role is `Invalid`. Otherwise `Unknown`.
    pub overall: CaseValidation,
    /// Multiplier to apply to `triplet.confidence` (product of role multipliers).
    pub confidence_multiplier: f64,
}

/// Required cases for each SVO role.
///
/// Subject must be Nominative (with pronoun fallback to Nominative-only).
/// Direct object: Accusative, or Genitive under negation.
/// Instrument: Instrumental.
/// Location: Locative (prepositional phrase).
const SUBJECT_CASES: &[GrammaticalCase] = &[GrammaticalCase::Nominative];
const OBJECT_CASES_AFFIRM: &[GrammaticalCase] = &[GrammaticalCase::Accusative];
const OBJECT_CASES_NEGATED: &[GrammaticalCase] =
    &[GrammaticalCase::Accusative, GrammaticalCase::Genitive];
const INSTRUMENT_CASES: &[GrammaticalCase] = &[GrammaticalCase::Instrumental];
const LOCATION_CASES: &[GrammaticalCase] = &[GrammaticalCase::Locative];

/// Find a token in the tagged slice whose word matches `name` (case-insensitive).
/// Returns the first match. We need this because SVO parser may capitalize
/// ("петро" → "Петро") or change form ("ворога" → "ворог").
fn find_token<'a>(tokens: &'a [TaggedToken], name: &str) -> Option<&'a TaggedToken> {
    let name_lower = name.to_lowercase();
    tokens
        .iter()
        .find(|t| t.word.to_lowercase() == name_lower)
        .or_else(|| {
            // Substring fallback: "ворога" might match token "ворога" exactly,
            // but if the SVO parser picked "ворог" (lemma), search by prefix.
            tokens
                .iter()
                .find(|t| t.word.to_lowercase().starts_with(&name_lower))
        })
}

/// Check if a token's case is in the allowed set.
fn check_case(token: &TaggedToken, allowed: &[GrammaticalCase]) -> CaseValidation {
    match token.selected_tag.case {
        Some(c) if allowed.contains(&c) => CaseValidation::Valid,
        Some(_) => CaseValidation::Invalid,
        None => CaseValidation::Unknown,
    }
}

/// Validate the grammatical cases of all roles in a SVO triplet.
///
/// # Arguments
/// * `triplet` — the SVO triplet to validate.
/// * `tokens` — the tagged tokens from which the triplet was extracted
///   (typically the output of `pos_tagger::tag_sentence()`).
///
/// # Returns
/// A [`CaseValidationResult`] with per-role verdicts and an overall multiplier.
pub fn validate_svo_cases(
    triplet: &SvoTriplet,
    tokens: &[TaggedToken],
) -> CaseValidationResult {
    // === Actor (Subject) ===
    let actor_token = find_token(tokens, &triplet.actor);
    let (actor_verdict, actor_case) = match actor_token {
        Some(t) => {
            let v = check_case(t, SUBJECT_CASES);
            (v, t.selected_tag.case)
        }
        None => (CaseValidation::Unknown, None),
    };

    // === Target (Direct Object) ===
    let object_cases = if triplet.polarity {
        OBJECT_CASES_AFFIRM
    } else {
        OBJECT_CASES_NEGATED
    };
    let (target_verdict, target_case) = match triplet.target.as_deref() {
        Some(target_name) => match find_token(tokens, target_name) {
            Some(t) => {
                let v = check_case(t, object_cases);
                (Some(v), t.selected_tag.case)
            }
            None => (Some(CaseValidation::Unknown), None),
        },
        None => (None, None),
    };

    // === Instrument ===
    let instrument_verdict = match triplet.instrument.as_deref() {
        Some(name) => find_token(tokens, name).map(|t| check_case(t, INSTRUMENT_CASES)),
        None => None,
    };

    // === Location ===
    let location_verdict = match triplet.location.as_deref() {
        Some(name) => find_token(tokens, name).map(|t| check_case(t, LOCATION_CASES)),
        None => None,
    };

    // === Overall verdict ===
    // Valid only if all present roles are Valid. Invalid if any is Invalid.
    let mut overall = CaseValidation::Valid;
    let mut multiplier: f64 = 1.0;

    for v in [Some(actor_verdict), target_verdict, instrument_verdict, location_verdict]
        .iter()
        .copied()
        .flatten()
    {
        if v == CaseValidation::Invalid {
            overall = CaseValidation::Invalid;
        } else if v == CaseValidation::Unknown && overall != CaseValidation::Invalid {
            overall = CaseValidation::Unknown;
        }
        multiplier *= v.confidence_multiplier();
    }

    CaseValidationResult {
        actor: actor_verdict,
        actor_case,
        target: target_verdict,
        target_case,
        instrument: instrument_verdict,
        location: location_verdict,
        overall,
        confidence_multiplier: multiplier,
    }
}

/// Apply the case-validation multiplier to a triplet's confidence.
///
/// Returns a new triplet with `confidence *= multiplier`. The original
/// triplet is not mutated.
pub fn apply_case_validation(triplet: &SvoTriplet, result: &CaseValidationResult) -> SvoTriplet {
    let mut new_triplet = triplet.clone();
    new_triplet.confidence *= result.confidence_multiplier;
    new_triplet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linguistic::pos_tagger::{PosClass, PosTag, TaggedToken};
    use crate::linguistic::svo_parser::SvoTriplet;

    fn make_token(word: &str, class: PosClass, case: Option<GrammaticalCase>) -> TaggedToken {
        TaggedToken {
            word: word.to_string(),
            lemma: word.to_lowercase(),
            selected_tag: PosTag {
                class,
                case,
                gender: None,
                number: None,
                animacy: None,
                aspect: None,
                tense: None,
                raw_tag: String::new(),
            },
            candidates: vec![],
            is_disambiguated: true,
            applied_rule: None,
        }
    }

    fn make_triplet(
        actor: &str,
        verb: &str,
        target: Option<&str>,
        polarity: bool,
        confidence: f64,
    ) -> SvoTriplet {
        SvoTriplet {
            actor: actor.to_string(),
            verb: verb.to_string(),
            target: target.map(|s| s.to_string()),
            instrument: None,
            location: None,
            polarity,
            confidence,
        }
    }

    #[test]
    fn test_valid_subject_nominative() {
        let triplet = make_triplet("Петро", "вбив", Some("ворога"), true, 1.0);
        let tokens = vec![
            make_token("Петро", PosClass::Noun, Some(GrammaticalCase::Nominative)),
            make_token("вбив", PosClass::Verb, None),
            make_token("ворога", PosClass::Noun, Some(GrammaticalCase::Accusative)),
        ];
        let result = validate_svo_cases(&triplet, &tokens);
        assert_eq!(result.actor, CaseValidation::Valid);
        assert_eq!(result.target, Some(CaseValidation::Valid));
        assert_eq!(result.overall, CaseValidation::Valid);
        assert!((result.confidence_multiplier - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_invalid_subject_dative() {
        // Subject in Dative — cannot be the actor.
        let triplet = make_triplet("Петру", "дав", Some("книгу"), true, 1.0);
        let tokens = vec![
            make_token("Петру", PosClass::Noun, Some(GrammaticalCase::Dative)),
            make_token("дав", PosClass::Verb, None),
            make_token("книгу", PosClass::Noun, Some(GrammaticalCase::Accusative)),
        ];
        let result = validate_svo_cases(&triplet, &tokens);
        assert_eq!(result.actor, CaseValidation::Invalid);
        assert_eq!(result.overall, CaseValidation::Invalid);
        // Multiplier: 0.3 (actor Invalid) * 1.0 (target Valid) = 0.3
        assert!((result.confidence_multiplier - 0.3).abs() < 1e-6);
    }

    #[test]
    fn test_negated_object_genitive_valid() {
        // "не вбив ворога" — Genitive of Negation is valid for negated verbs.
        let triplet = make_triplet("Петро", "вбив", Some("ворога"), false, 1.0);
        let tokens = vec![
            make_token("Петро", PosClass::Noun, Some(GrammaticalCase::Nominative)),
            make_token("вбив", PosClass::Verb, None),
            make_token("ворога", PosClass::Noun, Some(GrammaticalCase::Genitive)),
        ];
        let result = validate_svo_cases(&triplet, &tokens);
        assert_eq!(result.target, Some(CaseValidation::Valid));
        assert_eq!(result.overall, CaseValidation::Valid);
    }

    #[test]
    fn test_unknown_when_token_not_found() {
        let triplet = make_triplet("Хтось", "сказав", None, true, 1.0);
        let tokens = vec![make_token("сказав", PosClass::Verb, None)];
        let result = validate_svo_cases(&triplet, &tokens);
        assert_eq!(result.actor, CaseValidation::Unknown);
        // No target — overall stays Valid (vacuously true for absent roles).
        // But actor is Unknown, so overall is Unknown.
        assert_eq!(result.overall, CaseValidation::Unknown);
    }

    #[test]
    fn test_apply_validation_reduces_confidence() {
        let triplet = make_triplet("Петру", "дав", Some("книгу"), true, 0.9);
        let tokens = vec![
            make_token("Петру", PosClass::Noun, Some(GrammaticalCase::Dative)),
            make_token("дав", PosClass::Verb, None),
            make_token("книгу", PosClass::Noun, Some(GrammaticalCase::Accusative)),
        ];
        let result = validate_svo_cases(&triplet, &tokens);
        let adjusted = apply_case_validation(&triplet, &result);
        // Original 0.9 * 0.3 (Invalid actor) * 1.0 (Valid target) = 0.27
        assert!((adjusted.confidence - 0.27).abs() < 1e-6,
            "expected 0.27, got {}", adjusted.confidence);
    }

    #[test]
    fn test_confidence_multiplier_values() {
        assert_eq!(CaseValidation::Valid.confidence_multiplier(), 1.0);
        assert_eq!(CaseValidation::Invalid.confidence_multiplier(), 0.3);
        assert_eq!(CaseValidation::Unknown.confidence_multiplier(), 0.8);
    }

    #[test]
    fn test_instrument_valid_instrumental() {
        let mut t = make_triplet("Петро", "бив", Some("ворога"), true, 1.0);
        t.instrument = Some("ножем".to_string());
        let tokens = vec![
            make_token("Петро", PosClass::Noun, Some(GrammaticalCase::Nominative)),
            make_token("бив", PosClass::Verb, None),
            make_token("ворога", PosClass::Noun, Some(GrammaticalCase::Accusative)),
            make_token("ножем", PosClass::Noun, Some(GrammaticalCase::Instrumental)),
        ];
        let result = validate_svo_cases(&t, &tokens);
        assert_eq!(result.instrument, Some(CaseValidation::Valid));
        assert_eq!(result.overall, CaseValidation::Valid);
    }
}
