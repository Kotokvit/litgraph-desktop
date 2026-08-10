//! POLER Layer F — Tauri IPC commands for React frontend.
//!
//! This module exposes three Tauri commands that bridge the React frontend
//! to the canonical POLER v7.5-LEM engine (Layers A–E) implemented in
//! `litgraph-core`:
//!
//! 1. [`cmd_compute_epsilon_climax`] — compute ε_climax for a single chapter
//!    using the real Layer E [`NarrativeGraph`] (no placeholder Ω_conf).
//! 2. [`cmd_extract_svo`] — extract SVO triplets (Subject-Verb-Object) from
//!    text using the Rust-native [`SvoParser`] (Layer C). This is the
//!    Rust-native replacement for the legacy Python-based `extract_svo`
//!    in [`crate::commands::ner`].
//! 3. [`cmd_detect_paradoxes`] — scan a multi-chapter manuscript for temporal
//!    paradoxes (Dead-Speaking, …) using Layer E [`ParadoxDetector`].
//!
//! ## Why these three commands?
//!
//! The frontend needs three core POLER capabilities:
//! - **Per-chapter climax scoring** for the ε-heatmap (top of the editor).
//! - **SVO triplet inspection** for the "Action Inspector" panel (so authors
//!   can see exactly which subject→verb→object triplets POLER detected).
//! - **Paradox alerts** for the "Continuity Checker" panel (warnings about
//!   dead characters speaking, teleportation, etc.).
//!
//! ## Naming
//!
//! The `cmd_` prefix distinguishes these commands from the legacy
//! Python-backed ones in [`crate::commands::ner`] (which uses `extract_svo`).
//! Both can coexist: the frontend chooses which one to invoke based on
//! whether it wants the Rust-native symbolic engine or the Python spaCy one.
//!
//! ## Determinism
//!
//! All three commands are **pure** and **deterministic**: same input ⇒ same
//! output. No I/O, no global mutable state, no LLM calls. This is the
//! Symbolic AI principle of POLER — the frontend can call these in tight
//! loops without side effects.

use serde::{Deserialize, Serialize};

use crate::poler::{
    compute_epsilon_climax_with_analyzer, detect_chapters, detect_characters,
    ConflictAnalyzer, ConflictReport, EpsilonResult, ManuscriptAnalysis, NarrativeGraph,
    ParadoxDetector, Paradox, ParsedChapter, ParsedCharacter, SvoParser, SvoTriplet,
};

// ============================================================================
// DTOs (frontend-facing, camelCase JSON)
// ============================================================================

/// Frontend-facing ε_climax result.
///
/// Mirrors [`EpsilonResult`] but with `camelCase` field names so the React
/// frontend can consume it directly without case-conversion glue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpsilonClimaxDto {
    /// Raw ε_climax value (unnormalized).
    pub epsilon: f64,
    /// Normalized to 0–100 scale (relative to max chapter). Always 0 for
    /// single-chapter calls — call [`cmd_compute_epsilon_climax`] per chapter
    /// then normalize on the frontend, or compute ε for all chapters at once
    /// via `compute_epsilon_climax_batch` (future command).
    pub normalized: f64,
    pub word_count: usize,
    pub unique_words: usize,
    pub emotion_count: usize,
    pub kw_count: usize,
    pub canon_count: usize,
    pub action_count: usize,
    /// Adaptive noise threshold θ_rel = 3.5/κ.
    pub theta_rel: f64,
    /// True if ε < θ_rel (chapter is "noise").
    pub is_noise: bool,
    /// True if ε ≥ CLIMAX_THRESHOLD (chapter is a climax).
    pub is_climax: bool,
    pub formula_variant: String,
    /// Conflict magnitude Ω_conf from Layer E (Frobenius norm of A_POS).
    /// `0.0` if no characters interact in this chapter.
    pub omega_conf: f64,
    /// Spectral radius ρ(A_POS) — largest eigenvalue of character adjacency.
    pub spectral_radius: f64,
    /// Number of character nodes in the conflict graph for this chapter.
    pub node_count: usize,
    /// Number of directed edges in the conflict graph for this chapter.
    pub edge_count: usize,
}

