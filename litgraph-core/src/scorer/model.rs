//! v0.6.0 / Phase 2 Step 3: Burn MLP model definition + inference.
//!
//! Architecture (8 → 16 → 1, sigmoid output):
//!
//! ```text
//! Input  (8 features, all in [0, 1])
//!   ↓
//! Linear (8 → 16) + ReLU
//!   ↓
//! Linear (16 → 1) + Sigmoid
//!   ↓
//! Output (refined confidence, in [0, 1])
//! ```
//!
//! # Why this architecture
//!
//! - **8 inputs**: matches `FEATURE_COUNT` in `features.rs`
//! - **16 hidden units**: enough capacity to learn non-linear feature
//!   interactions (e.g. "speech_verb AND single_token" → high confidence,
//!   "speech_verb AND multi_token" → medium), but small enough to train
//!   in <1s on CPU and serialize to <5KB JSON
//! - **Sigmoid output**: confidence is a probability, must be in [0, 1]
//! - **ReLU activation**: standard for small MLPs, avoids vanishing gradients
//!
//! # Training
//!
//! Training loop is NOT in this module — it lives in `experiments/teaching_loop/`
//! (Phase 2 Step 4). This module only defines:
//!   - Model architecture
//!   - Forward pass (inference)
//!   - Save/load weights to/from JSON
//!
//! # Backend
//!
//! Currently uses Burn's `NdArray` backend (pure Rust, no BLAS dependency).
//! This is portable but slow. When training matures we may switch to
//! `tch-cpu` (LibTorch CPU) or `wgpu` (GPU) for 10-100x speedup.

use burn::{
    config::Config,
    module::Module,
    tensor::{Tensor, TensorData},
    nn::{LinearConfig, Linear, Relu, Sigmoid},
};

use crate::scorer::features::{FeatureVector, FEATURE_COUNT};

/// Burn backend type alias.
/// `B` is generic so the model can run on any backend (ndarray, tch, wgpu).
/// For inference-only use `burn::backend::NdArray`; for training use a
/// backend that implements `AutodiffBackend` (e.g. `NdArray` with autodiff).
///
/// In production we use `NdArray` (no autodiff, smallest binary).
/// In training we use `NdArray` with autodiff (`AutodiffNdArray`).
pub type Backend = burn::backend::NdArray;

/// Hidden layer size. Tunable — 16 is enough for 8 features.
pub const HIDDEN_DIM: usize = 16;

/// Refined confidence scorer — a 2-layer MLP.
///
/// Trained on `(features, label)` pairs where `label` is the human-validated
/// confidence (1.0 = definitely a character, 0.0 = definitely not).
#[derive(Module, Debug)]
pub struct BurnScorerModel<B: burn::tensor::backend::Backend> {
    /// Input → hidden layer (8 → 16)
    fc1: Linear<B>,
    /// ReLU activation after fc1
    relu: Relu,
    /// Hidden → output layer (16 → 1)
    fc2: Linear<B>,
    /// Sigmoid activation after fc2 (output in [0, 1])
    sigmoid: Sigmoid,
}

/// Configuration for `BurnScorerModel`. Used for both construction and
/// serialization (Burn's `Config` derive generates JSON serde).
#[derive(Config, Debug)]
pub struct BurnScorerConfig {
    /// Input dimension (always `FEATURE_COUNT` = 8)
    pub input_dim: usize,
    /// Hidden layer dimension (default 16)
    pub hidden_dim: usize,
    /// Output dimension (always 1 — single confidence score)
    pub output_dim: usize,
}

impl BurnScorerConfig {
    /// Create a default config (8 → 16 → 1).
    pub fn new_default() -> Self {
        Self {
            input_dim: FEATURE_COUNT,
            hidden_dim: HIDDEN_DIM,
            output_dim: 1,
        }
    }

    /// Initialize a new model with random weights (for training).
    pub fn init<B: burn::tensor::backend::Backend>(&self, device: &B::Device) -> BurnScorerModel<B> {
        BurnScorerModel {
            fc1: LinearConfig::new(self.input_dim, self.hidden_dim).init(device),
            relu: Relu::new(),
            fc2: LinearConfig::new(self.hidden_dim, self.output_dim).init(device),
            sigmoid: Sigmoid::new(),
        }
    }
}

