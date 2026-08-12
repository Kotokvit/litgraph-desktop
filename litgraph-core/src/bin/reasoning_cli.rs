//! v0.7.0 / Reasoning Engine CLI — end-to-end demonstration.
//!
//! Loads `litgraph-core/data/scorer_weights.json`, runs the full 7-stage
//! Reasoning Engine on a text file (or stdin), prints a structured report.
//!
//! # Usage
//!
//! ```bash
//! # Analyze a file
//! cargo run --release --bin reasoning_cli -- path/to/text.txt
//!
//! # Analyze from stdin
//! echo "Петро сказав Марті: йдемо." | cargo run --release --bin reasoning_cli -- -
//!
//! # Use custom weights
//! cargo run --release --bin reasoning_cli -- --weights custom_weights.json text.txt
//!
//! # JSON output (for piping)
//! cargo run --release --bin reasoning_cli -- --json text.txt
//! ```
//!
//! # Output
//!
//! Default: human-readable summary to stdout. With `--json`: full
//! `ReasoningReport` as JSON (one line, suitable for `jq`).

use std::io::{Read, Write};
use std::path::PathBuf;

use litgraph_core::reasoning::{ReasoningEngine, ReasoningReport};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut text_path: Option<PathBuf> = None;
    let mut weights_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/scorer_weights.json");
    let mut json_output = false;
    let mut kappa: f64 = 1.0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--weights" | "-w" => {
                i += 1;
                weights_path = PathBuf::from(&args[i]);
            }
            "--json" => {
                json_output = true;
            }
            "--kappa" | "-k" => {
                i += 1;
                kappa = args[i].parse().expect("kappa must be a float");
            }
            "--help" | "-h" => {
                print_usage(&args[0]);
                return;
            }
            "-" => {
                text_path = None; // read from stdin
            }
            path if !path.starts_with("--") => {
                text_path = Some(PathBuf::from(path));
            }
            _ => {}
        }
        i += 1;
    }

    // Load text
    let text = match text_path {
        Some(p) => {
            std::fs::read_to_string(&p).unwrap_or_else(|e| {
                eprintln!("Error reading {}: {}", p.display(), e);
                std::process::exit(1);
            })
        }
        None => {
            // Read from stdin
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf).expect("read stdin");
            buf
        }
    };

    if text.trim().is_empty() {
        eprintln!("Error: empty input text");
        std::process::exit(1);
    }

    // Build engine
    eprintln!("[reasoning_cli] Loading weights from: {}", weights_path.display());
    let engine = ReasoningEngine::new(&weights_path);
    eprintln!(
        "[reasoning_cli] Weights loaded (version={}, architecture={})",
        engine.weights_file().version,
        engine.weights_file().architecture
    );

    // Run analysis
    eprintln!("[reasoning_cli] Analyzing {} chars of text (kappa={})...", text.chars().count(), kappa);
    let report = engine.analyze(&text, kappa);

    if json_output {
        let json = serde_json::to_string_pretty(&report).expect("serialize report");
        println!("{}", json);
    } else {
        print_human_readable(&report, &mut std::io::stdout());
    }
}

