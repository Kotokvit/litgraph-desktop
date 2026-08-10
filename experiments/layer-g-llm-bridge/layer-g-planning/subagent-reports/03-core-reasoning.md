# Subagent 03: litgraph-core Reasoning Layer (NarrativeGraph + ParadoxDetector + ConflictAnalyzer)

> **Scope owner**: Layer E — Reasoning & Narrative Conflict Analysis (POS-filtered character adjacency matrix `A_POS`, Ω_conf Frobenius norm, ρ(A_POS) spectral radius, temporal paradox detection).
> **Location**: `/home/z/my-project/litgraph-desktop/litgraph-core/src/reasoning/`
> **Re-export bridge**: `src-tauri/src/poler/mod.rs` re-exports these symbols under `crate::poler::` for use by `src-tauri/src/commands/poler.rs`.

---

## 1. Scope
- **Files inspected**: 4
  - `litgraph-core/src/reasoning/mod.rs` (307 LOC)
  - `litgraph-core/src/reasoning/narrative_graph.rs` (470 LOC)
  - `litgraph-core/src/reasoning/paradox.rs` (346 LOC)
  - `litgraph-core/src/reasoning/stub.rs` (130 LOC)
- **Total LOC**: 1253 (incl. ~370 LOC of `#[cfg(test)]` modules)
- **Production LOC** (excluding tests): ~883
- **Key entry points**:
  - `mod.rs:104` — `pub trait ConflictAnalyzer` (DI interface consumed by Layer D `parser::epsilon::compute_epsilon_climax_with_analyzer`)
  - `narrative_graph.rs:44` — `pub struct NarrativeGraph` (canonical Layer E implementation)
  - `paradox.rs:95` — `pub struct ParadoxDetector` (temporal paradox scanner)
  - `stub.rs:31` — `pub struct StubConflictAnalyzer` (testing placeholder)
- **Dependents outside this scope**:
  - `litgraph-core/src/parser/epsilon.rs:48` — `use crate::reasoning::{ConflictAnalyzer, ConflictReport};`
  - `src-tauri/src/poler/mod.rs:27-30` — `pub use litgraph_core::reasoning::{ConflictAnalyzer, ConflictReport, ManuscriptAnalysis, NarrativeGraph, ParadoxDetector, Paradox, ParadoxKind};`
  - `src-tauri/src/commands/poler.rs:41-45` — `use crate::poler::{… NarrativeGraph, ParadoxDetector, Paradox, …};`

---

## 2. Atomic Inventory

### 2.1 Modules / Files

| File | LOC | Purpose | Public API | Dependencies |
|------|-----|---------|------------|--------------|
| `litgraph-core/src/reasoning/mod.rs` | 307 | Module root. Defines `ConflictReport`, `ManuscriptAnalysis<'a>`, `ConflictAnalyzer` trait, free functions `frobenius_norm`, `spectral_radius_power_iteration`, `build_node_index`. Re-exports `NarrativeGraph`, `Paradox`, `ParadoxDetector`, `ParadoxKind`, `StubConflictAnalyzer`. | `ConflictReport`, `ManuscriptAnalysis`, `ConflictAnalyzer` (trait), `frobenius_norm`, `spectral_radius_power_iteration`, `build_node_index` | `std::collections::HashMap`, `crate::linguistic::svo_parser::SvoTriplet`, `crate::parser::characters::ParsedCharacter`, `serde`, submodules |
| `litgraph-core/src/reasoning/narrative_graph.rs` | 470 | Canonical Layer E analyzer. Builds a `petgraph::DiGraph<String, f64>` of character→character interactions from SVO triplets, constructs a symmetric adjacency matrix `A_POS`, computes Ω_conf = ‖A_POS‖_F and ρ(A_POS) via power iteration. | `NarrativeGraph` (struct + `ConflictAnalyzer` impl) | `petgraph::graph::{DiGraph, NodeIndex}`, `std::collections::{HashMap, HashSet}`, `super::{ConflictAnalyzer, ConflictReport, ManuscriptAnalysis}`, `crate::linguistic::svo_parser::SvoTriplet`, `crate::parser::characters::{EntityType, ParsedCharacter}` |
| `litgraph-core/src/reasoning/paradox.rs` | 346 | Temporal paradox detector. Scans manuscript for death markers (Ukrainian past-tense verbs) and post-death speech/action events. | `ParadoxKind` (enum), `Paradox` (struct), `ParadoxDetector` (struct) | `crate::linguistic::svo_parser::SvoTriplet`, `crate::parser::characters::ParsedCharacter`, `crate::reasoning::ManuscriptAnalysis`, `serde` |
| `litgraph-core/src/reasoning/stub.rs` | 130 | Stub conflict analyzer for unit tests. Counts conflict keywords and returns synthetic Ω_conf = `weight × keyword_count`. | `StubConflictAnalyzer` (struct + `ConflictAnalyzer` impl) | `super::{ConflictAnalyzer, ConflictReport, ManuscriptAnalysis}` |

