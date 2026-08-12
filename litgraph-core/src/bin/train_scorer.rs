//! v0.6.1 / Phase 2 Step 4: Burn training binary.
//!
//! Читає `dataset.jsonl` (по одному training example per line), тренує
//! `BurnScorerModel` на (features, label) парах, зберігає оновлений
//! `weights.json`.
//!
//! Usage:
//!     cargo run --release --bin train_scorer -- \
//!         --dataset experiments/teaching_loop/dataset.jsonl \
//!         --weights experiments/teaching_loop/weights.json \
//!         --epochs 200
//!
//! Output: weights.json з trained weights + scaler params (mean/std per feature).

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use burn_core::{
    optim::{AdamConfig, Optimizer},
    tensor::{Tensor, TensorData},
};
use burn_autodiff::Autodiff;
use burn_ndarray::NdArray;

use litgraph_core::scorer::{
    features::FEATURE_COUNT,
    model::{Backend, BurnScorerConfig, BurnScorerModel, HIDDEN_DIM, ScalerParams},
    weights::WeightsFile,
};

use serde::Deserialize;

/// One training example from dataset.jsonl
#[derive(Debug, Deserialize)]
struct TrainingExample {
    features: Vec<f32>,
    label: f32,
    /// Optional metadata (not used in training, just for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    lemma: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rust_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<String>,
}

/// Compute mean and std for each feature across the dataset.
fn compute_scaler(examples: &[TrainingExample]) -> ScalerParams {
    let n = examples.len() as f32;
    let mut mean = vec![0.0f32; FEATURE_COUNT];
    let mut m2 = vec![0.0f32; FEATURE_COUNT];

    // Welford's online algorithm for numerical stability
    for (i, ex) in examples.iter().enumerate() {
        let i_f = (i + 1) as f32;
        for j in 0..FEATURE_COUNT {
            let x = ex.features[j];
            let delta = x - mean[j];
            mean[j] += delta / i_f;
            let delta2 = x - mean[j];
            m2[j] += delta * delta2;
        }
    }

    let std: Vec<f32> = (0..FEATURE_COUNT)
        .map(|j| {
            let variance = m2[j] / n;
            // Use larger minimum std (0.5) to avoid division by tiny number → NaN.
            // Features that are constant across all examples (e.g. is_capitalized
            // is always 1.0 for Characters) effectively get zeroed out
            // (mean=1.0, std=0.5 → scaled to 0.0).
            variance.sqrt().max(0.5)
        })
        .collect();

    ScalerParams { mean, std }
}

/// Apply scaler to features.
fn scale_features(features: &[f32], scaler: &ScalerParams) -> [f32; FEATURE_COUNT] {
    let mut out = [0.0f32; FEATURE_COUNT];
    for i in 0..FEATURE_COUNT {
        out[i] = (features[i] - scaler.mean[i]) / scaler.std[i];
    }
    out
}