impl EpsilonClimaxDto {
    /// Combine an [`EpsilonResult`] (from Layer D) with a [`ConflictReport`]
    /// (from Layer E) into a single frontend-facing DTO.
    ///
    /// This is the only place where the two layers' outputs are merged for
    /// the frontend, so the mapping is centralized here.
    pub fn from_layers(eps: EpsilonResult, report: &ConflictReport) -> Self {
        Self {
            epsilon: eps.epsilon,
            normalized: eps.normalized,
            word_count: eps.word_count,
            unique_words: eps.unique_words,
            emotion_count: eps.emotion_count,
            kw_count: eps.kw_count,
            canon_count: eps.canon_count,
            action_count: eps.action_count,
            theta_rel: eps.theta_rel,
            is_noise: eps.is_noise,
            is_climax: eps.is_climax,
            formula_variant: eps.formula_variant.to_string(),
            omega_conf: report.omega_conf,
            spectral_radius: report.spectral_radius,
            node_count: report.node_count,
            edge_count: report.edge_count,
        }
    }
}

/// Frontend-facing SVO triplet (camelCase JSON).
///
/// Mirrors [`SvoTriplet`] but with `camelCase` field names.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SvoTripletDto {
    pub actor: String,
    pub verb: String,
    pub target: Option<String>,
    pub instrument: Option<String>,
    pub location: Option<String>,
    /// `true` for affirmative, `false` for negated ("не вбив").
    pub polarity: bool,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f64,
}

impl From<SvoTriplet> for SvoTripletDto {
    fn from(t: SvoTriplet) -> Self {
        Self {
            actor: t.actor,
            verb: t.verb,
            target: t.target,
            instrument: t.instrument,
            location: t.location,
            polarity: t.polarity,
            confidence: t.confidence,
        }
    }
}

/// Frontend-facing paradox (camelCase JSON).
///
/// Mirrors [`Paradox`] but with `camelCase` field names and a stringified
/// `kind` for easier consumption in TypeScript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParadoxDto {
    /// `"dead_speaking"` or `"spatial_teleportation"`.
    pub kind: String,
    pub character: String,
    /// Index of the chapter where the paradox manifests.
    pub chapter_idx: usize,
    /// Index of the chapter where the originating event was recorded.
    pub origin_chapter_idx: usize,
    /// Human-readable explanation (Ukrainian/English).
    pub explanation: String,
}

impl From<Paradox> for ParadoxDto {
    fn from(p: Paradox) -> Self {
        let kind = match p.kind {
            crate::poler::ParadoxKind::DeadSpeaking => "dead_speaking",
            crate::poler::ParadoxKind::SpatialTeleportation => "spatial_teleportation",
        };
        Self {
            kind: kind.to_string(),
            character: p.character,
            chapter_idx: p.chapter_idx,
            origin_chapter_idx: p.origin_chapter_idx,
            explanation: p.explanation,
        }
    }
}

/// Frontend-facing per-chapter manuscript breakdown used by
/// [`cmd_detect_paradoxes`].
///
/// Returned alongside the paradox list so the UI can render the chapter list
/// with character counts and triplet counts for transparency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterBreakdownDto {
    pub chapter_idx: usize,
    pub title: String,
    /// Number of characters detected in this chapter (Layer B).
    pub character_count: usize,
    /// Number of SVO triplets extracted from this chapter (Layer C).
    pub triplet_count: usize,
    /// Character names detected in this chapter.
    pub characters: Vec<String>,
}

/// Result bundle for [`cmd_detect_paradoxes`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParadoxReportDto {
    /// All detected paradoxes, ordered by chapter_idx ASC.
    pub paradoxes: Vec<ParadoxDto>,
    /// Per-chapter breakdown (characters + triplets).
    pub chapters: Vec<ChapterBreakdownDto>,
    /// Total character count across all chapters.
    pub total_characters: usize,
    /// Total SVO triplets across all chapters.
    pub total_triplets: usize,
}

