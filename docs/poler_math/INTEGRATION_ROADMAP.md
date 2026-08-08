# POLER[Ψ] → LitGraph Integration Roadmap

**Status**: Draft v1.0 (2026-08-09)
**Spec**: `POLER_SPEC.md`
**Verification**: `poler_verify.py` (48/48 ✓)

---

## 0. Current State (LitGraph v0.2.2)

### 0.1. What works

| Component | Status | File |
|---|---|---|
| Tauri 2 + React 19 desktop app | ✅ running | `src-tauri/src/lib.rs` |
| spaCy NER extraction | ✅ works (with errors) | `src-tauri/python/ner_extract.py` |
| SVO triplet extraction | ✅ works (with errors) | `src-tauri/python/svo_extract.py` |
| J-matrix builder (antisymmetric) | ✅ correct | `src-tauri/python/build_j_matrix.py` |
| Conflict graph viz (SVG) | ✅ in-app | `src/components/litgraph/ConflictGraphDialog.tsx` |
| PNG/PDF export | ✅ works | `src/lib/conflict/export.ts` |

### 0.2. What fails (verified by vision-model on «Сфера Предела»)

1. NER merges/splits ("Рэй Вэнс" vs spurious "Винс")
2. No verb semantics ("ударил" = "увидел" in weight)
3. Negation without scope ("не остановил" → +16.6)
4. SVO inversions on passive voice
5. False NER entities ("Хаба", "Багга", "Квотарий")

### 0.3. Architecture

```
src-tauri/
├── src/
│   ├── lib.rs                  # Tauri command registration
│   ├── commands/
│   │   ├── ner.rs              # Python subprocess runner
│   │   └── conflict.rs         # get_conflict_graph Tauri command
│   ├── parser/                 # Markdown chapter splitter (Rust)
│   └── ai/                     # LLM bridge (unused in POLER path)
└── python/
    ├── ner_extract.py          # spaCy NER (keep as fallback)
    ├── svo_extract.py          # SVO + pronoun resolution (replace)
    ├── build_j_matrix.py       # J = A - A^T (keep, extend)
    └── conflict_graph.py       # Pipeline orchestrator
```

---

## 1. POLER[Ψ] Source Code Available

User provided 5 source documents covering an existing **Rust implementation** of POLER[Ψ]:

| File | Lines | Content |
|---|---|---|
| `1-POLER_UNIFIED_ALGORITHM.md` | 19 | 14 canonical equations with comments |
| `2-Mathematical Formulation.md` | 123 | Crypto variant over GF(2ⁿ) + Rieffel twist |
| `3-POLER_CORE_COMPLETE_CODE.md` | 90 | Index of 19 Rust files (7183 lines total) |
| `4-POLER_PART1_RUST_CORE.md` | 34 | File listing for Rust core + bridge |
| `5-poler_sctp_complete.txt.md` | 126 | Burn-based implementation (partial snippets) |

### 1.1. Existing Rust modules (from sources)

| Module | Lines | Role | Reuse for LitGraph? |
|---|---|---|---|
| `poler-core/src/lib.rs` | 64 | Library entry, constants, PolerMode enum | ✅ direct reuse |
| `poler-core/src/energy_engine.rs` | 463 | Free energy, entropy, mass, resonance (Eq.2,7,8,9,10,13,14) | ✅ direct reuse |
| `poler-core/src/synaptic_ops.rs` | 353 | ConstraintLayer, ResonanceMatrix, CanonicalDynamics, CosineTransform | ✅ direct reuse |
| `poler-core/src/quantum_bridge.rs` | 279 | Qubit-as-archetype bridge | ⚠️ optional |
| `poler-core/src/vulkan_accelerator.rs` | 324 | WGPU/Vulkan GPU acceleration | ⚠️ optional (CPU first) |
| `poler-core/src/subquantum_bridge.rs` | 1721 | Full physics engine | ⏸️ defer to v0.5+ |
| `poler-core/src/instruction_index.rs` | 2094 | 100-rule instruction set | ⏸️ defer |
| `poler-bridge/src/tcp_server.rs` | 487 | Python ↔ Rust TCP bridge | ✅ needed if Python stays for NER |
| `poler-bridge/src/bridge_loader.rs` | 235 | JSON registry import | ✅ direct reuse |
| `poler-tcp/shared_protocol.py` | 188 | Python TCP protocol | ✅ direct reuse |
| `poler-tcp/client_demo.py` | 328 | Python client | ✅ adapt for LitGraph |

