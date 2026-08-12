//! Narrative Graph: builds the POS-filtered character adjacency matrix `A_POS`
//! from SVO triplets and computes the conflict magnitude Ω_conf.
//!
//! ## Pipeline
//!
//! 1. **Node set**: union of all character names detected across all chapters
//!    (Layer B POS-tagger guarantees no homonym pollution — "Мати" the noun is
//!    distinct from "мати" the verb, so only true character names enter the graph).
//! 2. **Edge construction**: for each SVO triplet `(actor, verb, target, …)`,
//!    if `actor ∈ Characters` AND `target ∈ Characters`, add a directed edge
//!    `actor → target` with weight `+confidence`. Negated actions
//!    (`polarity == false`) still count as interactions (the conflict exists
//!    even if the action didn't happen — see Layer D spec for ε_climax).
//! 3. **Aggregation**: build symmetric matrix `A_POS[i][j] = Σ_confidences`
//!    for all edges `i → j` OR `j → i` (undirected conflict graph).
//! 4. **Ω_conf = ||A_POS||_F** (Frobenius norm).
//! 5. **ρ(A_POS)** = largest eigenvalue (via power iteration).
//!
//! ## Why POS-filtered?
//!
//! Without Layer B, "Мати" (noun) and "мати" (verb infinitive) collide, and
//! false co-occurrence links inflate the adjacency matrix. The POS filter
//! eliminates these by only including disambiguated character names. Per
//! `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md` §3.2, this reduces ρ(A) by ~4.16%.

use std::collections::{HashMap, HashSet};

use petgraph::graph::{DiGraph, NodeIndex};

use super::{ConflictAnalyzer, ConflictReport, ManuscriptAnalysis};
use crate::linguistic::svo_parser::SvoTriplet;
use crate::parser::characters::{EntityType, ParsedCharacter};

/// Default power-iteration parameters for spectral radius computation.
const DEFAULT_MAX_ITER: usize = 1000;
const DEFAULT_TOL: f64 = 1e-9;

/// Narrative graph conflict analyzer backed by `petgraph::DiGraph`.
///
/// Builds a POS-filtered character adjacency matrix from SVO triplets and
/// computes Ω_conf = ||A_POS||_F (Frobenius norm) plus ρ(A_POS) (spectral
/// radius via power iteration).
#[derive(Debug, Default)]
pub struct NarrativeGraph {
    /// Last-built graph (cached for inspection / visualization).
    graph: DiGraph<String, f64>,
    /// Node name → NodeIndex map.
    node_map: HashMap<String, NodeIndex>,
}