fn print_human_readable(report: &ReasoningReport, out: &mut impl Write) {
    let _ = writeln!(out, "═══════════════════════════════════════════════════════════════");
    let _ = writeln!(out, "  REASONING ENGINE REPORT  (weights v{}, {})", 
        report.weights_version, report.weights_architecture);
    let _ = writeln!(out, "═══════════════════════════════════════════════════════════════");
    let _ = writeln!(out, "Input: {} chars", report.text_length);
    let _ = writeln!(out);

    // Characters
    let _ = writeln!(out, "─── CHARACTERS ({} total: {} approved, {} rejected, {} review) ───",
        report.total_characters, report.approved_count, report.rejected_count, report.review_count);
    let _ = writeln!(out, "{:<4} {:<20} {:<8} {:<10} {:<10} {:<8}",
        "#", "name", "raw", "refined", "decision", "script");
    for (i, c) in report.characters.iter().enumerate() {
        let _ = writeln!(out, "{:<4} {:<20} {:<8.3} {:<10.3} {:<10} {:<8}",
            i,
            truncate_str(&c.parsed.name, 20),
            c.raw_confidence,
            c.refined_confidence,
            c.decision.as_str(),
            format!("{:?}", c.script).to_lowercase(),
        );
    }
    let _ = writeln!(out);

    // Triplets
    let _ = writeln!(out, "─── SVO TRIPLETS ({} total: {} valid cases, {} invalid) ───",
        report.total_triplets, report.triplets_valid_cases, report.triplets_invalid_cases);
    for (i, t) in report.triplets.iter().enumerate().take(20) {
        let target = t.triplet.target.as_deref().unwrap_or("—");
        let pol = if t.triplet.polarity { "+" } else { "-" };
        let verdict = t.case_validation.overall.as_str();
        let _ = writeln!(out, "{:<3} [{}] {} {} {} (conf={:.2}, case={})",
            i, pol, truncate_str(&t.triplet.actor, 12),
            truncate_str(&t.triplet.verb, 12), truncate_str(target, 12),
            t.triplet.confidence, verdict);
    }
    if report.total_triplets > 20 {
        let _ = writeln!(out, "... ({} more)", report.total_triplets - 20);
    }
    let _ = writeln!(out);

    // POLER ε
    let _ = writeln!(out, "─── POLER ε_CLIMAX ───");
    let _ = writeln!(out, "  ε          = {:.4}", report.epsilon.epsilon);
    let _ = writeln!(out, "  normalized = {:.2}", report.epsilon.normalized);
    let _ = writeln!(out, "  is_climax  = {} (threshold 7.5)", report.epsilon.is_climax);
    let _ = writeln!(out, "  is_noise   = {} (θ_rel = {:.2})", report.epsilon.is_noise, report.epsilon.theta_rel);
    let _ = writeln!(out);

    // Conflict
    let _ = writeln!(out, "─── CONFLICT (Layer E) ───");
    let _ = writeln!(out, "  Ω_conf (||A||_F) = {:.4}", report.conflict.omega_conf);
    let _ = writeln!(out, "  ρ(A)            = {:.4}", report.conflict.spectral_radius);
    let _ = writeln!(out, "  nodes / edges   = {} / {}", report.conflict.node_count, report.conflict.edge_count);
    let _ = writeln!(out, "  paradoxes       = {}", report.conflict.paradoxes.len());
    let _ = writeln!(out);

    // Diagnostics
    let _ = writeln!(out, "─── DIAGNOSTICS ───");
    let _ = writeln!(out, "  Overall health: {}", report.diagnostics.overall_health);
    let _ = writeln!(out);
    let _ = writeln!(out, "  Class imbalance:");
    let _ = writeln!(out, "    approve:reject = {:.1}:1 ({} / {})",
        report.diagnostics.class_imbalance.approve_reject_ratio,
        report.diagnostics.class_imbalance.approve_count,
        report.diagnostics.class_imbalance.reject_count);
    let _ = writeln!(out, "    imbalanced     = {}", report.diagnostics.class_imbalance.is_imbalanced);
    let _ = writeln!(out);
    let _ = writeln!(out, "  Score distribution:");
    let _ = writeln!(out, "    mean ± std    = {:.3} ± {:.3}",
        report.diagnostics.score_distribution.mean, report.diagnostics.score_distribution.std);
    let _ = writeln!(out, "    approve mean  = {:.3}", report.diagnostics.score_distribution.approve_mean);
    let _ = writeln!(out, "    reject mean   = {:.3}", report.diagnostics.score_distribution.reject_mean);
    let _ = writeln!(out, "    separation    = {:.3} ({}underfitting detected)",
        report.diagnostics.score_distribution.separation,
        if report.diagnostics.score_distribution.underfitting_detected { "" } else { "no " });
    let _ = writeln!(out);
    let _ = writeln!(out, "  Script analysis:");
    let _ = writeln!(out, "    cyrillic / latin / mixed = {} / {} / {}",
        report.diagnostics.script_analysis.cyrillic_count,
        report.diagnostics.script_analysis.latin_count,
        report.diagnostics.script_analysis.mixed_count);
    let _ = writeln!(out, "    parallel-text pollution  = {}",
        report.diagnostics.script_analysis.parallel_text_detected);
    let _ = writeln!(out);
    let _ = writeln!(out, "  Feature informativeness:");
    let _ = writeln!(out, "    low-info features = {:?}",
        report.diagnostics.feature_informativeness.low_information_features);
    let _ = writeln!(out);
    let _ = writeln!(out, "  Weight magnitude:");
    let _ = writeln!(out, "    fc1 std = {:.4} (collapse={})",
        report.diagnostics.weight_magnitude.fc1_weight_std,
        report.diagnostics.weight_magnitude.collapse_detected);
    let _ = writeln!(out, "    fc1 max = {:.4} (explosion={})",
        report.diagnostics.weight_magnitude.fc1_weight_max.abs(),
        report.diagnostics.weight_magnitude.explosion_detected);
    let _ = writeln!(out);

    if !report.diagnostics.recommendations.is_empty() {
        let _ = writeln!(out, "─── RECOMMENDATIONS ───");
        for (i, rec) in report.diagnostics.recommendations.iter().enumerate() {
            let _ = writeln!(out, "  [{}] {}", i + 1, rec);
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "═══════════════════════════════════════════════════════════════");
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        s
    } else {
        let mut end = 0;
        for (i, _) in s.char_indices().take(max) {
            end = i;
        }
        &s[..end]
    }
}

fn print_usage(prog: &str) {
    eprintln!("Usage: {} [OPTIONS] [TEXT_FILE|-]", prog);
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -w, --weights <PATH>  Path to weights.json (default: data/scorer_weights.json)");
    eprintln!("  -k, --kappa <FLOAT>   Sector coefficient (default: 1.0)");
    eprintln!("      --json            Output full report as JSON");
    eprintln!("  -h, --help            Show this help");
    eprintln!();
    eprintln!("If TEXT_FILE is '-' or omitted, reads from stdin.");
}
