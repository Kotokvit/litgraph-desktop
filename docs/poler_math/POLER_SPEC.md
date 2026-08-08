# POLER[Ψ] — Mathematical Specification for LitGraph v0.3

**Status**: Draft v1.0 (2026-08-09)
**Sources**: 5 documents (see `sources/`), answers from project mathematician
**Scope**: Mathematical foundation for replacing LitGraph v0.2 NLP pipeline (spaCy + SVO + J-matrix) with operator-algebraic POLER[Ψ] architecture

---

## 0. Context & Motivation

### 0.1. Why v0.2 fails

LitGraph v0.2 uses `spaCy ru_core_news_sm` for NER and rule-based SVO extraction. On the test novel «Сфера Предела» (2 MB, ~50 characters), it produced:

1. **NER merge errors**: "Рэй Вэнс" vs spurious "Винс" (algorithm split one character into two)
2. **No verb semantics**: "ударил" (physical aggression) and "увидел" (perception) get equal SVO weight
3. **Negation without scope**: "не остановил" → weight +16.6 as aggression (should be 0)
4. **SVO inversions**: passive voice breaks dependency parsing
5. **False NER entities**: "Хаба", "Багга", "Квотарий" — non-characters classified as PER

These errors cannot be fixed by tuning weights — they reveal that v0.2 operates on **surface statistics**, not on **semantic structure**.

### 0.2. What POLER[Ψ] replaces

v0.2 pipeline:
```
text → spaCy NER → SVO triplets → J-matrix (antisymmetric) → net aggression
```

v0.3 pipeline (POLER):
```
text → Perception (℘) → Image (O) → Logic (L) → Energy (ε) → Resonance (R[n]) → Ψ
       ↓                ↓            ↓          ↓             ↓              ↓
      operators        D=LLᵀ        Π_Λ        F=||obs-thought||²       R[n]=Σαρᵏs  HΨ=0
```

The 6 phases of the POLER cycle correspond to operator classes. "Understanding" is no longer a heuristic — it is the **stationary condition** HΨ = 0 (analogous to Wheeler-DeWitt equation in quantum gravity).

---

## 1. Space of Representation

### 1.1. Composite structure

POLER[Ψ] uses a **superposition** of three mathematical spaces:

| Space | Role | Why |
|---|---|---|
| **(b) Hilbert space ℋ** | "Arena" for operators D, J, H | Quantum-mechanical analogy: state = density matrix ρ, observables = Hermitian operators |
| **(c) Tensor space T^p_q(V)** | Multi-level SVO triplets as higher-rank tensors | "Meaning" is not a point — it is the result of deformed tensor product `a ⊗_ε b` |
| **(e) ∗-algebra** | Envelope ensuring correct interaction of non-commuting operators | Required for analytic commutator `[F, DM]_S` and for the annihilation of SVO inversions |

### 1.2. Dimensionality

- **Finite dimension D**, fixed via Rust `const generics`: D ∈ {128, 256, 512}
- Infinite (L²) rejected: leads to computational collapse, no real-time stationarity
- "Sparse Topology": although vocabulary is huge, per-query LENS activates only ~128 archetypes

### 1.3. Inner product ⟨a, b⟩

Defined as **cosine topology (semantic resonance)**:

```
⟨a, b⟩ = cos(θ_ab) ∈ [-1, 1]
```

- ⟨a, b⟩ → 1: stable causal link (e.g., "Рэй" and "Вэнс" must resonate for merging)
- Basis is **not orthonormal**: archetypes have complex overlaps (e.g., "aggression" ⊂ "action", but ⊥ "null-perception")
- Negation error fix lives here: the negation vector rotates phase, causing a Free Energy spike when v0.2 tries to assign +16.6 to "не остановил"

### 1.4. Engineering verdict

The transition to Hilbert space + tensor algebra of archetypes turns LitGraph from a "mention counter" into a "dynamic reality renderer". NER/SVO errors are now filtered by projector Π_Λ as physically impossible trajectories that don't lead to cognitive rest HΨ = 0.

---

## 2. The 14 Canonical Equations

These are the foundation. Each equation is annotated with its role in the LitGraph NLP pipeline.