**Total LOC immediately reusable**: ~2400 lines (core + bridge)
**Total LOC available**: 7183 lines

### 1.2. What's missing for LitGraph

POLER core handles the **dynamical system** (state evolution, energy, resonance). It does NOT handle:

1. **Text → character state mapping** (NER + SVO → operator inputs)
2. **Verb classification** (action vs perception vs communication)
3. **Coreference resolution** (P operator — partially in POLER but no NLP interface)
4. **Negation scope detection** (N operator needs syntactic tree position)
5. **LitGraph UI integration** (Tauri command, React component)

These are the LitGraph-specific layers we need to add.

---

## 2. Integration Strategy: Hybrid Pipeline

**Don't replace everything at once.** Build a hybrid where POLER validates and corrects v0.2 outputs, then progressively replaces v0.2 components.

### Phase 3.1 — POLER as Validator (2 weeks)

**Goal**: Run POLER alongside v0.2, flag errors, but don't change v0.2 outputs yet.

```
text → [v0.2: spaCy NER + SVO + J-matrix] → UI (current)
                ↓
       [POLER: project on Π_Λ]
                ↓
       {errors detected} → UI warnings
```

**Tasks**:
1. Port `poler-core` (energy_engine + synaptic_ops) into `src-tauri/src/poler/`
2. Add Tauri command `validate_conflict_graph(graph: ConflictGraph) -> ValidationResult`
3. Implement Π_Λ projector (idempotent, on Ker(J_c))
4. Compute `‖[F, DM]_S‖_F` as validation metric
5. UI: show validation score in ConflictGraphDialog footer

**Acceptance**: «Сфера Предела» analysis shows specific errors (e.g., "Винс" flagged as topological defect with Σ(t) spike).

### Phase 3.2 — POLER Replaces J-matrix (3 weeks)

**Goal**: Replace `build_j_matrix.py` with POLER Hamiltonian H = L + iγJ − B/m.

```
text → [v0.2: NER + SVO] → POLER Hamiltonian → eig(H), eig(iJ) → UI
```

**Tasks**:
1. Implement H construction in Rust (energy_engine has D, J; add L, B, m)
2. Compute spectrum of iJ (real eigenvalues = principal conflict axes)
3. Replace `net_aggression` with `eig(iJ)` ranking
4. UI: replace "out − in" bars with eigenvalue spectrum display
5. Validate: paraphrase test ("Иван ударил Петра" ↔ "Петр был побит Иваном" → same eig(iJ))

**Acceptance**: Paraphrase stability holds (spectrum invariant).

### Phase 3.3 — Verb Classification + Negation Scope (4 weeks)

**Goal**: Replace rule-based SVO with operator-aware verb morphisms.

```
text → [v0.2: NER] → POLER verb classifier → V̂_act / V̂_perc → J, Π_℘ → UI
```

**Tasks**:
1. Build verb lexicon (300 verbs × 6 classes: physical, perception, communication, motion, mental, possession)
2. Implement `V̂_act` (activates J) vs `V̂_perc` (activates Π_℘) split
3. Implement N operator (block-diagonal: -I on V_J, +I on V_A) — requires D ≥ 4
4. Implement Π_scope (sub-projector for negation scope via syntax tree position)
5. UI: "не остановил" → J=0, ε spike shown as "hidden significance of inaction"

