//! v0.6.0 / Phase 2 Step 3: Burn scorer — lightweight MLP for refined confidence.
//!
//! Architectural role (see `docs/architecture/LitGraph_Phase2_Teaching_Loop_Burn_Plan.docx` §6):
//!
//! ```text
//! Natasha (Python v2) → candidate entities with morph tags
//! Rust parser → candidate entities with byte offsets + 3-signal evidence
//! Burn scorer → refined confidence score [0.0, 1.0]
//! Production → use refined confidence to rerank / filter candidates
//! ```
//!
//! Burn **does NOT replace** Natasha or the Rust parser. It only learns to
//! weight the 8 features extracted from `ParsedCharacter` to produce a more
//! accurate confidence than the hardcoded `0.3 / 0.7 / 1.0` policy.
//!
//! # Layout
//!
//! - `features` — extract 8 numerical features from `ParsedCharacter`
//! - `model` — Burn MLP definition (8 → 16 → 1, sigmoid output)
//! - `weights` — JSON serialization for `weights.json` (training artifact)
//!
//! # Status
//!
//! **Spike**: forward pass + save/load works, training loop is a stub.
//! Real training happens in `experiments/teaching_loop/` (Phase 2 Step 4).

pub mod features;
pub mod model;
pub mod weights;

pub use features::{extract_features, FEATURE_COUNT, FeatureVector};
pub use model::{BurnScorer, BurnScorerConfig, BurnScorerModel, ScalerParams, HIDDEN_DIM, Backend};
pub use weights::{WeightsFile, WeightsError, WeightsData};
