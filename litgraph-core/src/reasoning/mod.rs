//! Layer E: Reasoning & Narrative Conflict Analysis.
//!
//! This module bridges local sentence-level analysis (Layers A–D, which produce
//! SVO triplets and per-fragment ε) with global narrative analysis (Layer E),
//! which builds a character interaction graph, computes the conflict magnitude
//! Ω_conf, and detects temporal paradoxes.
//!
//! ## Architecture (Dependency Inversion Principle)
//!
//! Layer D (`parser::epsilon::compute_epsilon_climax`) needs Ω_conf to compute
//! the climax formula. Rather than hard-coupling Layer D to a specific
//! implementation, we define a [`ConflictAnalyzer`] trait that Layer E
//! implementations must satisfy. Layer D accepts any `&impl ConflictAnalyzer`,
//! enabling:
//!
//! - Stub implementations for testing (returns synthetic Ω_conf).
//! - Real implementations backed by [`narrative_graph::NarrativeGraph`].
//! - Future LLM-backed implementations (Layer G).
//!
//! ```text
//!     ┌───────────────────────┐         ┌────────────────────────┐
//!     │   Layer D: ε_climax   │ uses    │   Trait:               │
//!     │   compute_epsilon_    │◄────────┤   ConflictAnalyzer     │
//!     │   climax(text, κ, A)  │         │   fn omega_conf(...)   │
//!     └───────────────────────┘         └─────────┬──────────────┘
//!                                                 │ implemented by
//!                                                 ▼
//!                                   ┌─────────────────────────────┐
//!                                   │  NarrativeGraph (Layer E)   │
//!                                   │  - builds A_POS adjacency   │
//!                                   │  - computes ρ(A_POS)        │
//!                                   │  - computes ||A_POS||_F     │
//!                                   │  - detects paradoxes        │
//!                                   └─────────────────────────────┘
//! ```

pub mod narrative_graph;
pub mod paradox;
pub mod stub;

pub use narrative_graph::NarrativeGraph;
pub use paradox::ParadoxDetector;
pub use stub::StubConflictAnalyzer;

use std::collections::HashMap;

use crate::linguistic::svo_parser::SvoTriplet;
use crate::parser::characters::ParsedCharacter;

/// Result of conflict analysis for a single text fragment (chapter/scene).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConflictReport {
    /// Conflict magnitude Ω_conf = ||A_POS||_F (Frobenius norm of the
    /// POS-filtered character adjacency matrix). Drives the
    /// `λ_conf · Ω_conf` term in the ε_climax formula.
    pub omega_conf: f64,
    /// Spectral radius ρ(A_POS) — largest absolute eigenvalue of the
    /// character adjacency matrix. Higher ρ ⇒ denser conflict web.
    pub spectral_radius: f64,
    /// Number of nodes (characters) in the conflict graph.
    pub node_count: usize,
    /// Number of directed edges (character → character interactions).
    pub edge_count: usize,
    /// Detected temporal paradoxes (Dead-Speaking, Teleportation).
    pub paradoxes: Vec<paradox::Paradox>,
}

impl Default for ConflictReport {
    fn default() -> Self {
        Self {
            omega_conf: 0.0,
            spectral_radius: 0.0,
            node_count: 0,
            edge_count: 0,
            paradoxes: Vec::new(),
        }
    }
}

/// Input bundle for conflict analysis: a slice of chapters, each chapter's
/// detected characters, and each chapter's extracted SVO triplets.
#[derive(Debug, Clone)]
pub struct ManuscriptAnalysis<'a> {
    /// Chapter text (full text).
    pub chapters: Vec<&'a str>,
    /// Characters detected per chapter (index-aligned with `chapters`).
    pub characters_per_chapter: Vec<Vec<ParsedCharacter>>,
    /// SVO triplets extracted per chapter (index-aligned with `chapters`).
    pub triplets_per_chapter: Vec<Vec<SvoTriplet>>,
}

