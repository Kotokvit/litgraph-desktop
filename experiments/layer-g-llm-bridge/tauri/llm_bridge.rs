//! POLER Layer G — Tauri IPC commands for the LLM Reasoning Bridge.
//!
//! This module exposes three Tauri commands that bridge the React frontend
//! to the canonical Layer G `LlmBridge` implemented in `litgraph-core`:
//!
//! 1. [`cmd_generate_llm_hypotheses`] — given a paradox, generate 4 canonical
//!    hypotheses (Flashback / DreamSequence / UnrecordedResurrection /
//!    DisguisedIdentity) via an LLM call.
//! 2. [`cmd_generate_resolution_text`] — given a chosen hypothesis, generate
//!    full chapter text (500-1500 words) that implements the hypothesis.
//! 3. [`cmd_validate_llm_response`] — validate the LLM-proposed text against
//!    the deterministic Layer E ParadoxDetector (no LLM call — pure symbolic
//!    check). Returns Accept / Reject / Retry.
//!
//! ## Naming
//!
//! The `cmd_` prefix matches the Layer F commands in [`super::poler`].
//!
//! ## Determinism
//!
//! Only [`cmd_validate_llm_response`] is deterministic. The other two call
//! an LLM and are non-deterministic — same input may produce different output
//! across runs. Tests must use a mock provider.

use serde::{Deserialize, Serialize};

use crate::ai::AiProvider;
use crate::poler::{LlmBridge, Paradox, ParadoxKind};
use crate::poler::Hypothesis as CoreHypothesis;
use crate::poler::HypothesisKind as CoreHypothesisKind;
use crate::poler::ValidationOutcome as CoreValidationOutcome;

// ============================================================================
// DTOs (camelCase, mirror litgraph-core types)
// ============================================================================

/// Frontend-facing hypothesis kind (camelCase JSON).
///
/// Mirrors [`CoreHypothesisKind`] but stringified for easier TS consumption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum HypothesisKindDto {
    Flashback,
    DreamSequence,
    UnrecordedResurrection,
    DisguisedIdentity,
}

impl From<CoreHypothesisKind> for HypothesisKindDto {
    fn from(k: CoreHypothesisKind) -> Self {
        match k {
            CoreHypothesisKind::Flashback => Self::Flashback,
            CoreHypothesisKind::DreamSequence => Self::DreamSequence,
            CoreHypothesisKind::UnrecordedResurrection => Self::UnrecordedResurrection,
            CoreHypothesisKind::DisguisedIdentity => Self::DisguisedIdentity,
        }
    }
}

impl From<HypothesisKindDto> for CoreHypothesisKind {
    fn from(k: HypothesisKindDto) -> Self {
        match k {
            HypothesisKindDto::Flashback => Self::Flashback,
            HypothesisKindDto::DreamSequence => Self::DreamSequence,
            HypothesisKindDto::UnrecordedResurrection => Self::UnrecordedResurrection,
            HypothesisKindDto::DisguisedIdentity => Self::DisguisedIdentity,
        }
    }
}

/// Frontend-facing hypothesis (camelCase JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HypothesisDto {
    pub id: String,
    pub paradox_id: String,
    pub kind: HypothesisKindDto,
    pub summary: String,
    pub proposed_text: Option<String>,
    pub confidence: f64,
    pub rationale: String,
}

impl From<CoreHypothesis> for HypothesisDto {
    fn from(h: CoreHypothesis) -> Self {
        Self {
            id: h.id,
            paradox_id: h.paradox_id,
            kind: h.kind.into(),
            summary: h.summary,
            proposed_text: h.proposed_text,
            confidence: h.confidence,
            rationale: h.rationale,
        }
    }
}

impl From<HypothesisDto> for CoreHypothesis {
    fn from(h: HypothesisDto) -> Self {
        Self {
            id: h.id,
            paradox_id: h.paradox_id,
            kind: h.kind.into(),
            summary: h.summary,
            proposed_text: h.proposed_text,
            confidence: h.confidence,
            rationale: h.rationale,
        }
    }
}

