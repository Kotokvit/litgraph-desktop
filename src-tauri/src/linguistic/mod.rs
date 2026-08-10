//! Linguistic layer for Symbolic UA-LP Engine (src-tauri).
//!
//! Re-exports canonical linguistic modules from `litgraph-core`:
//! - [`lemmatizer`]: Base form dictionary resolution via dict_uk
//! - [`pos_tagger`]: 3-pass POS disambiguation via LanguageTool UK rules
//! - [`svo_parser`]: Rule-based SVO Triplet extractor

pub use litgraph_core::linguistic::{lemmatizer, pos_tagger, svo_parser};