**Acceptance**: "не остановил" no longer gets +16.6 weight.

### Phase 3.4 — Coreference via P Projector (3 weeks)

**Goal**: Resolve "Рэй Вэнс" / "Винс" / "он" via idempotent P satisfying P^T J P = J.

```
text → [v0.2: NER candidates] → POLER P cascade → canonical characters → UI
```

**Tasks**:
1. Implement P_atom (morpheme/pronoun level)
2. Implement P_entity (per-character anchor)
3. Implement P_scene (global Π_Λ)
4. Detection of topological defects (Σ(t) spikes → "Винс" flagged as OCR/NER error, not alias)
5. UI: show "topological errors" panel with refracted-vs-rejected candidates

**Acceptance**: "Винс" either collapses into "Рэй Вэнс" or is flagged as error (not created as separate node).

### Phase 3.5 — Topological Analysis (2 weeks)

**Goal**: Add persistent homology (H₀, H₁, H₂) to conflict structure.

**Tasks**:
1. Build simplicial complex from J (filtered by ε threshold)
2. Use `gudhi` (Python) or port to Rust `ta-rs` for H₀, H₁, H₂
3. UI: "Topology" tab in ConflictGraphDialog showing persistence diagram
4. Bottleneck distance between scenes for similarity search

**Acceptance**: "Гамлет" and "Король Лев" (if both analyzed) show low bottleneck distance.

### Phase 3.6 — GPU Acceleration (optional, 2 weeks)

**Goal**: Burn + WGPU backend for phases O and ε.

**Tasks**:
1. Enable `wgpu` feature in Burn (Cargo.toml)
2. Move mass-parallel cosine topology to GPU
3. Benchmark: target 1/8 s for 2 MB text

**Acceptance**: 220% speedup vs CPU baseline.

---

## 3. File Structure After Integration

```
src-tauri/
├── src/
│   ├── lib.rs                       # + poler module
│   ├── commands/
│   │   ├── conflict.rs              # modified: returns POLER Hamiltonian
│   │   ├── poler_validate.rs        # NEW: validation command
│   │   └── poler_topologies.rs      # NEW: persistent homology
│   ├── poler/                       # NEW: POLER Rust core
│   │   ├── mod.rs
│   │   ├── energy_engine.rs         # ported from sources
│   │   ├── synaptic_ops.rs          # ported from sources
│   │   ├── projectors.rs            # NEW: Π_Λ, P_atom, P_entity, P_scene
│   │   ├── negation.rs              # NEW: N operator + scope
│   │   ├── verb_classifier.rs       # NEW: V̂_act, V̂_perc, V̂_comm
│   │   └── hamiltonian.rs           # NEW: H = L + iγJ - B/m
│   └── parser/                      # unchanged
└── python/
    ├── ner_extract.py               # keep as fallback
    ├── svo_extract.py               # keep as fallback
    ├── build_j_matrix.py            # deprecate (replaced by Rust)
    ├── conflict_graph.py            # modified: calls Rust POLER via TCP
    └── poler_topologies.py          # NEW: gudhi wrapper for homology

src/components/litgraph/
├── ConflictGraphDialog.tsx          # extended: POLER metrics, topology tab
├── PolerMetrics.tsx                 # NEW: ‖[F,DM]_S‖, S_spec, Φ display
└── TopologyPanel.tsx                # NEW: persistence diagram
```

---

## 4. Testing Strategy

### 4.1. Unit tests (Rust)

| Test | What it verifies |
|---|---|
| `test_j_antisymmetric` | J^T = -J for random A |
| `test_ij_hermitian` | (iJ)^† = iJ |
| `test_projector_idempotent` | P² = P, P^T J P = J |
| `test_negation_involution` | N² = I, N J = -J |
| `test_hamiltonian_real_spectrum` | eig(H) ∈ ℝ |
| `test_paraphrase_invariance` | "Иван ударил Петра" ↔ passive → same eig(iJ) |

### 4.2. Integration tests