// ============================================================================
// Command 1: cmd_compute_epsilon_climax
// ============================================================================

/// Compute ε_climax for a single chapter using the canonical POLER v7.5-LEM
/// formula with real Ω_conf from Layer E [`NarrativeGraph`].
///
/// ## Formula
///
/// ```text
///                    κ · I_loc · d̄² + γ_emo · E + λ_conf · Ω_conf
/// ε_climax  =  ──────────────────────────────────────────────────────
///                                ln(e + |U|)
/// ```
///
/// - `I_loc = 1 + canon_count_in_chapter` (canonical anchors intensity)
/// - `d̄²` = mean squared rarity over unique words
/// - `Ω_conf` = ‖A_POS‖_F (Frobenius norm of character adjacency matrix,
///   computed by `NarrativeGraph` from SVO triplets)
/// - `γ_emo = 1.0`, `λ_conf = 12.5`
///
/// ## Parameters
///
/// - `chapter_text` — full chapter text (Ukrainian/Russian).
/// - `keyword` — optional keyword for kw_count (currently unused in climax
///   formula but reserved for future use). Pass `None` if not needed.
/// - `kappa` — sector-adaptive coefficient (default `1.0`). Controls
///   θ_rel = 3.5/κ. Higher κ → lower noise threshold.
///
/// ## Returns
///
/// [`EpsilonClimaxDto`] with both the ε value and Layer E conflict metrics
/// (Ω_conf, ρ, node_count, edge_count).
///
/// ## Errors
///
/// Returns `Err(String)` only if `chapter_text` is empty or `kappa ≤ 0`.
///
/// ## Example (TypeScript / React)
///
/// ```typescript
/// const result = await invoke<EpsilonClimaxDto>("cmd_compute_epsilon_climax", {
///   chapterText: "Петро вбив ворога у бою.",
///   keyword: null,
///   kappa: 1.0,
/// });
/// console.log(result.epsilon, result.omegaConf, result.spectralRadius);
/// ```
#[tauri::command]
pub async fn cmd_compute_epsilon_climax(
    chapter_text: String,
    keyword: Option<String>,
    kappa: Option<f64>,
) -> Result<EpsilonClimaxDto, String> {
    if chapter_text.trim().is_empty() {
        return Err("Порожній текст глави — не можна обчислити ε_climax".to_string());
    }
    let k = kappa.unwrap_or(1.0);
    if k <= 0.0 {
        return Err(format!("kappa має бути > 0, отримано {}", k));
    }

    // Layer E: use the real NarrativeGraph analyzer (no placeholder).
    let analyzer = NarrativeGraph::new();

    // Layer D: compute ε_climax with the injected analyzer.
    // This internally calls:
    //   - characters::detect (Layer B)
    //   - SvoParser::parse_text (Layer C)
    //   - analyzer.analyze_chapter (Layer E) → ConflictReport
    let kw_ref: Option<&str> = keyword.as_deref();
    let eps = compute_epsilon_climax_with_analyzer(&chapter_text, kw_ref, k, &analyzer);

    // Re-run the analyzer's analyze_chapter to get the ConflictReport for the
    // DTO. This is cheap (no I/O, pure symbolic) and keeps the contract clean:
    // the ε formula doesn't expose the report it used internally.
    let detected_chars = detect_characters(&chapter_text);
    let triplets = SvoParser::new().parse_text(&chapter_text);
    let report: ConflictReport =
        analyzer.analyze_chapter(&chapter_text, detected_chars.clone(), triplets.clone());

    Ok(EpsilonClimaxDto::from_layers(eps, &report))
}

// ============================================================================
// Command 2: cmd_extract_svo
// ============================================================================