### 2.2 Public Types / Interfaces

#### `ConflictReport` (`mod.rs:51-66`)
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConflictReport {
    pub omega_conf: f64,        // Frobenius norm ‖A_POS‖_F
    pub spectral_radius: f64,   // ρ(A_POS) via power iteration
    pub node_count: usize,      // # characters in conflict graph
    pub edge_count: usize,      // # directed edges
    pub paradoxes: Vec<paradox::Paradox>,
}
```
- `Default` impl returns all-zeros with empty `paradoxes` (mod.rs:68-78).
- **CRITICAL**: When produced by `NarrativeGraph::analyze`, `paradoxes` is **always empty** (narrative_graph.rs:206 docstring: "Populated by `paradox::ParadoxDetector` (separate pass)"). The caller must invoke `ParadoxDetector::detect` separately and merge.

#### `ManuscriptAnalysis<'a>` (`mod.rs:82-90`)
```rust
#[derive(Debug, Clone)]
pub struct ManuscriptAnalysis<'a> {
    pub chapters: Vec<&'a str>,                              // raw chapter text slices
    pub characters_per_chapter: Vec<Vec<ParsedCharacter>>,   // index-aligned with chapters
    pub triplets_per_chapter: Vec<Vec<SvoTriplet>>,          // index-aligned with chapters
}
```
- **Not `Serialize`** — borrows raw `&str` slices, lifetime-tied to caller's text buffer. Cannot be persisted or sent across Tauri IPC directly; must be rebuilt on each call.
- **Index alignment contract**: `chapters[i]`, `characters_per_chapter[i]`, `triplets_per_chapter[i]` must all be co-indexed. No runtime check enforces this — caller bug = panic on index-out-of-bounds.

#### `ConflictAnalyzer` trait (`mod.rs:104-131`)
```rust
pub trait ConflictAnalyzer {
    fn analyze(&self, manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport;

    fn omega_conf(&self, manuscript: &ManuscriptAnalysis<'_>) -> f64 { /* default: delegates to analyze */ }

    fn analyze_chapter(
        &self,
        chapter_text: &str,
        characters: Vec<ParsedCharacter>,
        triplets: Vec<SvoTriplet>,
    ) -> ConflictReport { /* default: builds single-chapter ManuscriptAnalysis, calls analyze */ }
}
```
- Contract (doc lines 99-103): **Deterministic**, **Pure** (no I/O, no global mutable state), **Bounded** Ω_conf ∈ [0, +∞).
- Used by Layer D `parser::epsilon::compute_epsilon_climax_with_analyzer` (epsilon.rs:528).

#### `NarrativeGraph` (`narrative_graph.rs:43-49`)
```rust
#[derive(Debug, Default)]
pub struct NarrativeGraph {
    graph: DiGraph<String, f64>,           // petgraph directed graph, node weight = canonical name
    node_map: HashMap<String, NodeIndex>,  // name → petgraph NodeIndex
}
```
- Fields are **private** — only accessible via accessor methods.

#### `ParadoxKind` (`paradox.rs:31-38`)
```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParadoxKind {
    DeadSpeaking,           // IMPLEMENTED
    SpatialTeleportation,   // STUBBED — never emitted by detect()
}
```
- Serde JSON form: `"dead_speaking"` or `"spatial_teleportation"`.

#### `Paradox` (`paradox.rs:41-54`)
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Paradox {
    pub kind: ParadoxKind,
    pub character: String,
    pub chapter_idx: usize,         // where paradox manifests
    pub origin_chapter_idx: usize,  // death chapter (DeadSpeaking) or prev location (SpatialTeleportation)
    pub explanation: String,        // human-readable
}
```
- **No `id` field** — Layer G cannot reference paradoxes by ID; must use composite key `(character, chapter_idx, origin_chapter_idx)`.
- **No `evidence_text` field** — Layer G must fetch chapter text snippets separately via `ManuscriptAnalysis.chapters[chapter_idx]`.

#### `ParadoxDetector` (`paradox.rs:94-98`)
```rust
#[derive(Debug, Default)]
pub struct ParadoxDetector {
    deaths: Vec<(String, usize)>,  // (character_name, chapter_idx) pairs; private
}
```
- Mutable across `detect()` calls — `deaths` is cleared and rebuilt each invocation. Not idempotent across multiple manuscripts without re-`new()`.

#### `StubConflictAnalyzer` (`stub.rs:30-34`)
```rust
#[derive(Debug, Default, Clone)]
pub struct StubConflictAnalyzer {
    pub weight: f64,  // default 0.5
}
```

#### Cross-module types referenced (defined elsewhere)
- **`SvoTriplet`** (`litgraph-core/src/linguistic/svo_parser.rs:23-39`): `{ actor: String, verb: String, target: Option<String>, instrument: Option<String>, location: Option<String>, polarity: bool, confidence: f64 }`. The `location` field is currently UNUSED by `NarrativeGraph` and `ParadoxDetector` — relevant for future `SpatialTeleportation` implementation.
- **`ParsedCharacter`** (`litgraph-core/src/parser/characters.rs:31-53`): `{ name, aliases: Vec<String>, count, description, speech_count, direct_count, reason, entity_type: EntityType }`.
- **`EntityType`** (`litgraph-core/src/parser/characters.rs:59-70`): `{ Character, Organization, Concept }` with `#[serde(rename_all = "lowercase")]`. Only `Character` enters the conflict graph (narrative_graph.rs:72).
- **`ParsedChapter`** (`litgraph-core/src/parser/chapters.rs:42-89`): Referenced by `src-tauri/src/commands/poler.rs:402` to build `ManuscriptAnalysis` from `(Vec<ParsedChapter>, String)`. NOT a type defined in the reasoning module.