| # | Equation | Name | LitGraph role |
|---|---|---|---|
| **1** | `dp/dt = -η·[D·p + γ·J·p + λ_O·O·p]` | Canonical dynamics | Main state evolution. Drives scene from raw observation to fixed-point attractor |
| **2** | `D = L·Lᵀ` | Dissipation operator | "Friction" that damps noise. False NER entities ("Винс") dissipate here |
| **3** | `J = A - Aᵀ` | Resonance operator (linear) | Directed SVO matrix. Antisymmetric: J[i,j] = -J[j,i] |
| **4** | `J(p) = J + β·p² + β·sin(p)` | Nonlinear resonance | Chaos + structure. Emergent patterns |
| **5** | `η(t) = η₀·exp(-β_σ·Σ(t))` | Adaptive step | Slows down in complex regions (high curvature Σ) |
| **6** | `γ(t) = γ₀·exp(-α_σ·Σ(t))` | Adaptive resonance | Weakens resonance in unstable regions |
| **7** | `E = κ·‖observation - thought‖²` | Free energy | Mismatch between world and prediction |
| **8** | `S = -Σ p_i·log(p_i) / log(n)` | Entropy | S→0: concentration, S→1: uniform noise |
| **9** | `m = ‖∇E‖⁻¹` | Effective mass | Character inertia. Heavy chars (m→∞) anchor the plot |
| **10** | `R(t) = ρ·R(t-1) + α·thought·(1+E)` | Resonance memory | Links current state to history; amplifies patterns |
| **11** | `g(p) = W₂·tanh(W₁·p)` | Nonlinear activation | Universal approximator inside dynamics |
| **12** | `dp/dt = A·p + g(p)` | Core mechanics | Alternative form (linear + nonlinear) |
| **13** | `∂L/∂w = λ_ent·∂S/∂w + β_L2·2w` | Entropy regularization | Weight update with entropy homeostasis |
| **14** | `p_norm = (1-mix)·p + mix·p/‖p‖` | Quantum normalization | Stabilization, analogy with ‖ψ‖=1 |

### 2.1. Operator algebra structure

The set {A, J, Π_Λ, H, F, ε, R} is **not a closed Lie algebra** (mixed functional classes: idempotent projectors, symmetric dissipators, antisymmetric resonances).

It forms a **∗-algebra of observables** with key operator being the **analytic commutator**:

```
[F, DM]_S = F·DM·S - S·DM·F
```

where:
- `F` = Fock operator (energy of meaning)
- `DM` = density matrix (state of the system)
- `S` = synaptic constraint operator (semantic flow)

**Structural constants**: ε (energy of significance) and γ (resonance coefficient).

**Commutator [A, J]**: defines **semantic friction**. If [A, J] ≠ 0, the system is in a frustrated state and requires iterative descent toward HΨ = 0.

---

## 3. Hamiltonian and Complex Structure

### 3.1. The Hamiltonian

```
H = L + iγJ - B/m
```

| Component | Physical meaning in LitGraph |
|---|---|
| `L` | Lagrangian (kinetic + potential of scene) |
| `i` | Imaginary unit (forces complex Hilbert space) |
| `γ` | Resonance coefficient (Eq.6, adaptive) |
| `J` | Antisymmetric resonance matrix (Eq.3) |
| `B` | Bias / mass potential (anchors) |
| `m` | Effective mass (Eq.9) |

### 3.2. Why complex?

The imaginary unit `i` is **necessary** to preserve Hermiticity when J is antisymmetric:

- J^T = -J  →  (iJ)^† = iJ  (Hermitian)
- H^† = H ⟺ Re(H), Im(H) both Hermitian

### 3.3. Physical interpretation

- **Re(H)** = "Scene Energy": static landscape, gravitational wells of co-occurrence, dissipative friction
- **Im(H)** = "Conflict Direction": dynamic momentum, flow of actions from aggressor to victim, phase inertia preventing local minima

### 3.4. Eigenvalues of iJ

Since J is real antisymmetric, iJ is Hermitian with **real eigenvalues**:

- λ_k ∈ ℝ, k = 1, ..., N
- Interpretation: **"Principal axes of semantic resonance"** or frequencies of the "conflict heartbeat"
- Largest |λ_k| → dominant archetypes (protagonist/antagonist) generating most directed action

### 3.5. Decomposition X = A_X + J_X

Any operator decomposes into symmetric + antisymmetric parts:

- **Purely symmetric scene (J=0)**: "statistical gel". Characters co-exist without directed interaction. Maximum entropy, zero causality. Decor or passive background.
- **Purely antisymmetric scene (A=0)**: "pure kinetics". Conflict in vacuum, no shared context. Mathematical analogue: undamped oscillations in a quantum box.

---

## 4. Projector Π_Λ — Causal Manifold

### 4.1. Definition

Π_Λ is the orthogonal projector onto the null-space of the constraint matrix J_c:

```
Π_Λ = orthogonal projector on Null(J_c)
Π_Λ² = Π_Λ   (idempotent)
Π_Λ^T = Π_Λ  (symmetric)
```