/// Frontend-facing Paradox DTO (re-exported from Layer F poler.rs for
/// convenience — Layer G commands take ParadoxDto as input).
///
/// Mirrors [`Paradox`] but with `camelCase` field names and a stringified
/// `kind`. Same shape as [`crate::commands::poler::ParadoxDto`] — kept
/// duplicated here so callers don't need to import from `commands::poler`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParadoxDto {
    pub id: String,
    pub kind: String,
    pub character: String,
    pub chapter_idx: usize,
    pub origin_chapter_idx: usize,
    pub explanation: String,
    pub evidence_text: Vec<String>,
}

impl From<ParadoxDto> for Paradox {
    fn from(p: ParadoxDto) -> Self {
        let kind = match p.kind.as_str() {
            "dead_speaking" => ParadoxKind::DeadSpeaking,
            "spatial_teleportation" => ParadoxKind::SpatialTeleportation,
            other => panic!("Unknown paradox kind string: {}", other),
        };
        Paradox {
            id: p.id,
            kind,
            character: p.character,
            chapter_idx: p.chapter_idx,
            origin_chapter_idx: p.origin_chapter_idx,
            explanation: p.explanation,
            evidence_text: p.evidence_text,
        }
    }
}

/// Frontend-facing validation outcome (tagged enum, camelCase).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValidationOutcomeDto {
    Accept {
        violations: Vec<String>,
        paradoxes: Vec<String>,
    },
    Reject {
        violations: Vec<String>,
        feedback_prompt: String,
    },
    Retry {
        reason: String,
    },
}

impl From<CoreValidationOutcome> for ValidationOutcomeDto {
    fn from(o: CoreValidationOutcome) -> Self {
        match o {
            CoreValidationOutcome::Accept { violations, paradoxes } => Self::Accept {
                violations,
                paradoxes,
            },
            CoreValidationOutcome::Reject { violations, feedback_prompt } => Self::Reject {
                violations,
                feedback_prompt,
            },
            CoreValidationOutcome::Retry { reason } => Self::Retry { reason },
        }
    }
}

// ============================================================================
// Command 1: cmd_generate_llm_hypotheses
// ============================================================================

/// Generate 4 canonical LLM hypotheses for a single paradox.
///
/// ## Parameters
/// - `paradox` — the paradox to resolve (id, kind, character, evidence, etc.).
/// - `provider` — the AI provider config (Ollama / OpenAI-compat / Z.ai).
///
/// ## Returns
/// `Vec<HypothesisDto>` — exactly 4 hypotheses, one of each
/// [`HypothesisKindDto`] variant.
///
/// ## Errors
/// Returns `Err(String)` if:
/// - The LLM call fails (network error, auth error)
/// - The response can't be parsed as JSON
/// - The response doesn't contain all 4 canonical kinds
#[tauri::command]
pub async fn cmd_generate_llm_hypotheses(
    paradox: ParadoxDto,
    provider: AiProvider,
) -> Result<Vec<HypothesisDto>, String> {
    let paradox_core: Paradox = paradox.into();
    let bridge = LlmBridge::new(provider);
    let hypotheses = bridge.generate_hypotheses(&paradox_core).await?;
    Ok(hypotheses.into_iter().map(HypothesisDto::from).collect())
}

// ============================================================================
// Command 2: cmd_generate_resolution_text
// ============================================================================

/// Generate full resolution text for a chosen hypothesis.
///
/// ## Parameters
/// - `hypothesis` — the chosen hypothesis (kind + summary + rationale).
/// - `provider` — the AI provider config.
///
/// ## Returns
/// A new [`HypothesisDto`] with `proposed_text` populated (all other fields
/// copied from the input).
#[tauri::command]
pub async fn cmd_generate_resolution_text(
    hypothesis: HypothesisDto,
    provider: AiProvider,
) -> Result<HypothesisDto, String> {
    let hyp_core: CoreHypothesis = hypothesis.into();
    let bridge = LlmBridge::new(provider);
    let resolved = bridge.generate_resolution_text(&hyp_core).await?;
    Ok(HypothesisDto::from(resolved))
}

// ============================================================================
// Command 3: cmd_validate_llm_response
// ============================================================================