/// Conflict analyzer trait (Layer E interface).
///
/// Implementations produce a [`ConflictReport`] for the whole manuscript
/// (or a single chapter), which Layer D consumes via
/// `compute_epsilon_climax_with_analyzer`.
///
/// ## Contract
/// - **Deterministic**: same input ⇒ same `omega_conf` (Symbolic AI principle).
/// - **Pure**: no I/O, no global mutable state. Implementations may load
///   read-only linguistic resources (e.g., lexicons) but must not mutate them.
/// - **Bounded**: `omega_conf ∈ [0.0, +∞)`. Typical range: `[0.0, 100.0]`.
///   `0.0` means "no conflict detected" (single-character chapter, no paradoxes).
pub trait ConflictAnalyzer {
    /// Analyze the whole manuscript and produce a single conflict report.
    fn analyze(&self, manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport;

    /// Convenience: return only the Ω_conf magnitude for the whole manuscript.
    /// Default implementation delegates to [`analyze`](Self::analyze).
    fn omega_conf(&self, manuscript: &ManuscriptAnalysis<'_>) -> f64 {
        self.analyze(manuscript).omega_conf
    }

    /// Analyze a single chapter (by index) and produce its conflict report.
    /// Useful for per-chapter ε_climax computation.
    /// Default implementation builds a single-chapter [`ManuscriptAnalysis`]
    /// and delegates to [`analyze`](Self::analyze).
    fn analyze_chapter(
        &self,
        chapter_text: &str,
        characters: Vec<ParsedCharacter>,
        triplets: Vec<SvoTriplet>,
    ) -> ConflictReport {
        let manuscript = ManuscriptAnalysis {
            chapters: vec![chapter_text],
            characters_per_chapter: vec![characters],
            triplets_per_chapter: vec![triplets],
        };
        self.analyze(&manuscript)
    }
}

/// Compute the Frobenius norm of a symmetric dense matrix (row-major).
///
/// `||A||_F = √(Σ_i Σ_j |a_ij|²)`
///
/// Used as Ω_conf: the conflict magnitude of the character adjacency matrix.
pub fn frobenius_norm(matrix: &[Vec<f64>]) -> f64 {
    let mut sum_sq = 0.0_f64;
    for row in matrix {
        for &v in row {
            sum_sq += v * v;
        }
    }
    sum_sq.sqrt()
}

/// Compute the spectral radius ρ(A) via power iteration.
///
/// For a non-negative square matrix A, the spectral radius equals the largest
/// eigenvalue (Perron–Frobenius theorem). Power iteration converges to this
/// eigenvalue for non-negative matrices.
///
/// ## Algorithm
/// 1. Start with `v₀ = (1, 1, …, 1)ᵀ / √n` (uniform vector).
/// 2. Iterate: `v_{k+1} = A·v_k / ||A·v_k||`.
/// 3. Eigenvalue estimate: `λ_k = v_kᵀ · A · v_k` (Rayleigh quotient).
/// 4. Stop when `|λ_k − λ_{k−1}| < tol` or `k ≥ max_iter`.
///
/// ## Returns
/// - `0.0` for empty or non-square matrices.
/// - Otherwise, the largest eigenvalue magnitude (≥ 0).
pub fn spectral_radius_power_iteration(
    matrix: &[Vec<f64>],
    max_iter: usize,
    tol: f64,
) -> f64 {
    let n = matrix.len();
    if n == 0 {
        return 0.0;
    }
    // Validate square matrix
    for row in matrix {
        if row.len() != n {
            return 0.0;
        }
    }
    // Initialize v = (1, 1, ..., 1) / √n
    let mut v = vec![1.0_f64 / (n as f64).sqrt(); n];
    let mut lambda_prev = 0.0_f64;
    for _ in 0..max_iter {
        // w = A · v
        let mut w = vec![0.0_f64; n];
        for (i, row) in matrix.iter().enumerate() {
            for (j, &a_ij) in row.iter().enumerate() {
                w[i] += a_ij * v[j];
            }
        }
        // norm_w = ||w||
        let norm_w = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm_w < 1e-12 {
            return 0.0;
        }
        // v_new = w / ||w||
        let v_new: Vec<f64> = w.iter().map(|x| x / norm_w).collect();
        // λ = v_newᵀ · A · v_new (Rayleigh quotient)
        let mut av = vec![0.0_f64; n];
        for (i, row) in matrix.iter().enumerate() {
            for (j, &a_ij) in row.iter().enumerate() {
                av[i] += a_ij * v_new[j];
            }
        }
        let lambda: f64 = v_new.iter().zip(av.iter()).map(|(a, b)| a * b).sum();
        if (lambda - lambda_prev).abs() < tol {
            return lambda.max(0.0);
        }
        lambda_prev = lambda;
        v = v_new;
    }
    lambda_prev.max(0.0)
}

/// Build a node-name → index map for a list of character names.
pub fn build_node_index(character_names: &[String]) -> HashMap<String, usize> {
    character_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frobenius_norm_zero_matrix() {
        let m: Vec<Vec<f64>> = vec![];
        assert_eq!(frobenius_norm(&m), 0.0);
    }

    #[test]
    fn test_frobenius_norm_identity_3x3() {
        // ||I_3||_F = √3
        let m = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let got = frobenius_norm(&m);
        assert!((got - 3.0_f64.sqrt()).abs() < 1e-9, "got {}", got);
    }

    #[test]
    fn test_frobenius_norm_known_matrix() {
        // A = [[1, 2], [3, 4]] → ||A||_F = √(1+4+9+16) = √30 ≈ 5.4772
        let m = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let got = frobenius_norm(&m);
        assert!((got - 30.0_f64.sqrt()).abs() < 1e-9, "got {}", got);
    }

    #[test]
    fn test_spectral_radius_empty() {
        let m: Vec<Vec<f64>> = vec![];
        assert_eq!(spectral_radius_power_iteration(&m, 100, 1e-9), 0.0);
    }

    #[test]
    fn test_spectral_radius_non_square() {
        let m = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        assert_eq!(spectral_radius_power_iteration(&m, 100, 1e-9), 0.0);
    }

    #[test]
    fn test_spectral_radius_identity_n() {
        // ρ(I_n) = 1
        for n in [2, 3, 5, 10] {
            let m: Vec<Vec<f64>> = (0..n)
                .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
                .collect();
            let got = spectral_radius_power_iteration(&m, 500, 1e-10);
            assert!((got - 1.0).abs() < 1e-6, "n={}, ρ(I)={}, expected 1.0", n, got);
        }
    }

    #[test]
    fn test_spectral_radius_known_eigenvalue() {
        // A = [[0, 1], [1, 0]] has eigenvalues ±1, so ρ = 1.
        let m = vec![vec![0.0, 1.0], vec![1.0, 0.0]];
        let got = spectral_radius_power_iteration(&m, 500, 1e-10);
        assert!((got - 1.0).abs() < 1e-6, "got {}", got);
    }

    #[test]
    fn test_spectral_radius_complete_graph() {
        // Complete graph K_n adjacency (zero diagonal, ones elsewhere).
        // Eigenvalues: n-1 (multiplicity 1), -1 (multiplicity n-1).
        // So ρ(K_n) = n - 1.
        for n in [3, 4, 5] {
            let m: Vec<Vec<f64>> = (0..n)
                .map(|i| (0..n).map(|j| if i == j { 0.0 } else { 1.0 }).collect())
                .collect();
            let got = spectral_radius_power_iteration(&m, 1000, 1e-12);
            assert!((got - (n - 1) as f64).abs() < 1e-4, "n={}, ρ(K)={}, expected {}", n, got, n - 1);
        }
    }

    #[test]
    fn test_build_node_index_round_trip() {
        let names = vec!["Марта".to_string(), "Петро".to_string(), "Веня".to_string()];
        let idx = build_node_index(&names);
        assert_eq!(idx.len(), 3);
        assert_eq!(idx["Марта"], 0);
        assert_eq!(idx["Петро"], 1);
        assert_eq!(idx["Веня"], 2);
    }
}