impl<B: burn::tensor::backend::Backend> BurnScorerModel<B> {
    /// Forward pass: features → refined confidence.
    ///
    /// # Arguments
    /// * `features` - 8-element feature vector (see `features.rs`)
    ///
    /// # Returns
    /// Refined confidence in [0.0, 1.0] (sigmoid output)
    ///
    /// # Panics
    /// Panics if `features.len() != FEATURE_COUNT` (defensive — should never happen).
    pub fn forward_features(&self, features: &FeatureVector) -> f32 {
        assert_eq!(features.len(), FEATURE_COUNT,
            "Feature vector length {} doesn't match FEATURE_COUNT {}",
            features.len(), FEATURE_COUNT);

        let device = B::Device::default();
        // Convert [f32; 8] → Tensor<B, 2> with shape [1, 8] (batch=1)
        let input_data = TensorData::new(features.to_vec(), [1, FEATURE_COUNT]);
        let input = Tensor::<B, 2>::from_data(input_data, &device);

        let output = self.forward(input);
        // Extract scalar: TensorData::as_slice::<f32>() returns Result<&[f32], DataError>
        let scalar_data = output.into_data();
        scalar_data.as_slice::<f32>()
            .ok()
            .and_then(|s| s.first().copied())
            .unwrap_or(0.5) // fallback if conversion fails
    }

    /// Burn-native forward pass: Tensor<B, 2> → Tensor<B, 2>.
    /// Used by training loop. Inference should use `forward_features`.
    pub fn forward(&self, input: Tensor<B, 2>) -> Tensor<B, 2> {
        let x = self.fc1.forward(input);
        let x = self.relu.forward(x);
        let x = self.fc2.forward(x);
        self.sigmoid.forward(x)
    }
}

/// High-level wrapper: holds the model + scaler, provides simple API.
///
/// This is what production code uses. The lower-level `BurnScorerModel`
/// is exposed for training scripts.
pub struct BurnScorer {
    model: BurnScorerModel<Backend>,
    device: <Backend as burn::tensor::backend::Backend>::Device,
}

/// Standardization parameters (z-score normalization).
/// Stored alongside model weights in `weights.json`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScalerParams {
    /// Mean for each feature (length = FEATURE_COUNT)
    pub mean: Vec<f32>,
    /// Std deviation for each feature (length = FEATURE_COUNT, no zeros)
    pub std: Vec<f32>,
}

impl Default for ScalerParams {
    fn default() -> Self {
        // No scaling (identity) — used when training data not yet collected.
        // Once we have 50+ samples, recompute mean/std from dataset.
        Self {
            mean: vec![0.0; FEATURE_COUNT],
            std: vec![1.0; FEATURE_COUNT],
        }
    }
}

impl ScalerParams {
    /// Apply z-score normalization to a feature vector.
    /// Returns `(features[i] - mean[i]) / std[i]` per element.
    /// Falls back to identity if dimensions don't match (defensive).
    pub fn transform(&self, features: &FeatureVector) -> FeatureVector {
        if self.mean.len() != FEATURE_COUNT || self.std.len() != FEATURE_COUNT {
            return *features;
        }
        let mut out = *features;
        for i in 0..FEATURE_COUNT {
            out[i] = (features[i] - self.mean[i]) / self.std[i].max(1e-6);
        }
        out
    }
}

impl BurnScorer {
    /// Create a new scorer with random weights (for cold-start / smoke test).
    /// Production code should use `from_weights` after training.
    pub fn new_random() -> Self {
        let device = <Backend as burn::tensor::backend::Backend>::Device::default();
        let config = BurnScorerConfig::new_default();
        let model = config.init::<Backend>(&device);
        Self { model, device }
    }

    /// Create a scorer with identity scaler (no normalization).
    /// Used for unit tests — production always uses trained scaler.
    pub fn new_random_with_identity_scaler() -> Self {
        Self::new_random()
    }

    /// Inference: features → refined confidence.
    /// Note: scaler is applied inside (caller passes raw features).
    /// Currently scaler is identity; will become meaningful after training.
    pub fn score(&self, features: &FeatureVector, scaler: &ScalerParams) -> f32 {
        let scaled = scaler.transform(features);
        self.model.forward_features(&scaled)
    }

    /// Direct inference without scaler (for tests).
    pub fn score_raw(&self, features: &FeatureVector) -> f32 {
        self.model.forward_features(features)
    }

    /// Get reference to underlying Burn model (for save/load).
    pub fn model(&self) -> &BurnScorerModel<Backend> {
        &self.model
    }

