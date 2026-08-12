//! v0.6.0 / Phase 2 Step 3: Weights file I/O (JSON serialization).
//!
//! Defines the on-disk format for trained model artifacts. This is the
//! `weights.json` file mentioned in arch plan §5.4.
//!
//! # Format
//!
//! ```json
//! {
//!   "version": "0.6.0",
//!   "trained_at": "2026-08-12T14:30:00Z",
//!   "feature_count": 8,
//!   "hidden_dim": 16,
//!   "architecture": "mlp_8_16_1_sigmoid",
//!   "scaler": {
//!     "mean": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
//!     "std":  [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
//!   },
//!   "weights": {
//!     "fc1_weight": [[...16 arrays of 8 floats...]],
//!     "fc1_bias":   [...16 floats...],
//!     "fc2_weight": [[...1 array of 16 floats...]],
//!     "fc2_bias":   [...1 float...]
//!   }
//! }
//! ```
//!
//! # Status
//!
//! **Spike**: format is defined but `save()` / `load()` are stubs.
//! Real implementation comes in Phase 2 Step 4 (training loop) — for now
//! we only verify the JSON structure compiles and serializes correctly.

use crate::scorer::{ScalerParams, FEATURE_COUNT, HIDDEN_DIM};
use serde::{Deserialize, Serialize};

/// Error type for weights file operations.
#[derive(Debug, thiserror::Error)]
pub enum WeightsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("architecture mismatch: expected feature_count={expected}, got {actual}")]
    ArchitectureMismatch { expected: usize, actual: usize },
    #[error("missing field: {0}")]
    MissingField(String),
}

/// On-disk weights file format. See module docs for schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightsFile {
    /// Format version (semver). Bumped on incompatible changes.
    pub version: String,
    /// ISO-8601 timestamp of last training run.
    pub trained_at: String,
    /// Number of input features (must match `FEATURE_COUNT`).
    pub feature_count: usize,
    /// Hidden layer size (must match `HIDDEN_DIM`).
    pub hidden_dim: usize,
    /// Architecture string (e.g. "mlp_8_16_1_sigmoid").
    pub architecture: String,
    /// Z-score normalization parameters.
    pub scaler: ScalerParams,
    /// Model weights (raw arrays — Burn handles TensorData conversion).
    pub weights: WeightsData,
}

/// Raw weight arrays, matching `BurnScorerModel` layer-by-layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightsData {
    /// fc1 weight matrix, shape [hidden_dim, feature_count]
    pub fc1_weight: Vec<Vec<f32>>,
    /// fc1 bias vector, length hidden_dim
    pub fc1_bias: Vec<f32>,
    /// fc2 weight matrix, shape [1, hidden_dim]
    pub fc2_weight: Vec<Vec<f32>>,
    /// fc2 bias vector, length 1
    pub fc2_bias: Vec<f32>,
}