| Test | What it verifies |
|---|---|
| `test_vinces_error_detection` | "Винс" flagged as topological defect, not alias |
| `test_negation_scope` | "не остановил" → J=0, ε spike |
| `test_verb_split` | "ударил" → J≠0; "увидел" → J=0, A modified |
| `test_coreference_cascade` | "он" → "Рэй" via P_atom → P_entity |

### 4.3. Corpus tests

| Text | Expected behavior |
|---|---|
| `01_conflict_scene.md` (existing) | 4 chars, 5 directed edges (validated) |
| `1-Сфера Предела.md` (2 MB) | "Винс" flagged, "не остановил" → J=0 |
| Paraphrase pair | Same eig(iJ) |
| Empty text | Empty graph, no crash |

### 4.4. Validation metric

```rust
fn validation_score(graph: &ConflictGraph) -> f64 {
    let commutator_norm = compute_commutator_norm(&graph);
    let spectral_entropy = compute_spectral_entropy(&graph);
    let conductivity = 1.0 - commutator_norm / max_energy;
    conductivity  // → 1.0 means HΨ = 0 reached
}
```

Target: `validation_score > 0.9` on «Сфера Предела» after Phase 3.3.

---

## 5. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Burn framework incompatibility with Tauri | Medium | High | Use `ndarray` backend first, WGPU later |
| Π_Λ projector unstable for sparse J | Medium | Medium | Add regularization (Tikhonov) |
| Verb lexicon incomplete for Russian | High | Medium | Start with 300 verbs, expand via corpus analysis |
| GPU unavailable on user's Linux | Low | Low | CPU fallback always works |
| POLER cycle doesn't converge for some texts | Medium | High | Cap iterations at 100, return best state |
| Complex Hamiltonian confuses users | High | Low | UI shows Re(H) and Im(H) separately |

---

## 6. Open Questions for Mathematician (from POLER_SPEC.md §13)

These need answers before Phase 3.3:

1. **Crypto vs NLP variant**: Are they the same architectural template applied to different domains, or is NLP a continuous limit of crypto?
2. **Burn training**: Are Burn's `LinearConfig` layers trained (gradient descent) or analytically derived?
3. **⊗_ε formula for NLP**: Concrete formula over ℝ (not GF(2ⁿ))?
4. **Platinum Cube**: Real example of {J_a, Π_p} = 0?
5. **D choice**: How to pick D=128 vs 256 vs 512?

---

## 7. Milestone Summary

| Phase | Duration | Deliverable | Acceptance criterion |
|---|---|---|---|
| 3.1 | 2 weeks | POLER validator | «Винс» flagged |
| 3.2 | 3 weeks | Hamiltonian replaces J-matrix | Paraphrase stability |
| 3.3 | 4 weeks | Verb classes + negation | "не остановил" → J=0 |
| 3.4 | 3 weeks | Coreference P cascade | "Винс" resolved or rejected |
| 3.5 | 2 weeks | Topology (homology) | Persistence diagram in UI |
| 3.6 | 2 weeks | GPU acceleration | 1/8 s response time |

**Total**: 16 weeks (4 months) for full v0.3.

After v0.3: Phase 4 = Rust/Burn full port (consolidate), Phase 5 = literary critique module.

---

## 8. Next Immediate Action

**Start Phase 3.1** by porting `poler-core` into `src-tauri/src/poler/`:

1. Copy `energy_engine.rs` and `synaptic_ops.rs` from sources
2. Add to `Cargo.toml`: `burn = { version = "0.14", features = ["ndarray"] }`
3. Implement `validate_conflict_graph` Tauri command
4. Test on existing `01_j_matrix.json` (4×4) — compute ‖[F, DM]_S‖_F
5. If validation score is low for v0.2 output → confirms errors detected

This is the **minimal viable POLER integration**: 1 new Rust module, 1 new Tauri command, 1 new UI panel. ~500 lines of new code.