/// Extract SVO triplets (Subject-Verb-Object) from text using the
/// Rust-native [`SvoParser`] (Layer C, POLER v7.5-LEM).
///
/// This is the **Rust-native** replacement for the legacy Python-based
/// `extract_svo` in [`crate::commands::ner`]. It does NOT require Python,
/// spaCy, or pymorphy3 — it uses the in-process lemmatizer, POS-tagger, and
/// UD_Ukrainian-IU dependency templates bundled with `litgraph-core`.
///
/// ## Parameters
///
/// - `text` — input text (single sentence, paragraph, or full chapter).
///
/// ## Returns
///
/// `Vec<SvoTripletDto>` — one entry per detected triplet. Each triplet has:
/// - `actor` (Subject / Nominative)
/// - `verb` (canonical lemma, e.g. "вбити" not "вбив")
/// - `target` (Direct Object / Accusative or Genitive of Negation; `null` if intransitive)
/// - `instrument` (Instrument case; `null` if absent)
/// - `location` (Locative with preposition; `null` if absent)
/// - `polarity` (`false` if negated with "не"/"ні")
/// - `confidence` ∈ `[0.0, 1.0]`
///
/// ## Errors
///
/// Returns `Err(String)` only if `text` is empty.
///
/// ## Example (TypeScript / React)
///
/// ```typescript
/// const triplets = await invoke<SvoTripletDto[]>("cmd_extract_svo", {
///   text: "Петро вбив ворога ножем у лісі.",
/// });
/// // → [{ actor: "Петро", verb: "вбити", target: "ворога",
/// //     instrument: "ножем", location: "у лісі", polarity: true, confidence: 0.95 }]
/// ```
#[tauri::command]
pub async fn cmd_extract_svo(text: String) -> Result<Vec<SvoTripletDto>, String> {
    if text.trim().is_empty() {
        return Err("Порожній текст — SVO-парсер не має вхідних даних".to_string());
    }
    let parser = SvoParser::new();
    let triplets: Vec<SvoTriplet> = parser.parse_text(&text);
    let dtos: Vec<SvoTripletDto> = triplets.into_iter().map(SvoTripletDto::from).collect();
    Ok(dtos)
}

// ============================================================================
// Command 3: cmd_detect_paradoxes
// ============================================================================