### 4.2. Role

Projects state onto the **causal manifold** where:
- Logic laws hold
- Meaning conservation holds: p₀ - p₁ = 0 (initial state = final state of meaning)

### 4.3. Lagrangian interpretation

In symplectic geometry, a Lagrangian submanifold L of a symplectic manifold (M, ω) is:
- Isotropic: ω|_L = 0
- Maximal: dim L = (1/2) dim M

In POLER: Π_Λ projects onto a Lagrangian sub-space where the Hamiltonian dynamics stays stable (no transformation into chaos).

### 4.4. Annihilation of hallucinations

Any SVO inversion that violates p₀ - p₁ = 0 is **orthogonally cut off** by Π_Λ. This is the mathematical mechanism for:
- Detecting wrong NER merges ("Винс" not in causal manifold)
- Filtering impossible SVO inversions
- Enforcing narrative causality

---

## 5. Coreference as Idempotent Projector

### 5.1. The operator P

Coreference resolution is formalized as an **idempotent projector** P satisfying:

```
P² = P
P^T J P = J    (preserves conflict structure)
```

### 5.2. Ontology

- **Im(P)** — "Image": subspace of canonical archetypes. The "skeleton of reality". Each vector = invariant truth of one character.
- **Ker(P)** — "Kernel": semantic noise and aliases. Pronouns ("он", "его"), erroneous NER entities ("Винс"). Mathematically annihilated during projection.

### 5.3. Orthogonal decomposition

Any mention `x` decomposes as:

```
x = Px + (I-P)x
       ↓       ↓
   canonical  "information tax" — burns in dissipator D = L·Lᵀ
   part       only Px has "information superconductivity"
```

### 5.4. Hierarchy of projectors

POLER[Ψ] requires a **cascade**, not a single P:

```
P_atom   ⊂  P_entity  ⊂  P_scene
  ↓             ↓             ↓
morphemes    character     global Π_Λ
pronouns     "Рэй Вэнс"    causal coherence
```

### 5.5. Diagnosis of "Винс" as topological error

> User clarification: "Винс это ошыбка алгоритм должен Претположыть наличе ошыбки в тексте"

The system operates by **"No Excuses" principle**:

1. Direct string match "Винс" → "Вэнс" is blocked by low cosine similarity
2. But resonance R[n] indicates identity of action subject
3. → Activate **"Prism" archetype**: refract "Винс" vector toward nearest attractor with minimal surprise energy ε
4. **Instead of creating a new node**, vector is refracted
5. Error fact is recorded as a **topological curvature spike** Σ(t) ↑
6. System slows step η for trajectory correction

This is the difference between **alias resolution** (merging "он" → "Рэй") and **error detection** (recognizing "Винс" as OCR/NER corruption of "Вэнс").

---

## 6. Negation Operator N

### 6.1. Algebraic properties

N is a **unitary involutive operator** in ℋ:

| Property | Meaning |
|---|---|
| `N² = I` | Involution: "не не остановил" = "остановил" |
| `N^T = N` | Symmetric: preserves cosine metric, changes only vector orientation |
| `N·J = -J` | Reverses resonance. Phase shift by π. "Не ударил" = vector opposite to "ударил" |
| `N·A ≈ A` | Co-occurrence mostly preserved. Characters stay in same gravity well, but semantic valence (ε) deforms |

### 6.2. Scope as commutator

Scope is **not** a local property. Mathematically:

```
p_neg = Π_scope · N · p_t
```

where `Π_scope` is a sub-projector activated by the position operator `T_pos`.

Example: "Я не видел, как он её ударил"
- Π_scope isolates component "восприятие" (видел)
- Component "действие" (ударил) is **orthogonal** to scope window
- → "ударил" is NOT inverted by N

This is realized through the **Trajectory Decoder**, which distinguishes "meaning phonons" by their position in the temporal resonance R[n].

### 6.3. Representation in J-matrix

| Verb form | J value | Where it goes |
|---|---|---|
| "остановил" (action) | J = +w | Resonance matrix |
| "запретил" (counter-action) | J = -w | Resonance matrix |
| "не остановил" (absence) | **J = 0** | Vector transferred to dissipator D |

When J → 0 while expectation predicted action: **ε (surprise energy) spike** signals hidden significance of inaction.

---

## 7. Verb Classification — Tensor Category

### 7.1. Structure of verb set V

We choose **(c) Tensor category**: verbs are morphisms acting in the Hilbert space of character states.

```
v: SubjectState × ObjectState → SubjectState' × ObjectState'
```