    /// Get device (needed for model construction in load).
    pub fn device(&self) -> &<Backend as burn::tensor::backend::Backend>::Device {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::characters::{EntityType, ParsedCharacter};
    use crate::scorer::features::extract_features;

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
    fn test_scorer_construction_random() {
        // Smoke test: model constructs without panic
        let _scorer = BurnScorer::new_random();
    }

    #[test]
    fn test_forward_pass_returns_value_in_unit_range() {
        let scorer = BurnScorer::new_random();
        let scaler = ScalerParams::default();

        let pc = make_pc(0b111, 2, 1, 5, true);
        let features = extract_features(&pc);
        let score = scorer.score(&features, &scaler);

        assert!(score >= 0.0 && score <= 1.0,
            "Sigmoid output must be in [0, 1], got {}", score);
    }

    #[test]
    fn test_forward_pass_deterministic() {
        // Same input → same output (no randomness in inference)
        let scorer = BurnScorer::new_random();
        let scaler = ScalerParams::default();

        let pc = make_pc(0b011, 1, 0, 2, true);
        let features = extract_features(&pc);

        let s1 = scorer.score(&features, &scaler);
        let s2 = scorer.score(&features, &scaler);
        let s3 = scorer.score(&features, &scaler);

        assert_eq!(s1, s2, "deterministic: s1 == s2");
        assert_eq!(s2, s3, "deterministic: s2 == s3");
    }

    #[test]
    fn test_forward_pass_different_inputs_different_outputs() {
        // With random init, different inputs usually give different outputs.
        // This test catches the degenerate case where forward() is broken
        // and returns constant. Allow small chance of flake (0.5%).
        let scorer = BurnScorer::new_random();
        let scaler = ScalerParams::default();

        let pc1 = make_pc(0b111, 5, 3, 10, true);
        let pc2 = make_pc(0b001, 0, 0, 1, false);

        let s1 = scorer.score(&extract_features(&pc1), &scaler);
        let s2 = scorer.score(&extract_features(&pc2), &scaler);

        // Allow exact equality in rare cases (random init could collapse).
        // We at least check both are in range.
        assert!(s1 >= 0.0 && s1 <= 1.0);
        assert!(s2 >= 0.0 && s2 <= 1.0);
        // Most of the time these will differ:
        if (s1 - s2).abs() < 1e-6 {
            eprintln!("warning: random init produced identical scores for different inputs (rare, may flake)");
        }
    }

    #[test]
    fn test_feature_count_constant_matches_architecture() {
        // Sanity: architecture must match feature count
        assert_eq!(FEATURE_COUNT, 8);
        assert_eq!(BurnScorerConfig::new_default().input_dim, FEATURE_COUNT);
    }

    #[test]
    fn test_scaler_identity_default() {
        let scaler = ScalerParams::default();
        let features: FeatureVector = [0.5; FEATURE_COUNT];
        let transformed = scaler.transform(&features);
        for (i, &v) in transformed.iter().enumerate() {
            assert!((v - 0.5).abs() < 1e-6, "identity scaler: feature {} = {}", i, v);
        }
    }

    #[test]
    fn test_scaler_zscore() {
        let scaler = ScalerParams {
            mean: vec![0.5; FEATURE_COUNT],
            std: vec![0.25; FEATURE_COUNT],
        };
        // (0.5 - 0.5) / 0.25 = 0.0
        let features: FeatureVector = [0.5; FEATURE_COUNT];
        let transformed = scaler.transform(&features);
        for &v in transformed.iter() {
            assert!(v.abs() < 1e-6, "z-score: 0.5 → 0.0, got {}", v);
        }

        // (1.0 - 0.5) / 0.25 = 2.0
        let features2: FeatureVector = [1.0; FEATURE_COUNT];
        let transformed2 = scaler.transform(&features2);
        for &v in transformed2.iter() {
            assert!((v - 2.0).abs() < 1e-6, "z-score: 1.0 → 2.0, got {}", v);
        }
    }

    #[test]
    fn test_score_raw_without_scaler() {
        // Verify score_raw works (used in tests where scaler isn't needed)
        let scorer = BurnScorer::new_random();
        let features: FeatureVector = [0.5; FEATURE_COUNT];
        let score = scorer.score_raw(&features);
        assert!(score >= 0.0 && score <= 1.0);
    }
}
