//! v0.7.0 / Reasoning Engine: Pure-Rust MLP inference consuming `WeightsFile`.
//!
//! This module is the **production inference path** for the Burn scorer.
//! It reads `weights.json` (the training artifact produced by `train_scorer`
//! binary) and runs the forward pass in pure Rust — no Burn backend, no
//! autodiff, no Device generics. Burn remains the **training-only** backend.
//!
//! # Why a separate inference path?
//!
//! 1. **Cold-start reliability**: Burn 0.14 `Linear::with_weights()` requires
//!    `Param::from_data()` gymnastics and a backend `Device`. Pure Rust just
//!    needs `&[f32]` slices. No backend initialization, no panic paths.
//! 2. **WASM/Tauri portability**: Reasoning Engine will eventually run in the
//!    Tauri renderer thread. Pure Rust = no Burn-ndarray native lib dependency
//!    in the frontend bundle.
//! 3. **Determinism**: Burn's ndarray backend is deterministic, but it floats
//!    f32 through tensor abstractions that obscure audit. The Reasoning Engine
//!    must be auditable end-to-end: `weights[i] → output` should be traceable.
//! 4. **No runtime cost**: forward pass is ~16 MAC ops. Burn's overhead
//!    (TensorData conversion, backend dispatch) dominates at this scale.
//!
//! # Architecture match
//!
//! Forward pass mirrors `BurnScorerModel::forward()` in `model.rs`:
//!
//! ```text
//! Input  (8 features)
//!   ↓ z-score normalize: (x - mean) / std
//! Linear (8 → 16) + ReLU
//!   ↓
//! Linear (16 → 1) + Sigmoid
//!   ↓
//! Output (refined confidence, [0.0, 1.0])
//! ```
//!
//! # Weight layout
//!
//! From `BurnScorerModel::extract_weights_flat()` (model.rs):
//! - `fc1_weight: Vec<Vec<f32>>` shape [HIDDEN_DIM=16, FEATURE_COUNT=8]
//!   (rows = output neurons, cols = input features)
//! - `fc1_bias: Vec<f32>` length 16
//! - `fc2_weight: Vec<Vec<f32>>` shape [1, HIDDEN_DIM=16]
//! - `fc2_bias: Vec<f32>` length 1
//!
//! Forward pass:
//! ```text
//! for h in 0..16:
//!     z[h] = fc1_bias[h] + Σ_f (fc1_weight[h][f] * scaled[f])
//!     a[h] = max(0, z[h])                      // ReLU
//! output = 1 / (1 + exp(-(fc2_bias[0] + Σ_h (fc2_weight[0][h] * a[h]))))
//! ```

use crate::scorer::features::{FeatureVector, FEATURE_COUNT};
use crate::scorer::model::ScalerParams;
use crate::scorer::weights::{WeightsData, WeightsFile, WeightsError};

/// Hidden layer size — must match `HIDDEN_DIM` in `model.rs`.
const INFERENCE_HIDDEN_DIM: usize = 16;

/// Pure-Rust MLP scorer. Holds owned weights + scaler.
///
/// Constructed from a `WeightsFile` (loaded from `weights.json`).
/// Cheap to clone (16*8 + 16 + 16 + 1 = 161 f32 = ~644 bytes).
#[derive(Debug, Clone)]
pub struct InferenceScorer {
    /// Z-score normalization params (mean/std per feature).
    pub scaler: ScalerParams,
    /// FC1 weight matrix [HIDDEN_DIM][FEATURE_COUNT].
    pub fc1_weight: Vec<Vec<f32>>,
    /// FC1 bias [HIDDEN_DIM].
    pub fc1_bias: Vec<f32>,
    /// FC2 weight matrix [1][HIDDEN_DIM].
    pub fc2_weight: Vec<Vec<f32>>,
    /// FC2 bias [1].
    pub fc2_bias: Vec<f32>,
    /// Architecture string from source weights file (for audit).
    pub architecture: String,
    /// Version string from source weights file (for audit).
    pub version: String,
}