A verb is **not a point** — it is a transition operator. Each morphism v ∈ V has a **synaptic signature** determining which of {A, J, D, Π} it activates.

### 7.2. Composition law

Sequential actions compose in reverse order (operator composition):

```
"Он подошёл и ударил"   →   p_{t+2} = V̂_hit ∘ V̂_approach (p_t)
```

Because operators in ∗-algebra generally don't commute ([V₁, V₂] ≠ 0), **order is critical** for HΨ = 0.

### 7.3. Split: action vs perception

| Verb class | Operator | J effect | A effect |
|---|---|---|---|
| **Active action** ("ударил") | V̂_act → J ≠ 0 | Directed aggression impulse | — |
| **Perception** ("увидел") | V̂_perc → Π_℘ | J ≈ 0 | Modifies co-occurrence |

This solves v0.2's bug where "ударил" and "увидел" received equal SVO weight.

### 7.4. Commutation relations

```
[J_a, Π_p] ≠ 0    — observation changes aggression energy (observer effect)
{J_a, Π_p} = 0   — only in idealized isolated scenes ("Platinum Cube")
```

The analytic commutator measures **surprise (ε)**: larger deviation from zero → higher scene significance energy.

---

## 8. Topology of Narrative

### 8.1. Structure choice

**Composition** of:
- **(c) Simplicial complex**: characters + interactions form simplices (3 chars = 2-simplex, not 3 edges)
- **(d) ∞-category**: actions can be objects for higher-order morphisms (e.g., "betrayal" as a morphism between "friendship" and "enmity")

### 8.2. Persistent homology of J

| Homology | Meaning |
|---|---|
| H₀ | Connected components = isolated plot lines. HΨ = 0 ⟹ single component. |
| H₁ | Cycles = conflict triangles (A→B→C→A). Non-destroyable cycles = "rotor of meaning" ∇×J ≠ 0, endless oscillation. Useful for detecting drawn-out scenes. |
| H₂ | Cavities = multi-party contradictions, "blind spots" where F → ∞. |

### 8.3. Persistence diagrams

Bottleneck distance between diagrams = absolute metric of conflict-structure similarity, operating in "Observer-Kill" mode.

- Allows comparing "Гамлет" and "Король Лев" not by words but by **topological skeleton**
- If distance → 0, novels are isomorphic at archetype level

---

## 9. Invariants of Text

### 9.1. Paraphrase invariants

When rephrasing "Иван ударил Петра" ↔ "Петр был побит Иваном":

| Invariant | Why |
|---|---|
| **Spectrum of J** | Conflict intensity preserved |
| **Spectrum of H** | Stationary energy invariant under paraphrase group |
| **H₀, H₁** of simplicial complex | Topological skeleton of narrative |
| **Tr(H)** | Total semantic mass of scene |

### 9.2. Sense-change variants

When meaning changes ("ударил" → "поцеловал"):

| Variant | Change |
|---|---|
| Operator type V | J-channel vs Π_℘ channel |
| Spectrum of J | Magnitudes change (tension shifts) |
| Topological curvature Σ(t) | Spikes at meaning surprise |
| Metric tensor G(p) | Latent space geometry deforms |

### 9.3. Understanding = finding invariant I

Yes. "Understanding a scene" mathematically = **finding I(text)** such that I is invariant under the paraphrase group.

Stationary point `p*` satisfying `p* = a ⊗_ε p*` represents this invariant. Proof of understanding: `[F, DM]_S → 0`.

---

## 10. Computational Complexity

For N=50 characters, V=1000 verb types, 2 MB text:

| Operation | Complexity | Practical |
|---|---|---|
| Build J (50×50) | O(N²) | trivial |
| Build T (50×1000×50 = 2.5M) | O(real_interactions) | CSR sparse, 30-50% memory savings |
| Operator basis | 14 canonical mechanisms | — |
| Spectrum of Hermitian H | O(N³) = 125k ops | milliseconds on CPU |
| vs transformer forward pass | — | 6-175× more expensive |

**Simplicial complexes (GUDHI)**: direct Rips-Vietoris for 10k points = exponential wall. Solution: **Sparse Topology** — LENS filter activates local subgraphs (K_active ≈ 128).

**Hardware**:
- CPU (Rust + `faer` library): sufficient for 2 MB text, f64 precision, L1/L2 cache optimization
- GPU (Vulkan/WGPU via Burn): 220% boost for phases O and ε (mass-parallel cosine topology + tensor decompositions)
- Target: "Golden frequency" 1/8 s for real-time conflict rendering

---

## 11. Quality Metrics

### 11.1. The 3 metrics