### 2.3 Public Functions / Commands

#### Free functions in `mod.rs`

| Signature | Line | Purpose |
|-----------|------|---------|
| `pub fn frobenius_norm(matrix: &[Vec<f64>]) -> f64` | 138 | `‖A‖_F = √(Σ a_ij²)`. Empty matrix → 0.0. |
| `pub fn spectral_radius_power_iteration(matrix: &[Vec<f64>], max_iter: usize, tol: f64) -> f64` | 163 | Power iteration + Rayleigh quotient. Empty/non-square → 0.0. Returns `lambda.max(0.0)` (Perron-Frobenius assumption). |
| `pub fn build_node_index(character_names: &[String]) -> HashMap<String, usize>` | 214 | Helper: name → dense matrix index. |

#### Methods on `NarrativeGraph`

| Signature | Line | Visibility | Purpose |
|-----------|------|------------|---------|
| `pub fn new() -> Self` | 52 | pub | Default constructor. |
| `pub fn build(&mut self, manuscript: &ManuscriptAnalysis<'_>)` | 60 | pub | Clears `graph` + `node_map`, rebuilds from manuscript. Mutates self. |
| `fn add_triplet_edge(&mut self, triplet: &SvoTriplet, alias_to_canonical: &HashMap<String, String>)` | 108 | private | Resolves actor/target via alias map, aggregates edge weight by `triplet.confidence`. Skips self-loops. |
| `pub fn adjacency_matrix(&self) -> (Vec<String>, Vec<Vec<f64>>)` | 146 | pub | Returns (node_names, symmetric dense matrix). For each directed edge `i→j` with weight w, sets `matrix[i][j] += w` and `matrix[j][i] += w`. |
| `pub fn graph(&self) -> &DiGraph<String, f64>` | 170 | pub | Borrow underlying directed graph. |
| `pub fn node_map(&self) -> &HashMap<String, NodeIndex>` | 175 | pub | Borrow name → NodeIndex map. |
| `pub fn edge_count(&self) -> usize` | 180 | pub | Directed edge count. |
| `pub fn node_count(&self) -> usize` | 185 | pub | Node count. |
| `fn analyze(&self, manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport` | 191 | trait impl | **Builds a fresh `NarrativeGraph` internally** (does NOT use `self.graph`/`self.node_map`), so cached state is unused by `analyze`. Returns Ω_conf + ρ + counts with **empty `paradoxes`**. |

#### Methods on `ParadoxDetector`

| Signature | Line | Visibility | Purpose |
|-----------|------|------------|---------|
| `pub fn new() -> Self` | 101 | pub | Default constructor. |
| `pub fn detect(&mut self, manuscript: &ManuscriptAnalysis<'_>) -> Vec<Paradox>` | 106 | pub | Clears `self.deaths`, runs Phase 1 (death scan) + Phase 2 (post-death speech/action scan). Returns paradox list. |
| `fn has_death_marker(&self, text: &str, name: &str) -> bool` | 165 | private | Scans 40-char window after FIRST occurrence of `name` (lowercased) for any `DEATH_MARKERS` entry. |
| `fn has_speech_marker(&self, text: &str, name: &str) -> bool` | 186 | private | Same as `has_death_marker` but for `SPEECH_MARKERS`. |
| `fn find_death_before(&self, character: &str, current_ch: usize) -> Option<usize>` | 205 | private | Returns `.min()` death chapter — i.e. **earliest** death, NOT most recent (docstring says "most recent" → doc/code mismatch). |

#### Methods on `StubConflictAnalyzer`