/// Detect temporal paradoxes in a multi-chapter manuscript.
///
/// Splits `text` into chapters via [`detect_chapters`] (Layer A), runs
/// [`detect_characters`] (Layer B) and [`SvoParser`] (Layer C) on each
/// chapter, then feeds the full [`ManuscriptAnalysis`] to
/// [`ParadoxDetector::detect`] (Layer E).
///
/// ## Currently detected paradox types
///
/// - **Dead-Speaking**: a character marked as deceased in chapter N (via
///   death markers "помер", "загинув", "вбитий", …) acts or speaks in any
///   later chapter M > N.
/// - **Spatial-Teleportation**: *(placeholder — not yet implemented in
///   Layer E; requires Layer F location normalization)*.
///
/// ## Parameters
///
/// - `text` — full manuscript text (markdown with `^Глава N` headers or
///   plain text — the chapter detector handles both).
///
/// ## Returns
///
/// [`ParadoxReportDto`] with:
/// - `paradoxes` — list of detected paradoxes (ordered by chapter_idx).
/// - `chapters` — per-chapter breakdown (character_count, triplet_count, character names).
/// - `total_characters` — distinct characters across the whole manuscript.
/// - `total_triplets` — total SVO triplets across the whole manuscript.
///
/// ## Errors
///
/// Returns `Err(String)` only if `text` is empty.
///
/// ## Example (TypeScript / React)
///
/// ```typescript
/// const report = await invoke<ParadoxReportDto>("cmd_detect_paradoxes", {
///   text: fullManuscriptMarkdown,
/// });
/// if (report.paradoxes.length > 0) {
///   showWarning(`Знайдено ${report.paradoxes.length} парадоксів`);
/// }
/// ```
#[tauri::command]
pub async fn cmd_detect_paradoxes(text: String) -> Result<ParadoxReportDto, String> {
    if text.trim().is_empty() {
        return Err("Порожній текст — парадокси не виявляються".to_string());
    }

    // Layer A: split into chapters.
    let (parsed_chapters, _prologue): (Vec<ParsedChapter>, String) = detect_chapters(&text);

    // If no chapter headers found, treat the whole text as one chapter.
    let chapter_texts: Vec<&str> = if parsed_chapters.is_empty() {
        vec![text.as_str()]
    } else {
        parsed_chapters.iter().map(|c| c.full_text.as_str()).collect()
    };

    // Layer B + C: per-chapter characters + SVO triplets.
    let mut characters_per_chapter: Vec<Vec<ParsedCharacter>> = Vec::with_capacity(chapter_texts.len());
    let mut triplets_per_chapter: Vec<Vec<SvoTriplet>> = Vec::with_capacity(chapter_texts.len());
    let mut chapter_breakdowns: Vec<ChapterBreakdownDto> = Vec::with_capacity(chapter_texts.len());
    let parser = SvoParser::new();

    let mut total_triplets: usize = 0;
    for (idx, ch_text) in chapter_texts.iter().enumerate() {
        let chars = detect_characters(ch_text);
        let triplets = parser.parse_text(ch_text);
        total_triplets += triplets.len();

        let char_names: Vec<String> = chars
            .iter()
            .map(|c| c.name.clone())
            .collect();

        chapter_breakdowns.push(ChapterBreakdownDto {
            chapter_idx: idx,
            title: parsed_chapters
                .get(idx)
                .map(|c| c.title.clone())
                .unwrap_or_else(|| format!("Chapter {}", idx + 1)),
            character_count: chars.len(),
            triplet_count: triplets.len(),
            characters: char_names,
        });

        characters_per_chapter.push(chars);
        triplets_per_chapter.push(triplets);
    }

    // Distinct character count across the whole manuscript.
    let total_characters: usize = {
        use std::collections::HashSet;
        let mut seen: HashSet<String> = HashSet::new();
        for chars in &characters_per_chapter {
            for c in chars {
                seen.insert(c.name.to_lowercase());
            }
        }
        seen.len()
    };

    // Layer E: run ParadoxDetector.
    let manuscript = ManuscriptAnalysis {
        chapters: chapter_texts.into_iter().collect::<Vec<&str>>(),
        characters_per_chapter,
        triplets_per_chapter,
    };
    let mut detector = ParadoxDetector::new();
    let paradoxes: Vec<Paradox> = detector.detect(&manuscript);

    let paradox_dtos: Vec<ParadoxDto> = paradoxes.into_iter().map(ParadoxDto::from).collect();

    Ok(ParadoxReportDto {
        paradoxes: paradox_dtos,
        chapters: chapter_breakdowns,
        total_characters,
        total_triplets,
    })
}