impl WeightsFile {
    /// Create a default weights file (random init values, identity scaler).
    /// Used for cold-start when no trained weights exist yet.
    pub fn new_default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            trained_at: chrono::Utc::now().to_rfc3339(),
            feature_count: FEATURE_COUNT,
            hidden_dim: HIDDEN_DIM,
            architecture: format!("mlp_{}_{}_1_sigmoid", FEATURE_COUNT, HIDDEN_DIM),
            scaler: ScalerParams::default(),
            weights: WeightsData {
                // Random init values (placeholder — Burn will overwrite during training)
                // Use small deterministic values to make tests reproducible.
                fc1_weight: (0..HIDDEN_DIM)
                    .map(|i| (0..FEATURE_COUNT).map(|j| 0.01 * ((i + j) as f32)).collect())
                    .collect(),
                fc1_bias: vec![0.0; HIDDEN_DIM],
                fc2_weight: vec![(0..HIDDEN_DIM).map(|i| 0.01 * (i as f32)).collect()],
                fc2_bias: vec![0.0],
            },
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, WeightsError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, WeightsError> {
        let parsed: WeightsFile = serde_json::from_str(json)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// Write to file at given path.
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), WeightsError> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Read from file at given path.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, WeightsError> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json(&json)
    }

    /// Validate that dimensions match constants.
    pub fn validate(&self) -> Result<(), WeightsError> {
        if self.feature_count != FEATURE_COUNT {
            return Err(WeightsError::ArchitectureMismatch {
                expected: FEATURE_COUNT,
                actual: self.feature_count,
            });
        }
        if self.hidden_dim != HIDDEN_DIM {
            return Err(WeightsError::ArchitectureMismatch {
                expected: HIDDEN_DIM,
                actual: self.hidden_dim,
            });
        }
        if self.scaler.mean.len() != FEATURE_COUNT {
            return Err(WeightsError::MissingField(
                format!("scaler.mean has length {}, expected {}", self.scaler.mean.len(), FEATURE_COUNT)
            ));
        }
        if self.scaler.std.len() != FEATURE_COUNT {
            return Err(WeightsError::MissingField(
                format!("scaler.std has length {}, expected {}", self.scaler.std.len(), FEATURE_COUNT)
            ));
        }
        if self.weights.fc1_weight.len() != HIDDEN_DIM {
            return Err(WeightsError::MissingField(
                format!("fc1_weight has {} rows, expected {}", self.weights.fc1_weight.len(), HIDDEN_DIM)
            ));
        }
        if self.weights.fc1_bias.len() != HIDDEN_DIM {
            return Err(WeightsError::MissingField(
                format!("fc1_bias has length {}, expected {}", self.weights.fc1_bias.len(), HIDDEN_DIM)
            ));
        }
        if self.weights.fc2_weight.len() != 1 {
            return Err(WeightsError::MissingField(
                format!("fc2_weight has {} rows, expected 1", self.weights.fc2_weight.len())
            ));
        }
        if self.weights.fc2_bias.len() != 1 {
            return Err(WeightsError::MissingField(
                format!("fc2_bias has length {}, expected 1", self.weights.fc2_bias.len())
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights_file_validates() {
        let wf = WeightsFile::new_default();
        wf.validate().expect("default weights file should be valid");
    }

    #[test]
    fn test_json_roundtrip() {
        let wf = WeightsFile::new_default();
        let json = wf.to_json().expect("serialize");
        let wf2 = WeightsFile::from_json(&json).expect("deserialize");

        assert_eq!(wf.feature_count, wf2.feature_count);
        assert_eq!(wf.hidden_dim, wf2.hidden_dim);
        assert_eq!(wf.architecture, wf2.architecture);
        assert_eq!(wf.weights.fc1_weight.len(), wf2.weights.fc1_weight.len());
        assert_eq!(wf.weights.fc2_bias.len(), wf2.weights.fc2_bias.len());
    }

    #[test]
    fn test_file_roundtrip() {
        let tmp = std::env::temp_dir().join("litgraph_burn_weights_test.json");
        let wf = WeightsFile::new_default();
        wf.save_to_file(&tmp).expect("save");
        let wf2 = WeightsFile::load_from_file(&tmp).expect("load");
        assert_eq!(wf.feature_count, wf2.feature_count);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_invalid_feature_count_rejected() {
        let mut wf = WeightsFile::new_default();
        wf.feature_count = 99; // wrong
        let result = wf.validate();
        assert!(result.is_err(), "wrong feature_count must fail validation");
        match result.unwrap_err() {
            WeightsError::ArchitectureMismatch { expected, actual } => {
                assert_eq!(expected, FEATURE_COUNT);
                assert_eq!(actual, 99);
            }
            _ => panic!("expected ArchitectureMismatch error"),
        }
    }

    #[test]
    fn test_default_weights_file_json_has_expected_fields() {
        let wf = WeightsFile::new_default();
        let json = wf.to_json().expect("serialize");
        // Verify key fields are present in JSON
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"trained_at\""));
        assert!(json.contains("\"feature_count\": 8"));
        assert!(json.contains("\"hidden_dim\": 16"));
        assert!(json.contains("\"architecture\": \"mlp_8_16_1_sigmoid\""));
        assert!(json.contains("\"scaler\""));
        assert!(json.contains("\"weights\""));
        assert!(json.contains("\"fc1_weight\""));
        assert!(json.contains("\"fc1_bias\""));
        assert!(json.contains("\"fc2_weight\""));
        assert!(json.contains("\"fc2_bias\""));
    }

    #[test]
    fn test_default_weights_size_reasonable() {
        // Smoke: weights file should serialize to <50KB (small enough to git-track)
        let wf = WeightsFile::new_default();
        let json = wf.to_json().expect("serialize");
        assert!(json.len() < 50_000,
            "weights file too large: {} bytes (expected <50KB)", json.len());
    }
}