impl NarrativeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the conflict graph from manuscript analysis.
    ///
    /// After this call, `self.graph` and `self.node_map` are populated and
    /// can be inspected via [`graph`](Self::graph) and [`node_map`](Self::node_map).
    pub fn build(&mut self, manuscript: &ManuscriptAnalysis<'_>) {
        self.graph.clear();
        self.node_map.clear();

        // === Phase 1: collect character name set across all chapters ===
        // Only canonical names are nodes. Aliases are resolved via the
        // alias→canonical map in Phase 2 (no separate nodes for aliases).
        let mut character_names: HashSet<String> = HashSet::new();
        for chars_in_ch in &manuscript.characters_per_chapter {
            for c in chars_in_ch {
                // Only characters (not concepts or organizations) enter the
                // conflict graph. Organizations have no individual agency.
                if c.entity_type == EntityType::Character {
                    character_names.insert(c.name.clone());
                }
            }
        }

        // Add nodes
        for name in &character_names {
            let idx = self.graph.add_node(name.clone());
            self.node_map.insert(name.clone(), idx);
        }

        // === Phase 2: add edges from SVO triplets ===
        // Build alias→canonical map for resolving actor/target to character names.
        let mut alias_to_canonical: HashMap<String, String> = HashMap::new();
        for chars_in_ch in &manuscript.characters_per_chapter {
            for c in chars_in_ch {
                if c.entity_type != EntityType::Character {
                    continue;
                }
                alias_to_canonical.insert(c.name.to_lowercase(), c.name.clone());
                for alias in &c.aliases {
                    alias_to_canonical.insert(alias.to_lowercase(), c.name.clone());
                }
            }
        }

        for triplets in &manuscript.triplets_per_chapter {
            for t in triplets {
                self.add_triplet_edge(t, &alias_to_canonical);
            }
        }
    }

    /// Try to add an edge from a single SVO triplet.
    /// Both actor and target must resolve to a known character.
    fn add_triplet_edge(
        &mut self,
        triplet: &SvoTriplet,
        alias_to_canonical: &HashMap<String, String>,
    ) {
        let actor_canonical = alias_to_canonical.get(&triplet.actor.to_lowercase());
        let target_canonical = triplet
            .target
            .as_ref()
            .and_then(|t| alias_to_canonical.get(&t.to_lowercase()));

        let (Some(actor_name), Some(target_name)) = (actor_canonical, target_canonical) else {
            return; // Not a character-to-character interaction
        };
        if actor_name == target_name {
            return; // No self-loops in conflict graph
        }
        let Some(&actor_idx) = self.node_map.get(actor_name) else {
            return;
        };
        let Some(&target_idx) = self.node_map.get(target_name) else {
            return;
        };
        // Aggregate weight: add confidence to existing edge (or create new).
        if let Some(edge_idx) = self.graph.find_edge(actor_idx, target_idx) {
            let w = self.graph.edge_weight_mut(edge_idx).unwrap();
            *w += triplet.confidence;
        } else {
            self.graph.add_edge(actor_idx, target_idx, triplet.confidence);
        }
    }

    /// Build a symmetric dense adjacency matrix A_POS from the directed graph.
    ///
    /// For conflict analysis we treat interactions as undirected: an edge
    /// `i → j` and `j → i` both contribute to `A[i][j]` and `A[j][i]`.
    /// The result is a symmetric matrix where `A[i][j] = A[j][i]` = total
    /// interaction weight between i and j.
    pub fn adjacency_matrix(&self) -> (Vec<String>, Vec<Vec<f64>>) {
        let n = self.graph.node_count();
        let mut names: Vec<String> = Vec::with_capacity(n);
        let mut idx_to_name: Vec<String> = Vec::with_capacity(n);
        // Build stable ordering: NodeIndex 0..n
        for i in 0..n {
            let node_idx = NodeIndex::new(i);
            let name = self.graph.node_weight(node_idx).cloned().unwrap_or_default();
            idx_to_name.push(name.clone());
            names.push(name);
        }
        let mut matrix = vec![vec![0.0_f64; n]; n];
        for edge in self.graph.raw_edges() {
            let i = edge.source().index();
            let j = edge.target().index();
            let w = edge.weight;
            // Symmetric aggregation
            matrix[i][j] += w;
            matrix[j][i] += w;
        }
        (names, matrix)
    }

    /// Access the underlying directed graph (for visualization / debugging).
    pub fn graph(&self) -> &DiGraph<String, f64> {
        &self.graph
    }

    /// Access the node-name → NodeIndex map.
    pub fn node_map(&self) -> &HashMap<String, NodeIndex> {
        &self.node_map
    }

    /// Count directed edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Count nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}