| Signature | Line | Visibility | Purpose |
|-----------|------|------------|---------|
| `pub fn new() -> Self` | 38 | pub | weight = 0.5. |
| `pub fn with_weight(weight: f64) -> Self` | 43 | pub | Custom weight. |
| `pub fn count_keywords(&self, text: &str) -> usize` | 48 | pub | Case-insensitive substring count of `CONFLICT_KEYWORDS`. |
| `fn analyze(&self, manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport` | 58 | trait impl | Returns `omega_conf = weight × total_keyword_count`; `spectral_radius = 0.0`, `node_count = 0`, `edge_count = 0`, `paradoxes = Vec::new()`. |

#### Module constants

| Constant | File:line | Entries | Notes |
|----------|-----------|---------|-------|
| `DEFAULT_MAX_ITER: usize = 1000` | narrative_graph.rs:35 | — | Power iteration max steps. |
| `DEFAULT_TOL: f64 = 1e-9` | narrative_graph.rs:36 | — | Power iteration convergence threshold. |
| `DEATH_MARKERS: &[&str]` | paradox.rs:57-65 | 17 (with **4 duplicates**: `померла`, `помер`, `загинув`, `загинула` each appear 2×) | Ukrainian past-tense death verbs (masc+fem+plural). |
| `SPEECH_MARKERS: &[&str]` | paradox.rs:68-77 | 24 entries | Ukrainian speech verbs (infinitive + masc/fem past). |
| `CONFLICT_KEYWORDS: &[&str]` | stub.rs:21-27 | 17 entries | Ukrainian conflict lexicon for stub analyzer. |

---

## 3. Current State

### ✅ Fully implemented & unit-tested
- `frobenius_norm` — 4 tests (zero matrix, identity, known matrix, round-trip).
- `spectral_radius_power_iteration` — 4 tests (empty, non-square, identity, complete graph K_n with ρ = n−1).
- `build_node_index` — 1 test (round-trip).
- `NarrativeGraph::build` / `adjacency_matrix` / `analyze` — 10 tests covering: empty manuscript, single-character self-action, two-character interaction (Ω = √2, ρ = 1), three-character K_3 (Ω = √6, ρ = 2), negated actions, non-character target, alias resolution, concept exclusion, weight aggregation, determinism, SvoParser integration.
- `StubConflictAnalyzer::analyze` / `count_keywords` — 4 tests (empty, basic count, proportionality, determinism, case-insensitive).
- `ParadoxDetector::detect` (DeadSpeaking variant) — 6 tests: no-death baseline, dead-speaking via speech marker, dead-acting via SVO triplet, same-chapter death+action is not paradox, multiple-deaths tracking, feminine `померла` marker.

### ⚠️ Partially implemented
- `NarrativeGraph::analyze` — works but **always returns `paradoxes: Vec::new()`**. The docstring (line 206) explicitly says paradoxes are populated by a separate `ParadoxDetector` pass. No orchestration helper in `litgraph-core` merges them; only `src-tauri/src/commands/poler.rs:cmd_detect_paradoxes` does this manually.
- `spectral_radius_power_iteration` — handles non-negative matrices correctly (Perron-Frobenius), but returns `lambda.max(0.0)`, which would mask negative dominant eigenvalues for signed matrices. For adjacency matrices this is fine, but the function is generic enough to be misused.
- `find_death_before` — works but uses `.min()` (earliest death) while the docstring says "most recent death chapter" (line 204). Documentation/code mismatch.

### ❌ Stubbed / missing
- **`ParadoxKind::SpatialTeleportation`** — declared in enum (paradox.rs:37) but **NEVER emitted** by `detect()`. Docstring (paradox.rs:90-93) explicitly says: *"Not yet implemented. Requires Layer F location normalization to detect non-adjacent location pairs."*
  - Required data is partially available: `SvoTriplet.location: Option<String>` exists but is never consumed by `ParadoxDetector` or `NarrativeGraph`.
- **`ConflictReport.paradoxes` from `NarrativeGraph`** — always empty (see above).
- **`StubConflictAnalyzer` spectral_radius / node_count / edge_count** — always 0.0 / 0 / 0 (stub.rs:67-69 comment: "Stub doesn't compute ρ"). Only `omega_conf` is real (keyword-density based).

### Coverage of paradox kinds (CRITICAL for Layer G)

| ParadoxKind | Variant declared | Detection logic | Emits `Paradox`? | Tests? |
|-------------|------------------|------------------|------------------|--------|
| `DeadSpeaking` | ✅ paradox.rs:35 | ✅ Phase 1+2 in `detect()` | ✅ | ✅ 6 tests |
| `SpatialTeleportation` | ✅ paradox.rs:37 | ❌ placeholder per docstring | ❌ never | ❌ zero tests for emission (1 DTO conversion test exists in poler.rs) |

---

## 4. Gaps / Bugs / TODOs

