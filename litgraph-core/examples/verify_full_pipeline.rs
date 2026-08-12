//! Smoke test for the full Reasoning Engine pipeline with real trained weights.
//!
//! This example exercises the EXACT same code path that the new Tauri command
//! `reasoning_run_full_pipeline` (in `src-tauri/src/commands/reasoning.rs`)
//! uses to wire the engine into the UI. Running this example verifies that:
//!
//! 1. `include_str!`'d weights.json parses correctly via `WeightsFile::from_json`.
//! 2. `ReasoningEngine::with_weights_file(...)` constructs without panicking.
//! 3. `engine.analyze(text, kappa)` runs end-to-end and produces a sensible
//!    `ReasoningReport` (non-empty characters, valid decision tallies,
//!    diagnostics present, weights metadata populated).
//!
//! Run with:
//! ```bash
//! cargo run --example verify_full_pipeline
//! ```

use litgraph_core::reasoning::{ReasoningEngine, ReasoningReport};
use litgraph_core::scorer::WeightsFile;

/// The trained weights file, compiled into the binary at build time.
/// Same path is used by the Tauri command's `include_str!`.
const WEIGHTS_JSON: &str = include_str!("../data/scorer_weights.json");

fn main() {
    println!("=== Reasoning Engine — Full Pipeline Smoke Test ===\n");

    // 1. Load weights from the embedded JSON.
    println!("[1/4] Loading embedded weights.json ({} bytes)...",
        WEIGHTS_JSON.len());
    let weights_file = match WeightsFile::from_json(WEIGHTS_JSON) {
        Ok(w) => {
            println!("  ✓ Loaded: architecture={}, version={}, feature_count={}, hidden_dim={}",
                w.architecture, w.version, w.feature_count, w.hidden_dim);
            w
        }
        Err(e) => {
            eprintln!("  ✗ Failed to load weights: {}", e);
            std::process::exit(1);
        }
    };

    // 2. Construct the engine.
    println!("\n[2/4] Constructing ReasoningEngine...");
    let engine = ReasoningEngine::with_weights_file(weights_file);
    println!("  ✓ Engine ready (scaler mean len={}, std len={})",
        engine.weights_file().scaler.mean.len(),
        engine.weights_file().scaler.std.len());

    // 3. Run analysis on a sample Ukrainian text.
    let text = "Петро сказав Марті: йдемо у ліс. Веня відповів: добре. \
                Іван вбив ворога. Марта пішла додому.";
    println!("\n[3/4] Analyzing sample text ({} chars):", text.chars().count());
    println!("  \"{}\"", text);

    let report: ReasoningReport = engine.analyze(text, 1.0);

    // 4. Print the report.
    println!("\n[4/4] ReasoningReport:");
    println!("  ├─ Characters: total={}, approve={}, reject={}, review={}",
        report.total_characters,
        report.approved_count,
        report.rejected_count,
        report.review_count);
    for c in &report.characters {
        println!("  │   • name={:?} script={:?} decision={:?} refined={:.4} raw={:.4} features={:?}",
            c.parsed.name, c.script, c.decision,
            c.refined_confidence, c.raw_confidence, c.features);
    }
    println!("  ├─ Triplets: total={}, valid_cases={}, invalid_cases={}",
        report.total_triplets, report.triplets_valid_cases, report.triplets_invalid_cases);
    for t in report.triplets.iter().take(5) {
        println!("  │   • actor={:?} verb={:?} target={:?} case={:?} actor_is_char={} target_is_char={}",
            t.triplet.actor, t.triplet.verb, t.triplet.target,
            t.case_validation.overall, t.is_actor_character, t.is_target_character);
    }
    println!("  ├─ POLER ε_climax: epsilon={:.4} normalized={:.2} is_climax={} word_count={}",
        report.epsilon.epsilon, report.epsilon.normalized,
        report.epsilon.is_climax, report.epsilon.word_count);
    println!("  ├─ Conflict: Ω_conf={:.4} ρ(A)={:.4} nodes={} edges={} paradoxes={}",
        report.conflict.omega_conf, report.conflict.spectral_radius,
        report.conflict.node_count, report.conflict.edge_count,
        report.conflict.paradoxes.len());
    println!("  ├─ Diagnostics: overall_health={:?}",
        report.diagnostics.overall_health);
    println!("  │   ├─ class_imbalance: approve={}, reject={}, review={}, ratio={:.2}, imbalanced={}",
        report.diagnostics.class_imbalance.approve_count,
        report.diagnostics.class_imbalance.reject_count,
        report.diagnostics.class_imbalance.review_count,
        report.diagnostics.class_imbalance.approve_reject_ratio,
        report.diagnostics.class_imbalance.is_imbalanced);
    println!("  │   ├─ score_distribution: mean={:.4} std={:.4} separation={:.4} underfitting_detected={}",
        report.diagnostics.score_distribution.mean,
        report.diagnostics.score_distribution.std,
        report.diagnostics.score_distribution.separation,
        report.diagnostics.score_distribution.underfitting_detected);
    println!("  │   ├─ script_analysis: cyr={} lat={} mixed={} other={} latin_frac={:.2} parallel_text_detected={}",
        report.diagnostics.script_analysis.cyrillic_count,
        report.diagnostics.script_analysis.latin_count,
        report.diagnostics.script_analysis.mixed_count,
        report.diagnostics.script_analysis.other_count,
        report.diagnostics.script_analysis.latin_fraction,
        report.diagnostics.script_analysis.parallel_text_detected);
    println!("  │   ├─ feature_informativeness: low_info_count={} (indices={:?})",
        report.diagnostics.feature_informativeness.low_information_features.len(),
        report.diagnostics.feature_informativeness.low_information_features);
    println!("  │   │   per_feature_std = {:?}",
        report.diagnostics.feature_informativeness.per_feature_std);
    println!("  │   └─ weight_magnitude: fc1_std={:.4} fc1_max={:.4} fc2_std={:.4} collapse={} explosion={}",
        report.diagnostics.weight_magnitude.fc1_weight_std,
        report.diagnostics.weight_magnitude.fc1_weight_max,
        report.diagnostics.weight_magnitude.fc2_weight_std,
        report.diagnostics.weight_magnitude.collapse_detected,
        report.diagnostics.weight_magnitude.explosion_detected);
    println!("  ├─ Weights: version={}, architecture={}",
        report.weights_version, report.weights_architecture);
    println!("  └─ text_length={}", report.text_length);

    // Sanity assertions.
    println!("\n=== Sanity assertions ===");
    assert!(report.total_characters >= 1, "must detect >=1 character");
    assert_eq!(
        report.approved_count + report.rejected_count + report.review_count,
        report.total_characters,
        "decision tallies must sum to total"
    );
    assert!(!report.weights_version.is_empty(), "weights version must be populated");
    assert!(!report.diagnostics.overall_health.is_empty(), "diagnostics must have overall_health");
    assert_eq!(
        report.diagnostics.feature_informativeness.per_feature_std.len(),
        11,
        "must have 11 features (case-aware MLP)"
    );
    println!("  ✓ All sanity assertions passed.");
    println!("\n=== Smoke test PASSED — ReasoningEngine full pipeline works end-to-end. ===");
    println!("\nThis is the exact code path that the Tauri command");
    println!("`reasoning_run_full_pipeline` (in src-tauri/src/commands/reasoning.rs)");
    println!("will invoke when the user clicks the Reasoning button in the UI.");
}