impl InferenceScorer {
    /// Load weights from a `WeightsFile` (already deserialized from JSON).
    ///
    /// Validates dimensions match `FEATURE_COUNT` and `HIDDEN_DIM` constants.
    pub fn from_weights_file(wf: WeightsFile) -> Result<Self, WeightsError> {
        wf.validate()?;
        let WeightsData {
            fc1_weight,
            fc1_bias,
            fc2_weight,
            fc2_bias,
        } = wf.weights;

        // Extra defensive: validate inner dimensions
        if fc1_weight.len() != INFERENCE_HIDDEN_DIM {
            return Err(WeightsError::MissingField(format!(
                "fc1_weight rows = {}, expected {}",
                fc1_weight.len(),
                INFERENCE_HIDDEN_DIM
            )));
        }
        for (h, row) in fc1_weight.iter().enumerate() {
            if row.len() != FEATURE_COUNT {
                return Err(WeightsError::MissingField(format!(
                    "fc1_weight[{}] cols = {}, expected {}",
                    h,
                    row.len(),
                    FEATURE_COUNT
                )));
            }
        }
        if fc1_bias.len() != INFERENCE_HIDDEN_DIM {
            return Err(WeightsError::MissingField(format!(
                "fc1_bias len = {}, expected {}",
                fc1_bias.len(),
                INFERENCE_HIDDEN_DIM
            )));
        }
        if fc2_weight.len() != 1 || fc2_weight[0].len() != INFERENCE_HIDDEN_DIM {
            return Err(WeightsError::MissingField(format!(
                "fc2_weight shape = {:?}, expected [[{}]]",
                fc2_weight.iter().map(|r| r.len()).collect::<Vec<_>>(),
                INFERENCE_HIDDEN_DIM
            )));
        }
        if fc2_bias.len() != 1 {
            return Err(WeightsError::MissingField(format!(
                "fc2_bias len = {}, expected 1",
                fc2_bias.len()
            )));
        }

        Ok(Self {
            scaler: wf.scaler,
            fc1_weight,
            fc1_bias,
            fc2_weight,
            fc2_bias,
            architecture: wf.architecture,
            version: wf.version,
        })
    }

    /// Load weights directly from a JSON file path.
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, WeightsError> {
        let wf = WeightsFile::load_from_file(path)?;
        Self::from_weights_file(wf)
    }

    /// Forward pass: 8 raw features → refined confidence in [0, 1].
    ///
    /// Mirrors `BurnScorerModel::forward_features()` math exactly:
    ///   1. z-score normalize: `scaled[i] = (features[i] - mean[i]) / std[i]`
    ///   2. FC1 + ReLU:        `a[h] = relu(fc1_bias[h] + Σ_f fc1_weight[h][f] * scaled[f])`
    ///   3. FC2 + Sigmoid:     `out = sigmoid(fc2_bias[0] + Σ_h fc2_weight[0][h] * a[h])`
    pub fn score(&self, features: &FeatureVector) -> f32 {
        // Step 1: z-score normalization
        let scaled = self.scaler.transform(features);

        // Step 2: FC1 + ReLU
        let mut hidden = [0.0_f32; INFERENCE_HIDDEN_DIM];
        for h in 0..INFERENCE_HIDDEN_DIM {
            let mut acc = self.fc1_bias[h];
            let row = &self.fc1_weight[h];
            for f in 0..FEATURE_COUNT {
                acc += row[f] * scaled[f];
            }
            hidden[h] = acc.max(0.0); // ReLU
        }

        // Step 3: FC2 + Sigmoid
        let mut z_out = self.fc2_bias[0];
        let row = &self.fc2_weight[0];
        for h in 0..INFERENCE_HIDDEN_DIM {
            z_out += row[h] * hidden[h];
        }

        // Numerically stable sigmoid
        sigmoid(z_out)
    }

    /// Score a batch of feature vectors. Returns Vec<f32> aligned with input.
    pub fn score_batch(&self, features: &[FeatureVector]) -> Vec<f32> {
        features.iter().map(|f| self.score(f)).collect()
    }

    /// Decision policy: convert raw score to discrete [`Decision`].
    ///
    /// Thresholds calibrated for class-imbalance awareness:
    /// - `score >= 0.65` → `Approve` (high-confidence character)
    /// - `score <= 0.35` → `Reject` (high-confidence non-character)
    /// - `0.35 < score < 0.65` → `Review` (manual inspection needed)
    ///
    /// The wide `Review` band is deliberate: when the model is underfit
    /// (class imbalance 10:1, low-informative features), border cases
    /// are where most errors cluster. Routing them to `Review` keeps
    /// the engine safe while diagnostics (see `diagnostics.rs`) flag
    /// the underfitting.
    pub fn decide(&self, score: f32) -> Decision {
        if score >= 0.65 {
            Decision::Approve
        } else if score <= 0.35 {
            Decision::Reject
        } else {
            Decision::Review
        }
    }

    /// Combined score + decision in one call.
    pub fn score_and_decide(&self, features: &FeatureVector) -> (f32, Decision) {
        let s = self.score(features);
        (s, self.decide(s))
    }
}

/// Numerically stable sigmoid: `1 / (1 + exp(-x))`.
///
/// For large positive x: returns 1.0 (saturates).
/// For large negative x: returns 0.0 (saturates).
/// Avoids `exp(overflow)` → NaN.
fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        let z = (-x).exp();
        1.0 / (1.0 + z)
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// Discrete decision produced by the Reasoning Engine for each character candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// Model is confident this is a character (score >= 0.65).
    Approve,
    /// Model is confident this is NOT a character (score <= 0.35).
    Reject,
    /// Model is uncertain — manual review needed (0.35 < score < 0.65).
    Review,
}

