//! Linguistic layer for Symbolic UA-LP Engine.
//!
//! This module hosts the runtime linguistic components that power the
//! Symbolic Ukrainian Linguistic Processing stack:
//!
//! - [`lemmatizer`]: word-form → lemma resolution via dict_uk (ВЕСУМ) index.
//!   Pure symbolic morphology — no ML, no statistics. Loads
//!   `lemma_index.json.gz` (built by `xtask build-lemmatizer`) at first use.
//!
//! - [`pos_tagger`]: POS disambiguation via LanguageTool UK rules.
//!   Loads `pos_rules.json.gz` (built by `xtask build-pos-tables`) at first use.
//!   Resolves homonymy like "мати" (noun vs. verb) using 450 contextual rules
//!   + 37,728 verb→case government entries.
//!
//! Future layers (planned, not yet implemented):
//! - `svo_parser`: subject-verb-object extraction via UD-Ukrainian-IU patterns.
//!
//! ## Architecture
//!
//! ```text
//! Text → tokenize → lemmatizer → pos_tagger → svo_parser → POLER ε
//! ```
//!
//! Each layer is a self-contained module with deterministic behavior.
//! No layer depends on ML training or stochastic weights.

pub mod case_validation;
pub mod lemmatizer;
pub mod pos_tagger;
pub mod svo_parser;