### [BUG] `find_death_before` doc/code mismatch (paradox.rs:204-211)
- Docstring: *"Find the most recent death chapter for `character` before chapter `current_ch`."*
- Code: `self.deaths.iter().filter(…).map(|(_, ch)| *ch).min()` — returns **earliest** chapter, not most recent.
- Impact: If a character has multiple death markers (e.g., false positive in ch.1, real death in ch.5), the detector picks ch.1 as the "death chapter" for all subsequent paradoxes — could mislead Layer G's prompt construction.
- Fix: `.max()` if "most recent" is intended, OR fix docstring if "earliest" is intended.

### [BUG] `has_death_marker` / `has_speech_marker` find only FIRST name occurrence (paradox.rs:170, 190)
- `lower.find(&name_lower)` returns the byte offset of the **first** occurrence. If the character's name appears multiple times in the chapter (e.g., "Петро помер. … Петро ожив. … Петро знову помер."), only the first occurrence's 40-char window is scanned.
- Impact: If first occurrence has no death marker but a later one does, the death event is missed entirely.
- Fix: iterate `lower.match_indices(&name_lower)` and check each window.

### [BUG] Duplicate entries in `DEATH_MARKERS` (paradox.rs:57-65)
- `померла` appears on lines 58 and 61.
- `помер` appears on lines 58 and 62.
- `загинув` appears on lines 59 and 62.
- `загинула` appears on lines 59 and 63.
- Impact: minor perf hit (4 extra comparisons per death-marker scan), readability issue, suggests the array was edited without dedup.

### [STUB] `SpatialTeleportation` paradox — never emitted (paradox.rs:90-93)
- The `detect()` function has NO code path that pushes a `Paradox { kind: ParadoxKind::SpatialTeleportation, … }` to the result vector.
- Doc says: "Requires Layer F location normalization."
- Required signals already exist in `SvoTriplet.location: Option<String>` but are unused.

### [STUB] `NarrativeGraph::analyze` paradoxes always empty (narrative_graph.rs:206)
- `paradoxes: Vec::new()` hardcoded in `ConflictReport`.
- No `analyze_full()` helper in `litgraph-core` that orchestrates `NarrativeGraph + ParadoxDetector` together. Only the Tauri command `cmd_detect_paradoxes` does this assembly manually (src-tauri/src/commands/poler.rs:455-464).

### [GAP] `Paradox` lacks stable identifier
- No `id: String` or `id: u64` field.
- Layer G must reference paradoxes by composite key `(character, chapter_idx, origin_chapter_idx)` — fragile if multiple paradoxes share these (e.g., character dies twice in same chapter range).

### [GAP] `Paradox` lacks evidence text snippet
- Layer G prompt construction needs the actual text of the death chapter and the paradox chapter to ask an LLM "why is X speaking in ch.N when they died in ch.M?"
- Currently Layer G must re-fetch `ManuscriptAnalysis.chapters[chapter_idx]` and `ManuscriptAnalysis.chapters[origin_chapter_idx]` — requires holding the original text buffer, which is impossible across IPC boundaries.

### [GAP] `ManuscriptAnalysis` is not `Serialize` (mod.rs:82)
- Borrows `&'a str` slices, lifetime-tied to caller's text buffer.
- Cannot be sent over Tauri IPC or persisted to disk.
- Every Tauri command rebuilds it from `Vec<ParsedChapter>` (see poler.rs:402, 456-460).

### [GAP] No `analyze_chapter` returns per-chapter paradoxes
- The default `ConflictAnalyzer::analyze_chapter` builds a single-chapter `ManuscriptAnalysis` and calls `analyze`.
- But `ParadoxDetector::detect` requires **multiple chapters** (Phase 2 needs `deaths` populated by Phase 1 in earlier chapters) — so per-chapter paradox detection is meaningless via `analyze_chapter`.
- Layer D `compute_epsilon_climax_with_analyzer` calls `analyze_chapter` (epsilon.rs:528-546) — so paradoxes are inherently unavailable in the per-chapter ε_climax path.

### [GAP] `NarrativeGraph::analyze` rebuilds internally — cached `self.graph` is unused
- `analyze` (narrative_graph.rs:191-208) constructs `let mut ng = NarrativeGraph::new(); ng.build(manuscript);` internally.
- The receiver `&self` is treated as stateless — even if caller did `ng.build(manuscript)` first, `ng.analyze(manuscript)` ignores that cached graph.
- This is intentional (purity contract) but means the only way to inspect the built graph is to call `ng.build(manuscript)` and then `ng.graph()` / `ng.node_map()` separately from `ng.analyze(manuscript)`.

### [GAP] Negated actions inflate Ω_conf without penalty (narrative_graph.rs:131-137)
- "Петро не вбив ворога" (polarity=false) still adds `triplet.confidence` to the edge weight.
- Per Layer D ε_climax spec, negated actions still register conflict tension (intentional), but the edge weight is the SAME as affirmative — no polarity-based discounting.
- Could inflate Ω_conf for "denial sequences" (long passages of "X did not Y").

