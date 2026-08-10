//! Linguistic layer for Symbolic UA-LP Engine (src-tauri variant).
//!
//! This module mirrors `litgraph-core/src/linguistic/` so that the Tauri
//! backend has direct access to the linguistic pipeline without depending
//! on the `litgraph-core` crate.
//!
//! ## Layers
//!
//! - [`pos_tagger`]: POS disambiguation via LanguageTool UK rules.
//!   Loads `pos_rules.json.gz` (built by `xtask build-pos-tables`).
//!
//! Note: The lemmatizer (Layer A) is NOT included in src-tauri yet —
//! `epsilon.rs` here uses the v7.0 (no-lemmatization) variant.
//! For full Layer A+B pipeline, use `litgraph-core`.

pub mod pos_tagger;
pub mod svo_parser;