impl Decision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Decision::Approve => "approve",
            Decision::Reject => "reject",
            Decision::Review => "review",
        }
    }

    /// Numerical label for ML training: Approve → 1.0, Reject → 0.0, Review → 0.5.
    /// (Review is rarely written to dataset.jsonl — auto_reviewer decides whether
    /// to surface Review cases as Approve or Reject based on linguistic signals.)
    pub fn as_label(&self) -> f32 {
        match self {
            Decision::Approve => 1.0,
            Decision::Reject => 0.0,
            Decision::Review => 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_scorer() -> InferenceScorer {
        // Hand-constructed scorer: identity scaler, deterministic weights.
        // fc1 = identity (16x8 padded with zeros), fc2 = average of hidden.
        let fc1_weight: Vec<Vec<f32>> = (0..INFERENCE_HIDDEN_DIM)
            .map(|h| {
                (0..FEATURE_COUNT)
                    .map(|f| if h < FEATURE_COUNT && h == f { 1.0 } else { 0.0 })
                    .collect()
            })
            .collect();
        let fc1_bias = vec![0.0; INFERENCE_HIDDEN_DIM];
        let fc2_weight = vec![{
            let mut row = vec![0.0; INFERENCE_HIDDEN_DIM];
            for h in 0..FEATURE_COUNT.min(INFERENCE_HIDDEN_DIM) {
                row[h] = 1.0 / FEATURE_COUNT as f32;
            }
            row
        }];
        let fc2_bias = vec![0.0];

        InferenceScorer {
            scaler: ScalerParams::default(),
            fc1_weight,
            fc1_bias,
            fc2_weight,
            fc2_bias,
            architecture: "test_mlp_8_16_1".to_string(),
            version: "test".to_string(),
        }
    }

    #[test]
    fn test_sigmoid_stable_for_large_values() {
        // Sigmoid must not produce NaN/Inf for extreme inputs.
        assert!((sigmoid(1000.0) - 1.0).abs() < 1e-6, "large positive");
        assert!(sigmoid(-1000.0).abs() < 1e-6, "large negative");
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6, "zero");
        // Symmetry: sigmoid(x) + sigmoid(-x) = 1
        for x in [-5.0_f32, -1.0, 0.5, 2.5, 10.0] {
            let s = sigmoid(x) + sigmoid(-x);
            assert!((s - 1.0).abs() < 1e-5, "symmetry broken at x={}: {}", x, s);
        }
    }

    #[test]
    fn test_score_in_unit_range() {
        let scorer = make_test_scorer();
        let features: FeatureVector = [0.5; FEATURE_COUNT];
        let s = scorer.score(&features);
        assert!(s >= 0.0 && s <= 1.0, "score out of range: {}", s);
    }

    #[test]
    fn test_score_deterministic() {
        let scorer = make_test_scorer();
        let features: FeatureVector = [0.3, 0.7, 0.1, 0.9, 0.5, 0.2, 0.8, 0.4];
        let s1 = scorer.score(&features);
        let s2 = scorer.score(&features);
        let s3 = scorer.score(&features);
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }

    #[test]
    fn test_decision_thresholds() {
        let scorer = make_test_scorer();
        assert_eq!(scorer.decide(0.80), Decision::Approve);
        assert_eq!(scorer.decide(0.65), Decision::Approve);
        assert_eq!(scorer.decide(0.64), Decision::Review);
        assert_eq!(scorer.decide(0.50), Decision::Review);
        assert_eq!(scorer.decide(0.36), Decision::Review);
        assert_eq!(scorer.decide(0.35), Decision::Reject);
        assert_eq!(scorer.decide(0.10), Decision::Reject);
    }

    #[test]
    fn test_decision_as_label_round_trip() {
        assert_eq!(Decision::Approve.as_label(), 1.0);
        assert_eq!(Decision::Reject.as_label(), 0.0);
        assert_eq!(Decision::Review.as_label(), 0.5);
    }

    #[test]
    fn test_load_from_default_weights_file() {
        // The default WeightsFile is valid and should load successfully.
        let wf = WeightsFile::new_default();
        let scorer = InferenceScorer::from_weights_file(wf)
            .expect("default weights file should load");
        let features: FeatureVector = [0.5; FEATURE_COUNT];
        let s = scorer.score(&features);
        assert!(s >= 0.0 && s <= 1.0, "default scorer output out of range: {}", s);
    }

    #[test]
    fn test_score_batch_aligned() {
        let scorer = make_test_scorer();
        let batch = vec![
            [0.5; FEATURE_COUNT],
            [0.1; FEATURE_COUNT],
            [0.9; FEATURE_COUNT],
        ];
        let scores = scorer.score_batch(&batch);
        assert_eq!(scores.len(), batch.len());
        for (i, &s) in scores.iter().enumerate() {
            let single = scorer.score(&batch[i]);
            assert!((s - single).abs() < 1e-7, "batch/single mismatch at {}", i);
        }
    }

    #[test]
    fn test_rejects_mismatched_dimensions() {
        let mut wf = WeightsFile::new_default();
        // Corrupt fc1_weight to have wrong number of rows
        wf.weights.fc1_weight = vec![vec![0.0; FEATURE_COUNT]; 10]; // should be 16
        let result = InferenceScorer::from_weights_file(wf);
        assert!(result.is_err(), "must reject wrong fc1_weight rows");
    }
}