### [GAP] Self-loops silently dropped (narrative_graph.rs:122-124)
- `if actor_name == target_name { return; }` — skips introspective actions like "Петро вбив себе".
- Minor: suicide / self-harm events don't enter the conflict graph at all.

### [GAP] `add_triplet_edge` skips edges where actor or target isn't in `node_map`
- Triplets referencing un-detected characters (e.g., SVO parser extracts "він" pronoun but no character entry) are silently dropped (narrative_graph.rs:125-130).
- No counter / log — hard to debug "why is my graph empty?" without enabling tracing.

### [PERF] `spectral_radius_power_iteration` allocates 3 `Vec<f64>` per iteration (mod.rs:181-208)
- `w = vec![0.0; n]`, `v_new: Vec<f64>`, `av = vec![0.0; n]` — 3 allocations per power iteration step.
- For n=1000 characters × 1000 iterations = 3M allocations. Currently OK for manuscript-scale (n ≤ 100), but profiling needed if scaled.

---

## 5. Refactoring Opportunities

### [REFACTOR] Add `analyze_full()` orchestration helper in `mod.rs`
- A single function that runs both `NarrativeGraph::analyze` and `ParadoxDetector::detect` and merges into a complete `ConflictReport` with populated `paradoxes`.
- Benefit: removes duplicate orchestration code from `src-tauri/src/commands/poler.rs` (lines 455-464); makes Layer G consumption trivial.

```rust
pub fn analyze_full(manuscript: &ManuscriptAnalysis<'_>) -> ConflictReport {
    let ng = NarrativeGraph::new();
    let mut report = ng.analyze(manuscript);
    let mut det = ParadoxDetector::new();
    report.paradoxes = det.detect(manuscript);
    report
}
```

### [REFACTOR] Extract `DeathMarkerScanner` from `ParadoxDetector`
- `has_death_marker` / `has_speech_marker` are generic "name + verb-window match" — could be extracted into a reusable `WindowedMarkerScanner { markers: &[&str], window: usize }` struct.
- Benefit: enables adding new marker types (e.g., location markers, emotion markers) without duplicating the windowing logic.

### [REFACTOR] Use `HashMap<String, Vec<usize>>` for `ParadoxDetector::deaths`
- Current `Vec<(String, usize)>` requires O(n) scan in `find_death_before`. A map keyed by character name would make lookup O(1).
- Benefit: scales better for manuscripts with many characters.

### [REFACTOR] Deduplicate `DEATH_MARKERS` constant
- Use a `const` with unique entries, or use `phf::set!` for compile-time deduplication.

### [REFACTOR] Add `Paradox::id` and `Paradox::evidence_text`
- `id: String` (UUIDv4) — for Layer G hypothesis referencing.
- `evidence_text: String` — snippet of death chapter + paradox chapter text, ready for LLM prompt.

### [REFACTOR] Make `ManuscriptAnalysis` own its data
- Change `chapters: Vec<&'a str>` to `chapters: Vec<String>` and add `#[derive(Serialize, Deserialize)]`.
- Drop the lifetime parameter.
- Benefit: serializable, can be sent over IPC, persisted, cached.
- Cost: one extra `String` allocation per chapter — negligible.

### [REFACTOR] Extract `ParadoxDetector::detect` into a free function
- The detector is essentially stateless across `detect()` calls (it clears `self.deaths` at the start). The struct provides no value beyond namespacing.
- Could be `pub fn detect_paradoxes(manuscript: &ManuscriptAnalysis<'_>) -> Vec<Paradox>`.

### [REFACTOR] Add tracing instrumentation
- All public methods are silent. Adding `tracing::debug!` calls at key points (death detected, paradox emitted, edge added) would help debugging Layer G integration issues.

---

## 6. Layer G Relevance

**This module is the critical upstream feed for Layer G (LLM Reasoning Bridge).** Layer G will:

1. **Consume `Vec<Paradox>` from `ParadoxDetector::detect`** as the primary signal for hypothesis generation.
2. **Generate hypotheses** of kinds: `Flashback`, `Dream`, `Resurrection`, `Impostor` — one per paradox (or one per cluster of paradoxes sharing the same character).
3. **Validate LLM-generated explanations** against `NarrativeGraph` state (e.g., "if LLM proposes Impostor, verify no impostor character node exists in `graph.node_map()`").

### Data available for Layer G prompt construction

#### From `ParadoxDetector::detect(manuscript)` → `Vec<Paradox>`
For each paradox, Layer G has access to:
- `kind: ParadoxKind` — currently only `DeadSpeaking` is emitted; `SpatialTeleportation` is placeholder.
- `character: String` — canonical character name (already alias-resolved upstream by `NarrativeGraph::build`, but raw in `ParadoxDetector` — uses `c.name` directly from `ParsedCharacter`).
- `chapter_idx: usize` — index into `ManuscriptAnalysis.chapters` where the paradox manifests. Layer G can fetch `manuscript.chapters[chapter_idx]` for the offending scene text.
- `origin_chapter_idx: usize` — index into `ManuscriptAnalysis.chapters` where the death was recorded (for DeadSpeaking).
- `explanation: String` — pre-formatted English string: `"Character '{}' acts in chapter {} but died in chapter {}"` or `"Character '{}' speaks in chapter {} but died in chapter {}"`.

