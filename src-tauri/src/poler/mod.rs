//! POLER Layer E — Tauri-side bridge to litgraph-core's reasoning module.
//!
//! This module re-exports the canonical Layer E types and functions from
//! `litgraph_core::reasoning` so that `commands::poler` can consume them
//! without reaching across crate boundaries directly.
//!
//! ## Layer E recap
//!
//! Layer E (implemented in `litgraph-core/src/reasoning/`) provides:
//! - [`NarrativeGraph`] — builds the POS-filtered character adjacency matrix
//!   `A_POS` from SVO triplets and computes Ω_conf = ‖A_POS‖_F (Frobenius
//!   norm) and ρ(A_POS) (spectral radius via power iteration).
//! - [`ParadoxDetector`] — scans the manuscript for temporal paradoxes
//!   (Dead-Speaking, Spatial-Teleportation-placeholder).
//! - [`ConflictAnalyzer`] trait — the dependency-injection interface that
//!   Layer D's `compute_epsilon_climax_with_analyzer` accepts.
//! - [`ConflictReport`] — serializable result of conflict analysis.
//!
//! ## Why a separate module?
//!
//! `src-tauri/src/reasoning/` already exists for the *legacy* Wave 1–5
//! reasoning engine (Facts / State / Inference / Cycle / Planner). Layer E
//! is a distinct subsystem (symbolic narrative-graph analysis) that lives
//! in `litgraph-core`. To avoid name collisions and keep imports clean,
//! Layer E is exposed under `crate::poler`.

pub use litgraph_core::reasoning::{
    ConflictAnalyzer, ConflictReport, ManuscriptAnalysis, NarrativeGraph, ParadoxDetector,
    Paradox, ParadoxKind,
};

pub use litgraph_core::parser::epsilon::{compute_epsilon_climax_with_analyzer, EpsilonResult};

pub use litgraph_core::linguistic::svo_parser::{SvoParser, SvoTriplet};

pub use litgraph_core::parser::characters::{detect as detect_characters, ParsedCharacter};

pub use litgraph_core::parser::chapters::{detect as detect_chapters, ParsedChapter};