impl ConflictAnalyzer for NarrativeGraph {
    fn analyze(&self, manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport {
        // Build a temporary graph (we don't mutate self in analyze — pure function).
        let mut ng = NarrativeGraph::new();
        ng.build(manuscript);

        let (_, matrix) = ng.adjacency_matrix();
        let omega_conf = super::frobenius_norm(&matrix);
        let spectral_radius =
            super::spectral_radius_power_iteration(&matrix, DEFAULT_MAX_ITER, DEFAULT_TOL);

        ConflictReport {
            omega_conf,
            spectral_radius,
            node_count: ng.node_count(),
            edge_count: ng.edge_count(),
            paradoxes: Vec::new(), // Populated by `paradox::ParadoxDetector` (separate pass).
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linguistic::svo_parser::{SvoParser, SvoTriplet};
    use crate::parser::characters::EntityType;

    fn make_character(name: &str, aliases: &[&str]) -> ParsedCharacter {
        use crate::parser::characters::{SIGNAL_CAPITALIZED, SIGNAL_SPEECH_VERB};
        // Test helper: 2 signals (cap + speech) → confidence 0.7 (single-token)
        let signals = SIGNAL_CAPITALIZED | SIGNAL_SPEECH_VERB;
        ParsedCharacter {
            name: name.to_string(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            count: 1,
            description: String::new(),
            speech_count: 1,
            direct_count: 0,
            reason: "test".to_string(),
            entity_type: EntityType::Character,
            evidence_signals: signals,
            confidence: ParsedCharacter::confidence_from_signals(signals, true),
            mention_starts: vec![0],
            first_mention: Some(0),
            nominative_count: 0,
            accusative_count: 0,
            genitive_negated_count: 0,
        }
    }

    fn make_triplet(actor: &str, verb: &str, target: Option<&str>, conf: f64, polarity: bool) -> SvoTriplet {
        SvoTriplet {
            actor: actor.to_string(),
            verb: verb.to_string(),
            target: target.map(|s| s.to_string()),
            instrument: None,
            location: None,
            polarity,
            confidence: conf,
        }
    }

    #[test]
    fn test_empty_manuscript_produces_empty_graph() {
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec![],
            characters_per_chapter: vec![],
            triplets_per_chapter: vec![],
        });
        assert_eq!(report.omega_conf, 0.0);
        assert_eq!(report.spectral_radius, 0.0);
        assert_eq!(report.node_count, 0);
        assert_eq!(report.edge_count, 0);
    }

    #[test]
    fn test_single_character_no_conflict() {
        // One character with a self-action — no edges, no conflict.
        let chars = vec![make_character("Марта", &[])];
        let triplets = vec![make_triplet("Марта", "йти", None, 0.9, true)];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Марта йде."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 1, "Single character node");
        assert_eq!(report.edge_count, 0, "No edges (self-action skipped)");
        assert_eq!(report.omega_conf, 0.0, "No conflict");
        assert_eq!(report.spectral_radius, 0.0);
    }

    #[test]
    fn test_two_character_interaction_produces_edge() {
        // "Петро вбив ворога" — both are characters.
        let chars = vec![
            make_character("Петро", &[]),
            make_character("ворог", &["ворога"]),
        ];
        let triplets = vec![make_triplet("Петро", "вбити", Some("ворога"), 1.0, true)];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро вбив ворога."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 2);
        assert_eq!(report.edge_count, 1, "One directed edge Петро → ворог");
        // Matrix: [[0, 1], [1, 0]] → ||A||_F = √2, ρ = 1
        let expected_omega = 2.0_f64.sqrt();
        assert!((report.omega_conf - expected_omega).abs() < 1e-9,
                "Ω_conf={} expected {}", report.omega_conf, expected_omega);
        assert!((report.spectral_radius - 1.0).abs() < 1e-6,
                "ρ(A)={} expected 1.0", report.spectral_radius);
    }

    #[test]
    fn test_three_character_complete_graph() {
        // Three characters all interacting pairwise (K_3 conflict web).
        let chars = vec![
            make_character("Марта", &[]),
            make_character("Петро", &[]),
            make_character("Веня", &[]),
        ];
        let triplets = vec![
            make_triplet("Марта", "вбити", Some("Петро"), 1.0, true),
            make_triplet("Петро", "вбити", Some("Веня"), 1.0, true),
            make_triplet("Веня", "вбити", Some("Марта"), 1.0, true),
        ];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Три герої."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 3);
        // 3 directed edges → symmetric matrix has 6 non-zero entries (off-diagonal)
        // ||A||_F = √6 ≈ 2.449
        let expected_omega = 6.0_f64.sqrt();
        assert!((report.omega_conf - expected_omega).abs() < 1e-9,
                "Ω_conf={} expected {}", report.omega_conf, expected_omega);
        // ρ(K_3 adjacency) = 2 (Perron-Frobenius for complete graph K_n: ρ = n-1)
        assert!((report.spectral_radius - 2.0).abs() < 1e-4,
                "ρ(K_3)={} expected 2.0", report.spectral_radius);
    }

    #[test]
    fn test_negated_actions_still_register_conflict() {
        // "Петро не вбив ворога" — the action didn't happen, but the conflict
        // tension between characters is still present (per ε_climax spec).
        let chars = vec![
            make_character("Петро", &[]),
            make_character("ворог", &["ворога"]),
        ];
        let triplets = vec![make_triplet("Петро", "вбити", Some("ворога"), 0.95, false)];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро не вбив ворога."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.edge_count, 1, "Negated actions still create edges");
        assert!(report.omega_conf > 0.0);
    }

    #[test]
    fn test_non_character_target_does_not_create_edge() {
        // "Петро вбив собаку" — собака is not a character (no character entry).
        let chars = vec![make_character("Петро", &[])];
        let triplets = vec![make_triplet("Петро", "вбити", Some("собаку"), 1.0, true)];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро вбив собаку."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 1, "Only Петро is a character");
        assert_eq!(report.edge_count, 0, "собака is not a character — no edge");
        assert_eq!(report.omega_conf, 0.0);
    }

    #[test]
    fn test_alias_resolution() {
        // Actor "Петро" but target "ворога" (accusative form) — must resolve
        // via alias map to canonical "ворог".
        let chars = vec![
            make_character("Петро", &[]),
            make_character("ворог", &["ворога", "ворога"]),
        ];
        let triplets = vec![make_triplet("Петро", "вбити", Some("ворога"), 1.0, true)];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро вбив ворога."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 2, "ворога → ворог resolved via alias");
        assert_eq!(report.edge_count, 1);
    }

    #[test]
    fn test_concept_entity_excluded_from_graph() {
        // "Бездна" appears as a Concept (no speech_count) — should NOT be in graph.
        let chars = vec![
            make_character("Петро", &[]),
            ParsedCharacter {
                name: "Бездна".to_string(),
                aliases: vec![],
                count: 5,
                description: String::new(),
                speech_count: 0,
                direct_count: 0,
                reason: "concept".to_string(),
                entity_type: EntityType::Concept,
                evidence_signals: crate::parser::characters::SIGNAL_CAPITALIZED,
                confidence: 0.3,
                mention_starts: vec![],
                first_mention: None,
                nominative_count: 0,
                accusative_count: 0,
                genitive_negated_count: 0,
            },
        ];
        let triplets = vec![make_triplet("Петро", "бачити", Some("Бездна"), 0.9, true)];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро бачив Бездну."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 1, "Concept excluded — only Петро remains");
        assert_eq!(report.edge_count, 0);
    }

    #[test]
    fn test_weight_aggregation_multiple_interactions() {
        // Two separate triplets Петро→ворог: weights should add.
        let chars = vec![
            make_character("Петро", &[]),
            make_character("ворог", &["ворога"]),
        ];
        let triplets = vec![
            make_triplet("Петро", "вбити", Some("ворога"), 1.0, true),
            make_triplet("Петро", "поранити", Some("ворога"), 0.8, true),
        ];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро двічі атакував ворога."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        // Matrix: [[0, 1.8], [1.8, 0]] → ||A||_F = √(2·1.8²) = 1.8·√2
        let expected_omega = 1.8 * 2.0_f64.sqrt();
        assert!((report.omega_conf - expected_omega).abs() < 1e-9,
                "Ω_conf={} expected {}", report.omega_conf, expected_omega);
    }

    #[test]
    fn test_determinism_same_input_same_output() {
        let chars = vec![
            make_character("Марта", &[]),
            make_character("Петро", &[]),
        ];
        let triplets = vec![make_triplet("Марта", "вбити", Some("Петро"), 1.0, true)];
        let manuscript = ManuscriptAnalysis {
            chapters: vec!["Марта вбила Петра."],
            characters_per_chapter: vec![chars.clone()],
            triplets_per_chapter: vec![triplets.clone()],
        };
        let ng = NarrativeGraph::new();
        let r1 = ng.analyze(&manuscript);
        let r2 = ng.analyze(&manuscript);
        assert_eq!(r1, r2, "Determinism: same input must yield identical report");
    }

    #[test]
    fn test_svo_parser_integration_with_narrative_graph() {
        // Integration: use the real SvoParser to extract triplets from text,
        // then feed them to NarrativeGraph.
        let parser = SvoParser::new();
        let triplets = parser.parse_text("Петро вбив ворога.");
        assert!(!triplets.is_empty(), "SvoParser should extract at least 1 triplet");

        let chars = vec![
            make_character("Петро", &[]),
            make_character("ворог", &["ворога"]),
        ];
        let ng = NarrativeGraph::new();
        let report = ng.analyze(&ManuscriptAnalysis {
            chapters: vec!["Петро вбив ворога."],
            characters_per_chapter: vec![chars],
            triplets_per_chapter: vec![triplets],
        });
        assert_eq!(report.node_count, 2);
        assert!(report.omega_conf > 0.0, "Real SVO pipeline must produce conflict");
    }
}