#### From `NarrativeGraph::build(manuscript)` then `graph()` / `node_map()` / `adjacency_matrix()`
- `node_map: &HashMap<String, NodeIndex>` — all character names in the conflict graph.
- `graph: &DiGraph<String, f64>` — full petgraph structure; Layer G can traverse neighbors of a paradox character to identify "witnesses" (other characters who interacted with the deceased after death).
- `adjacency_matrix() -> (Vec<String>, Vec<Vec<f64>>)` — symmetric dense matrix; can be JSON-serialized for the LLM context window as a "conflict web" summary.
- `edge_count() / node_count()` — quick statistics for prompt context ("in a manuscript with N characters and M conflict edges…").

#### From `ManuscriptAnalysis` itself
- `chapters: Vec<&str>` — raw chapter text. **CRITICAL**: this is the only way for Layer G to fetch the actual narrative context (death scene, speaking scene) for the LLM prompt. Lifetime-bound to caller's buffer.
- `characters_per_chapter: Vec<Vec<ParsedCharacter>>` — full character data (aliases, speech_count, direct_count, entity_type) per chapter. Useful for prompt context ("Петро is a Character with 5 speech acts and 2 direct addresses").
- `triplets_per_chapter: Vec<Vec<SvoTriplet>>` — full SVO extraction. Layer G can use this to construct "action timelines" for a character (e.g., "Петро's actions across all chapters: [Kill ворога ch.1, Speak ch.2 (paradox), Travel ch.3]").

### Data NOT available (gaps blocking Layer G)

1. **`SpatialTeleportation` paradoxes never emitted** — Layer G cannot consume location-teleportation signals without implementing location normalization in Layer F + adding detection logic to `ParadoxDetector::detect`. The `SvoTriplet.location` field is already there, but unused.
2. **`Paradox.id`** missing — Layer G must track paradox→hypothesis mapping by composite key. If a paradox is regenerated (e.g., manuscript edited), the mapping breaks.
3. **`Paradox.evidence_text`** missing — Layer G must re-fetch chapter text snippets, which requires holding the original `ManuscriptAnalysis.chapters` buffer in memory alongside the paradox list. Awkward across IPC.
4. **`ConflictReport.paradoxes` always empty when produced by `NarrativeGraph::analyze`** — Layer G must call `ParadoxDetector::detect` separately. Easy to forget; no compile-time enforcement.
5. **No `analyze_full()` helper** — Layer G must manually orchestrate `NarrativeGraph` + `ParadoxDetector`. Currently only `src-tauri/src/commands/poler.rs:cmd_detect_paradoxes` does this correctly.
6. **`Paradox` lacks `confidence: f64`** — Layer G cannot prioritize which paradoxes to send to the LLM first (e.g., for token-budget triage).

### Recommended Layer G integration shape

Layer G should add a Tauri command in `src-tauri/src/commands/` (e.g., `hypotheses.rs`) that:

```rust
#[tauri::command]
pub async fn cmd_generate_hypotheses(
    text: String,
    project: Project,
    llm_config: LlmConfig,
) -> Result<HypothesisReport, String> {
    // 1. Build ManuscriptAnalysis from text (reuse cmd_detect_paradoxes pipeline).
    // 2. Run ParadoxDetector::detect → Vec<Paradox>.
    // 3. For each Paradox, construct LLM prompt with:
    //    - character name + chapter_idx + origin_chapter_idx
    //    - chapter text snippets (from manuscript.chapters)
    //    - NarrativeGraph context (node_count, edge_count, neighbors of character)
    // 4. Call LLM via existing ai/ollama.rs or ai/openai_compat.rs.
    // 5. Parse LLM response into Hypothesis { paradox_ref, kind, confidence, evidence_text }.
    // 6. Validate hypothesis against NarrativeGraph state (e.g., Impostor requires
    //    no existing impostor node; Resurrection requires alive=false in WorldState).
    // 7. Return HypothesisReport { hypotheses, unresolvable_paradoxes, stats }.
}
```

The existing `ai/ollama.rs` and `ai/openai_compat.rs` modules already provide `chat_completion()` — Layer G can reuse this LLM client infrastructure.

The existing `src-tauri/src/reasoning/` module (separate from `litgraph-core/src/reasoning/` — the Symbolic UA LP engine) has its own `hypotheses.rs` and `llm_bridge.rs` that Layer G can model on or extend. **Note**: these are two parallel reasoning systems — `litgraph-core/src/reasoning/` (Layer E, POS-based) and `src-tauri/src/reasoning/` (Wave 5, semantic IR-based). Layer G must decide which to bridge through, or unify both.