// ============================================================================
// Unit tests — DTO conversion logic
// ============================================================================
//
// These tests cover the pure `From` / `from_layers` conversions, which is the
// only logic in this module that isn't a thin wrapper around litgraph-core.
// The Tauri command wrappers themselves are integration-tested via the
// frontend (see `src/components/PolerPanel.test.tsx` — future work).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poler::{ParadoxKind, SvoTriplet};

    // ---------- SvoTripletDto conversion ----------

    #[test]
    fn test_svo_triplet_dto_preserves_all_fields() {
        let t = SvoTriplet {
            actor: "Петро".to_string(),
            verb: "вбити".to_string(),
            target: Some("ворога".to_string()),
            instrument: Some("ножем".to_string()),
            location: Some("у лісі".to_string()),
            polarity: true,
            confidence: 0.95,
        };
        let dto: SvoTripletDto = t.into();
        assert_eq!(dto.actor, "Петро");
        assert_eq!(dto.verb, "вбити");
        assert_eq!(dto.target.as_deref(), Some("ворога"));
        assert_eq!(dto.instrument.as_deref(), Some("ножем"));
        assert_eq!(dto.location.as_deref(), Some("у лісі"));
        assert!(dto.polarity);
        assert!((dto.confidence - 0.95).abs() < 1e-9);
    }

    #[test]
    fn test_svo_triplet_dto_handles_intransitive() {
        let t = SvoTriplet {
            actor: "Марта".to_string(),
            verb: "йти".to_string(),
            target: None,
            instrument: None,
            location: None,
            polarity: true,
            confidence: 0.7,
        };
        let dto: SvoTripletDto = t.into();
        assert!(dto.target.is_none());
        assert!(dto.instrument.is_none());
        assert!(dto.location.is_none());
    }

    #[test]
    fn test_svo_triplet_dto_negated_polarity() {
        let t = SvoTriplet {
            actor: "Петро".to_string(),
            verb: "вбити".to_string(),
            target: Some("ворога".to_string()),
            instrument: None,
            location: None,
            polarity: false,
            confidence: 0.9,
        };
        let dto: SvoTripletDto = t.into();
        assert!(!dto.polarity, "Negated verb must have polarity=false");
    }

    // ---------- ParadoxDto conversion ----------

    #[test]
    fn test_paradox_dto_dead_speaking() {
        let p = Paradox {
            kind: ParadoxKind::DeadSpeaking,
            character: "Петро".to_string(),
            chapter_idx: 5,
            origin_chapter_idx: 2,
            explanation: "Character 'Петро' speaks in chapter 5 but died in chapter 2".to_string(),
        };
        let dto: ParadoxDto = p.into();
        assert_eq!(dto.kind, "dead_speaking");
        assert_eq!(dto.character, "Петро");
        assert_eq!(dto.chapter_idx, 5);
        assert_eq!(dto.origin_chapter_idx, 2);
        assert!(dto.explanation.contains("Петро"));
    }

    #[test]
    fn test_paradox_dto_spatial_teleportation() {
        let p = Paradox {
            kind: ParadoxKind::SpatialTeleportation,
            character: "Марта".to_string(),
            chapter_idx: 3,
            origin_chapter_idx: 1,
            explanation: "Teleportation detected".to_string(),
        };
        let dto: ParadoxDto = p.into();
        assert_eq!(dto.kind, "spatial_teleportation");
        assert_eq!(dto.character, "Марта");
    }

    // ---------- EpsilonClimaxDto merging ----------

    #[test]
    fn test_epsilon_climax_dto_merges_layers_correctly() {
        // Construct a synthetic EpsilonResult and ConflictReport, verify the
        // DTO carries both layers' fields without loss.
        let eps = EpsilonResult {
            epsilon: 7.42,
            normalized: 85.3,
            word_count: 120,
            unique_words: 65,
            emotion_count: 3,
            kw_count: 2,
            canon_count: 1,
            action_count: 4,
            theta_rel: 3.5,
            is_noise: false,
            is_climax: true,
            formula_variant: "climax",
        };
        let report = ConflictReport {
            omega_conf: 2.828, // √8 ≈ 2.828
            spectral_radius: 2.0,
            node_count: 3,
            edge_count: 4,
            paradoxes: vec![],
        };

        let dto = EpsilonClimaxDto::from_layers(eps, &report);

        // Layer D fields
        assert!((dto.epsilon - 7.42).abs() < 1e-9);
        assert!((dto.normalized - 85.3).abs() < 1e-9);
        assert_eq!(dto.word_count, 120);
        assert_eq!(dto.unique_words, 65);
        assert_eq!(dto.emotion_count, 3);
        assert_eq!(dto.kw_count, 2);
        assert_eq!(dto.canon_count, 1);
        assert_eq!(dto.action_count, 4);
        assert!((dto.theta_rel - 3.5).abs() < 1e-9);
        assert!(!dto.is_noise);
        assert!(dto.is_climax);
        assert_eq!(dto.formula_variant, "climax");

        // Layer E fields
        assert!((dto.omega_conf - 2.828).abs() < 1e-3);
        assert!((dto.spectral_radius - 2.0).abs() < 1e-9);
        assert_eq!(dto.node_count, 3);
        assert_eq!(dto.edge_count, 4);
    }

    // ---------- End-to-end smoke tests via the public poler API ----------
    //
    // These don't go through Tauri's `invoke_handler!` (which requires the
    // Tauri runtime), but they DO exercise the same code path by calling
    // the underlying functions directly. This catches integration bugs
    // between Layers D + E that the per-layer tests in litgraph-core might
    // miss.

    #[test]
    fn test_cmd_epsilon_climax_smoke_via_poler_api() {
        // Same code path as cmd_compute_epsilon_climax, just without the
        // Tauri command wrapper.
        let analyzer = NarrativeGraph::new();
        let test_text = "Петро сказав прощання і вбив ворога у бою.";
        let eps = compute_epsilon_climax_with_analyzer(
            test_text,
            None,
            1.0,
            &analyzer,
        );
        assert!(eps.epsilon > 0.0, "ε_climax must be positive for conflict text");
        assert_eq!(eps.formula_variant, "climax");

        // Re-derive the ConflictReport for the DTO (mirrors the command body).
        let chars = detect_characters(test_text);
        let triplets = SvoParser::new().parse_text(test_text);
        let report = analyzer.analyze_chapter(test_text, chars, triplets);

        let dto = EpsilonClimaxDto::from_layers(eps, &report);
        assert!(dto.omega_conf >= 0.0);
        assert!(dto.node_count > 0, "node_count must be > 0 for parsed character graph");
    }

    #[test]
    fn test_cmd_extract_svo_smoke_via_poler_api() {
        // Same code path as cmd_extract_svo.
        let parser = SvoParser::new();
        let triplets = parser.parse_text("Петро вбив ворога.");
        assert!(!triplets.is_empty(), "SvoParser must extract at least one triplet");

        let dtos: Vec<SvoTripletDto> = triplets.into_iter().map(SvoTripletDto::from).collect();
        for dto in &dtos {
            assert!(!dto.actor.is_empty(), "Actor must be non-empty");
            assert!(!dto.verb.is_empty(), "Verb must be non-empty");
            assert!(dto.confidence >= 0.0 && dto.confidence <= 1.0, "Confidence out of [0,1]");
        }
    }

    #[test]
    fn test_cmd_detect_paradoxes_smoke_via_poler_api() {
        // Same code path as cmd_detect_paradoxes — a 2-chapter manuscript
        // where Петро dies in ch.1 and speaks in ch.2 (a Dead-Speaking paradox).
        let manuscript_text = "Глава 1\nПетро сказав прощання і помер у бою.\n\nГлава 2\nПетро сказав останнє слово.";
        let (chapters, _prologue) = detect_chapters(manuscript_text);
        // The detector requires at least 2 chapters for a paradox.
        assert!(chapters.len() >= 2, "Chapter detector must split into ≥2 chapters");

        let chapter_texts: Vec<&str> = chapters.iter().map(|c| c.full_text.as_str()).collect();
        let mut chars_per_ch: Vec<Vec<ParsedCharacter>> = Vec::new();
        let mut triplets_per_ch: Vec<Vec<SvoTriplet>> = Vec::new();
        let parser = SvoParser::new();
        for ct in &chapter_texts {
            chars_per_ch.push(detect_characters(ct));
            triplets_per_ch.push(parser.parse_text(ct));
        }

        let manuscript = ManuscriptAnalysis {
            chapters: chapter_texts.into_iter().collect::<Vec<&str>>(),
            characters_per_chapter: chars_per_ch,
            triplets_per_chapter: triplets_per_ch,
        };
        let mut detector = ParadoxDetector::new();
        let paradoxes = detector.detect(&manuscript);
        assert!(
            paradoxes.iter().any(|p| p.kind == ParadoxKind::DeadSpeaking),
            "Must detect Dead-Speaking paradox for Петро"
        );
    }
}