/// Validate LLM-proposed text against the deterministic Layer E
/// ParadoxDetector.
///
/// ## Parameters
/// - `proposed_text` — the LLM-generated chapter text.
/// - `original_paradoxes` — the paradoxes that the LLM was asked to resolve
///   (used to determine whether they've been addressed).
///
/// ## Returns
/// [`ValidationOutcomeDto`] — `Accept` if no new paradoxes and originals
/// resolved, `Reject` with feedback if new paradoxes introduced or originals
/// persist, `Retry` if the text is empty.
#[tauri::command]
pub async fn cmd_validate_llm_response(
    proposed_text: String,
    original_paradoxes: Vec<ParadoxDto>,
) -> Result<ValidationOutcomeDto, String> {
    let original_paradoxes_core: Vec<Paradox> =
        original_paradoxes.into_iter().map(Paradox::from).collect();
    // Use a dummy provider for validation — validate() doesn't make LLM calls.
    let bridge = LlmBridge::new(AiProvider::Ollama {
        url: String::new(),
        model: String::new(),
    });
    let outcome = bridge.validate(&proposed_text, &original_paradoxes_core);
    Ok(ValidationOutcomeDto::from(outcome))
}

// ============================================================================
// Unit tests — DTO conversion logic
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypothesis_dto_round_trip() {
        let core = CoreHypothesis {
            id: "hg-test-001".to_string(),
            paradox_id: "px-test-001".to_string(),
            kind: CoreHypothesisKind::Flashback,
            summary: "Спогад про Петра".to_string(),
            proposed_text: Some("Текст глави...".to_string()),
            confidence: 0.85,
            rationale: "Тому що...".to_string(),
        };
        let dto: HypothesisDto = core.clone().into();
        let back: CoreHypothesis = dto.into();
        assert_eq!(back.id, core.id);
        assert_eq!(back.paradox_id, core.paradox_id);
        assert_eq!(back.kind, core.kind);
        assert_eq!(back.summary, core.summary);
        assert_eq!(back.proposed_text, core.proposed_text);
        assert!((back.confidence - core.confidence).abs() < 1e-9);
        assert_eq!(back.rationale, core.rationale);
    }

    #[test]
    fn test_hypothesis_kind_dto_all_variants() {
        for kind in CoreHypothesisKind::all() {
            let dto: HypothesisKindDto = kind.clone().into();
            let back: CoreHypothesisKind = dto.into();
            assert_eq!(&back, kind);
        }
    }

    #[test]
    fn test_paradox_dto_conversion() {
        let dto = ParadoxDto {
            id: "px-test-001".to_string(),
            kind: "dead_speaking".to_string(),
            character: "Петро".to_string(),
            chapter_idx: 5,
            origin_chapter_idx: 2,
            explanation: "Test explanation".to_string(),
            evidence_text: vec!["…snip…".to_string()],
        };
        let core: Paradox = dto.into();
        assert_eq!(core.id, "px-test-001");
        assert_eq!(core.kind, ParadoxKind::DeadSpeaking);
        assert_eq!(core.character, "Петро");
        assert_eq!(core.chapter_idx, 5);
        assert_eq!(core.origin_chapter_idx, 2);
        assert_eq!(core.evidence_text.len(), 1);
    }

    #[test]
    fn test_validation_outcome_dto_accept() {
        let core = CoreValidationOutcome::Accept {
            violations: vec![],
            paradoxes: vec![],
        };
        let dto: ValidationOutcomeDto = core.into();
        match dto {
            ValidationOutcomeDto::Accept { violations, paradoxes } => {
                assert!(violations.is_empty());
                assert!(paradoxes.is_empty());
            }
            _ => panic!("Expected Accept"),
        }
    }

    #[test]
    fn test_validation_outcome_dto_reject() {
        let core = CoreValidationOutcome::Reject {
            violations: vec!["v1".to_string()],
            feedback_prompt: "Rewrite...".to_string(),
        };
        let dto: ValidationOutcomeDto = core.into();
        match dto {
            ValidationOutcomeDto::Reject { violations, feedback_prompt } => {
                assert_eq!(violations.len(), 1);
                assert!(feedback_prompt.contains("Rewrite"));
            }
            _ => panic!("Expected Reject"),
        }
    }

    #[test]
    fn test_validation_outcome_dto_retry() {
        let core = CoreValidationOutcome::Retry {
            reason: "Empty text".to_string(),
        };
        let dto: ValidationOutcomeDto = core.into();
        match dto {
            ValidationOutcomeDto::Retry { reason } => {
                assert_eq!(reason, "Empty text");
            }
            _ => panic!("Expected Retry"),
        }
    }
}