| Metric | Formula | Meaning |
|---|---|---|
| **Commutator norm** | `‖[F_fock, DM]_S‖_F` | Main metric. → 0 means HΨ = 0 reached = mathematical proof of understanding |
| **Spectral entropy** | `S_spec = -Σ λ_k log(λ_k) / log(N)` | Low = strong structure, high = noise |
| **Cognitive conductivity** | `Φ ≈ 1 - F/F_max` | 0.9999 = nearly ideal information flow |

### 11.2. Validation without ground truth

| Principle | Check |
|---|---|
| **Paraphrase stability** | Spectrum J invariant under any meaning-preserving paraphrase |
| **Physical integrity** | J^T = -J, P² = P, DM² = DM |
| **Observer-Kill** | Different random initializations → same global attractor HΨ = 0 |

Monitor `‖[F, DM]_S‖_F` instead of perplexity. When commutator freezes at 0, understanding becomes a mathematically proven fact.

---

## 12. Noether Theorem — Symmetries and Conservation Laws

| Symmetry | Conservation law |
|---|---|
| Phase rotation (J / Sp(2n, ℝ)) | **Meaning Conservation**: energy of intent doesn't disappear, only refracts through Prism until stationarity |
| Time translation | **Cognitive Hamiltonian H**: internally consistent scene keeps energy constant on trajectory |
| Scale (ε) | **Information Density**: T = ΔI/ΔΣ stays constant during rephrasing |
| Projection (Π_Λ) | **Causality**: p₀ - p₁ = 0 enforced |

---

## 13. Open Questions for Mathematician

These are **not** blocking — they are clarification requests for v0.4+ iterations.

### Q-1. Cryptographic vs NLP variant

Source `[2]` describes POLER as a **cryptographic framework** over GF(2ⁿ) with cellular automata (Rule 90/150), Rieffel twist cocycles, and finite-field polar inversion `I_ε(y) = y^(q-2) mod q`.

Your answers describe POLER for NLP over **ℂ** with Hilbert space, Hermitian operators, and continuous J-matrix.

**Are these:**
- (a) Two separate implementations of the same architectural template?
- (b) Same system at different levels (e.g., continuous math for NLP, finite-field for crypto verification)?
- (c) The NLP version is the continuous limit of the discrete crypto version?

### Q-2. Burn vs "no PyTorch"

Source `[3, 5]` uses **Burn** (Rust-native deep learning framework) with `LinearConfig`, `LayerNorm`, `Tensor::random` — these are **parametric layers** that require training.

You said "PyTorch + transformers нам не нужны это устаревшие примитивы".

**Clarification needed:**
- (a) Burn with parameter training IS acceptable (just not PyTorch/Python)?
- (b) Burn is only used for tensor operations, not for trained layers?
- (c) All weights are **analytically derived** (not learned)?

### Q-3. Deformed tensor product ⊗_ε in NLP context

In crypto: `a ⊗_ε b = (a·b) ⊕ (ε·((a∧b) ⊕ Φ(a⊕b))) mod q`

For NLP, what's the concrete formula? Candidates:
- (a) Same formula but over ℝ with `·` = inner product, `⊕` = vector sum, `Φ` = activation function
- (b) Deformed Kronecker product: `A ⊗_ε B = A ⊗ B + ε·(A·B - B·A)`
- (c) Rieffel twist on representation: `a ⊗_ε b = Σ_g,h ε(g,h) α_g(a) ⊗ α_h(b)` where G is a character symmetry group

### Q-4. "Platinum Cube" anti-commutator {J_a, Π_p} = 0

You mentioned this holds "only in idealized isolated scenes". What's an example of such a scene in literary text? Is this a useful idealization or purely theoretical?

### Q-5. Scale of const generics D

You suggested D ∈ {128, 256, 512}. How to choose?
- D=128: fast but might miss nuance
- D=512: rich but slow

What's the empirical criterion? Number of distinct characters in text? Number of archetype classes?

---

## 14. Next Steps

After this spec is reviewed:

1. **Verify algebra symbolically** — `poler_verify.py` (sympy) checks:
   - J^T = -J for arbitrary A
   - (iJ)^† = iJ (Hermiticity)
   - P² = P for typical projector
   - Π_Λ² = Π_Λ (idempotency)
   - [A, J] structure for sample matrices

2. **Plan integration** — `INTEGRATION_ROADMAP.md` covers:
   - What to keep from v0.2 (text ingestion, character list)
   - What to replace (J-matrix builder → POLER Hamiltonian)
   - What to add (Burn backend in Rust, GPU phase)
   - Test corpus for validation

3. **Iterate** — answer open questions, refine spec to v1.1