/// Extract weights from Burn model into serializable WeightsData.
/// Delegates to model.extract_weights_flat() (added in Step 4).
fn extract_weights<B: burn::tensor::backend::Backend>(
    model: &BurnScorerModel<B>,
) -> litgraph_core::scorer::weights::WeightsData {
    model.extract_weights_flat()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut dataset_path = PathBuf::from("experiments/teaching_loop/dataset.jsonl");
    let mut weights_path = PathBuf::from("experiments/teaching_loop/weights.json");
    let mut epochs: usize = 200;
    let mut learning_rate: f64 = 0.01;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dataset" => {
                i += 1;
                dataset_path = PathBuf::from(&args[i]);
            }
            "--weights" => {
                i += 1;
                weights_path = PathBuf::from(&args[i]);
            }
            "--epochs" => {
                i += 1;
                epochs = args[i].parse().expect("epochs");
            }
            "--lr" => {
                i += 1;
                learning_rate = args[i].parse().expect("lr");
            }
            "--help" | "-h" => {
                println!("Usage: train_scorer --dataset <path> --weights <path> [--epochs N] [--lr F]");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    println!("Loading dataset from {}", dataset_path.display());
    let dataset_text = fs::read_to_string(&dataset_path)
        .unwrap_or_else(|e| { eprintln!("Error reading dataset: {}", e); std::process::exit(1); });

    let examples: Vec<TrainingExample> = dataset_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|e| {
                eprintln!("Error parsing line: {}\n  line: {}", e, &line[..line.len().min(200)]);
                std::process::exit(1);
            })
        })
        .collect();

    println!("  Loaded {} examples", examples.len());
    if examples.is_empty() {
        eprintln!("ERROR: empty dataset");
        std::process::exit(1);
    }
    if examples.len() < 50 {
        eprintln!("WARNING: only {} examples (recommended minimum 50)", examples.len());
    }

    let approve_count = examples.iter().filter(|e| e.label == 1.0).count();
    let reject_count = examples.iter().filter(|e| e.label == 0.0).count();
    println!("  Approve (label=1.0): {}", approve_count);
    println!("  Reject  (label=0.0): {}", reject_count);

    // Compute scaler (z-score normalization per feature)
    let scaler = compute_scaler(&examples);
    println!("  Scaler mean: {:?}", scaler.mean);
    println!("  Scaler std:  {:?}", scaler.std);

    // Build training tensors
    let device = <Autodiff<NdArray> as burn_core::tensor::backend::Backend>::Device::default();

    // Pre-scale all features
    let scaled_features: Vec<[f32; FEATURE_COUNT]> = examples.iter()
        .map(|ex| scale_features(&ex.features, &scaler))
        .collect();
    let labels: Vec<f32> = examples.iter().map(|ex| ex.label).collect();

    let n_samples = scaled_features.len();

    // Build input tensor [N, FEATURE_COUNT]
    let input_data: Vec<f32> = scaled_features.iter().flat_map(|f| f.iter().copied()).collect();
    let input_tensor = Tensor::<Autodiff<NdArray>, 2>::from_data(
        TensorData::new(input_data, [n_samples, FEATURE_COUNT]),
        &device,
    );

    // Build label tensor [N, 1]
    let label_tensor = Tensor::<Autodiff<NdArray>, 2>::from_data(
        TensorData::new(labels, [n_samples, 1]),
        &device,
    );

    // Initialize model + optimizer
    let config = BurnScorerConfig::new_default();
    let mut model: BurnScorerModel<Autodiff<NdArray>> = config.init(&device);
    let mut optim = AdamConfig::new().init();

    println!("\nTraining {} epochs (lr={})...", epochs, learning_rate);
    let start = Instant::now();

    for epoch in 0..epochs {
        // Forward
        let preds = model.forward(input_tensor.clone());

        // MSE loss against label_tensor: mean((preds - labels)^2)
        let diff = preds - label_tensor.clone();
        let squared = diff.clone() * diff;  // element-wise square (safe, no NaN)
        let loss = squared.mean();

        let loss_val = loss.to_data()
            .as_slice::<f32>()
            .ok()
            .and_then(|s| s.first().copied())
            .unwrap_or(0.0);

        // Backward + step
        let grads = loss.backward();
        let grads = burn_core::optim::GradientsParams::from_grads(grads, &model);
        model = optim.step(learning_rate, model, grads);

        if epoch % 20 == 0 || epoch == epochs - 1 {
            println!("  epoch {:3}/{:3}: loss = {:.6}", epoch + 1, epochs, loss_val);
        }
    }

    let duration = start.elapsed();
    println!("\n✓ Training done in {:.2}s", duration.as_secs_f64());

    // Extract weights into serializable format
    let weights_data = extract_weights(&model);

    // Build weights file
    let weights_file = WeightsFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        trained_at: chrono::Utc::now().to_rfc3339(),
        feature_count: FEATURE_COUNT,
        hidden_dim: HIDDEN_DIM,
        architecture: format!("mlp_{}_{}_1_sigmoid", FEATURE_COUNT, HIDDEN_DIM),
        scaler,
        weights: weights_data,
    };

    weights_file.validate().expect("trained weights must validate");

    weights_file.save_to_file(&weights_path)
        .unwrap_or_else(|e| { eprintln!("Error saving weights: {}", e); std::process::exit(1); });

    println!("✓ Wrote {}", weights_path.display());

    // Quick sanity: show first few predictions vs labels
    println!("\nSanity check (first 10 examples):");
    let model_inference: BurnScorerModel<Backend> = BurnScorerConfig::new_default()
        .init(&<Backend as burn::tensor::backend::Backend>::Device::default());

    // We can't easily re-load weights into Burn from our JSON format without more code.
    // For now, just re-run forward with the autodiff model.
    let preds = model.forward(input_tensor.clone());
    let preds_slice = preds.into_data().as_slice::<f32>().expect("preds").to_vec();

    for (i, ex) in examples.iter().take(10).enumerate() {
        let pred = preds_slice.get(i).copied().unwrap_or(0.0);
        let lemma = ex.lemma.as_deref().unwrap_or("?");
        let decision = ex.decision.as_deref().unwrap_or("?");
        println!("  [{}] lemma={:<15} label={:.1} pred={:.4} decision={}",
            i, lemma, ex.label, pred, decision);
    }
}