---

## 7. Recommended Next Actions

1. **Add `analyze_full()` helper in `mod.rs`** — S (1-2h). Removes duplicate orchestration; enables Layer G to call a single function.
2. **Add `Paradox::id: String` (UUIDv4) and `Paradox::evidence_text: String` fields** — S (1h). Critical for Layer G hypothesis referencing and prompt construction.
3. **Implement `SpatialTeleportation` paradox detection** — M (4-8h). Requires:
   - Layer F location normalization (canonical location names from `SvoTriplet.location`).
   - New `LOCATION_MARKERS` constant + `has_location_marker` method.
   - Phase 3 in `detect()`: for each character, build location timeline; if two consecutive chapters have non-adjacent locations with no transit verb, emit `Paradox`.
4. **Fix `find_death_before` doc/code mismatch** — S (15min). Decide: earliest or most recent? Fix code or docstring.
5. **Fix `has_death_marker`/`has_speech_marker` to scan all name occurrences** — S (30min). Use `match_indices` instead of `find`.
6. **Deduplicate `DEATH_MARKERS` constant** — S (5min). Remove the 4 duplicate entries.
7. **Add `Paradox::confidence: f64` field** — S (30min). For Layer G token-budget triage.
8. **Make `ManuscriptAnalysis` own its data (drop `'a` lifetime)** — M (2-4h). Makes it serializable; ripple effect on all consumers (`epsilon.rs`, `poler.rs`, tests).
9. **Add tracing instrumentation to `NarrativeGraph::build` and `ParadoxDetector::detect`** — S (1h). Helps debug Layer G integration.
10. **Write integration test: full pipeline `text → chapters → characters → SVO → NarrativeGraph + ParadoxDetector → ConflictReport with non-empty paradoxes`** — S (1-2h). Currently no end-to-end test in `litgraph-core`.

---

## 8. Dependencies / Blockers

### Depends on
- `petgraph = "0.6"` (Cargo.toml:21) — for `DiGraph<String, f64>` and `NodeIndex`. No version pinning issue.
- `serde = { version = "1", features = ["derive"] }` (Cargo.toml:8) — for `ConflictReport` / `Paradox` / `ParadoxKind` serialization.
- `crate::linguistic::svo_parser::SvoTriplet` — provides `actor`, `verb`, `target`, `instrument`, `location`, `polarity`, `confidence` fields. The `location` field is the key dependency for `SpatialTeleportation` implementation.
- `crate::parser::characters::{ParsedCharacter, EntityType}` — provides `name`, `aliases`, `entity_type` (filter: only `Character` enters graph).
- `crate::parser::chapters::detect` (via `src-tauri/src/poler/mod.rs:38`) — produces `Vec<ParsedChapter>` consumed by Tauri commands to build `ManuscriptAnalysis`.

### Blocks
- **Layer G (LLM Reasoning Bridge)** — blocked on:
  - `Paradox::id` and `Paradox::evidence_text` (action 2) — without these, hypothesis referencing is fragile.
  - `analyze_full()` helper (action 1) — without this, Layer G must duplicate orchestration.
  - `SpatialTeleportation` implementation (action 3) — without this, Layer G only gets DeadSpeaking signals.
- **Layer D `compute_epsilon_climax_with_analyzer`** (epsilon.rs:528) — currently consumes `ConflictAnalyzer` trait; benefits from `analyze_full()` for richer per-chapter reports.
- **`src-tauri/src/commands/poler.rs::cmd_detect_paradoxes`** — already consumes this module; would benefit from `analyze_full()` to remove manual orchestration (poler.rs:455-464).
- **`src-tauri/src/reasoning/`** (Wave 5 Symbolic UA LP engine) — parallel reasoning system with its own `TemporalParadox` type (commands/reasoning.rs:22 imports `crate::reasoning::contradictions::TemporalParadox`). These two paradox types must be reconciled or unified before Layer G can consume both feeds.

### Cross-system paradox type duplication (CRITICAL ARCHITECTURAL NOTE)
There are **TWO** separate `TemporalParadox` types in the codebase:
1. `litgraph-core/src/reasoning/paradox.rs::Paradox` (Layer E, this module) — kind: `DeadSpeaking` | `SpatialTeleportation`.
2. `src-tauri/src/reasoning/contradictions.rs::TemporalParadox` (Wave 5, separate) — different fields, different detection logic.

Layer G must decide: consume one, the other, or unify both. The Tauri command `reasoning_validate_text` (commands/reasoning.rs:352) uses Wave 5's `TemporalParadox`, while `cmd_detect_paradoxes` (commands/poler.rs:396) uses Layer E's `Paradox`. **This is a critical architectural decision blocking Layer G.**
