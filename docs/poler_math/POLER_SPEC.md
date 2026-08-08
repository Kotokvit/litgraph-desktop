# POLER[Ψ] — Mathematical Specification for LitGraph v0.3

**Status**: v1.1 (2026-08-09) — 5 open questions resolved by mathematician
**Sources**: 5 documents (see `sources/`), full mathematician Q&A, poler-os crypto reference (https://github.com/Kotokvit/poler-os)
**Scope**: Mathematical foundation for replacing LitGraph v0.2 NLP pipeline (spaCy + SVO + J-matrix) with operator-algebraic POLER[Ψ] architecture

**Changelog v1.1** (supersedes v1.0):
- §13 Open Questions → RESOLVED. Each of 5 questions now carries the mathematician's verdict + physical justification.
- Added §14 NLP ↔ Crypto Duality — references `poler-os` Zig kernel and GF(2ⁿ) discrete crystal.
- Added §15 Deformed Tensor Product ⊗_ε for NLP — Rieffel deformation over ℝ/ℂ.
- Added §16 Burn Structural Tuning — distinguishing parametric layers from statistical learning.
- Added §17 Platinum Cube — concrete literary example of {J_a, Π_p}=0.
- Added §18 Dimensionality Selection — Qualia-count criterion for D ∈ {128, 256, 512}.
- §14 Next Steps renumbered to §19 and expanded with Jupyter verification plan.

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

## 13. Resolved Decisions (Mathematician's Verdicts)

The 5 open questions from v1.0 are now closed. Each subsection records the verdict, the physical-mathematical justification, and the engineering consequence for LitGraph v0.3.

### 13.1. Q-1 — Crypto vs NLP: Status of Realization → **Superposition (b) + (c)**

**Verdict**: NLP-version is the **continuous limit** of the discrete crypto-version; both obey the same 14 canonical equations.

**Physical-mathematical justification**:

POLER is a single dynamical system. Text (NLP) and cipher (Crypto) are two projections of the same algebraic structure:

- **Crypto variant over GF(2ⁿ)** — "discrete crystal" for FPGA. Non-linearity is locked into a bit lattice. The polar inversion `I_ε(y) = y^(q-2) mod q` is a rigid Galois-field automorphism. Implemented in the `poler-os` Zig kernel (`src64/poler_core.zig`, `polarInversion32`, `pndMix`, `nilpotentOperator`).
- **NLP variant over ℝ/ℂ** — continuous limit of the same algebra. Discrete GF operations unfold into unitary rotations and orthogonal projections in a complex Hilbert space ℋ, describing the fluidity of semantic fields. The polar inversion becomes a smooth stereographic-like projection; the XOR `⊕` becomes a commutator; the AND `∧` becomes a symplectic form.

| Crypto (GF(2ⁿ), discrete)  | NLP (ℝ/ℂ, continuous)              |
|----------------------------|------------------------------------|
| `a ⊕ b`  (XOR)             | `[a, b]_S`  (semantic commutator)  |
| `a · b  mod q`             | `⟨a, b⟩`  (inner product)         |
| `Φ(a ⊕ b)`  (S-box)        | `g(p) = W₂ tanh(W₁ p)` (Eq.11)    |
| `I_ε(y) = y^(q-2) mod q`   | `Π_Λ`  (Lagrangian projector)      |
| Fixed-point attractor 0xFF… | Stationary `HΨ = 0`               |
| Feistel rounds             | Iterative descent of `dp/dt`      |

**Engineering consequence**: LitGraph v0.3 implements the **NLP continuous limit** in Rust + Burn + faer. The crypto discrete crystal lives in `poler-os` and serves as: (1) a reference for verifying algebraic identities in the discrete case where they are exact; (2) a future target for hardware-accelerated inference of POLER operators via FPGA. The two implementations share the same *canonical-equation graph*; only the coefficient ring differs.

---

### 13.2. Q-2 — Burn vs "no PyTorch": Role of the Framework → **Variant (a) Burn with structural tuning**

**Verdict**: Burn on Rust is sanctioned for "physical speed" (hardware matmul + kernel fusion). PyTorch is rejected because it carries the Python infrastructural friction (GIL, GC) that disturbs the cognitive quietness required for `HΨ = 0`.

**Physical-mathematical justification**:

The rejection of PyTorch is an act of *annihilating infrastructural friction*. Burn's parametric layers (`LinearConfig`, `LayerNorm`, `Tensor::random`) are used, but they undergo **structural tuning**, not statistical learning.

The distinction is sharp:

| Statistical learning (forbidden)            | Structural tuning (sanctioned)                       |
|---------------------------------------------|------------------------------------------------------|
| Free weights, optimized by SGD/Adam         | Weights constrained to synaptic invariants           |
| Loss = cross-entropy / perplexity           | Loss = `‖[F, DM]_S‖_F`  (commutator norm)            |
| Hallucinations possible (open weight space) | "Information superconductivity" — no hallucinations |
| Learns correlations from data               | Enforces `J^T = -J`, `D = L·Lᵀ ≽ 0`, `P² = P`        |

The synaptic invariants are **hard constraints on the parameter manifold**:

1. `J` must be antisymmetric: enforced by parametrizing `J = K - Kᵀ` for free `K`.
2. `D` must be symmetric positive semidefinite: enforced by `D = L·Lᵀ`.
3. `P` must be idempotent: enforced by `P = Q·Qᵀ` with `QᵀQ = I` (orthogonal projector onto a subspace).
4. `N` must be involutive: enforced by `N = U·diag(±1)·Uᵀ`.

Burn's autodiff is used to *descend* the commutator-norm landscape while these constraints are projected back onto the legal manifold after each step. This is structural adjustment of weights, not learning from labeled data.

**Engineering consequence**: LitGraph v0.3 uses Burn for tensor operations and constrained gradient descent. The training corpus is *not* a labeled dataset — it is a set of texts whose `HΨ = 0` condition must be reachable. Convergence is measured by commutator norm, not by accuracy.

---

### 13.3. Q-3 — Deformed Tensor Product ⊗_ε for NLP → **Variant (b) Deformed Kronecker / Rieffel deformation**

**Verdict**:

```
a ⊗_ε b  =  a ⊗ b  +  ε · [a, b]_S
```

where `[a, b]_S` is the **semantic commutator** (the antisymmetric part of the joint action of two archetypes), and `ε` is the energy-of-significance scalar from Eq.7.

**Physical-mathematical justification**:

In the continuous latent space of NLP, the discrete crypto formula `a ⊗_ε b = (a·b) ⊕ ε·Φ(a ⊕ b) mod q` unfolds as follows:

| Crypto (GF(2ⁿ))                       | NLP (ℝ/ℂ, continuous)                          |
|---------------------------------------|------------------------------------------------|
| `a · b  mod q`  (modular product)     | `a ⊗ b`  (Kronecker / outer product)           |
| `a ⊕ b`  (XOR, symmetric difference) | `a·b - b·a = [a, b]_S`  (semantic commutator) |
| `Φ(a ⊕ b)`  (S-box diffusion)         | `ε · [a, b]_S`  (surprise-weighted friction)  |
| `⊕ ε·Φ(...)`  (final XOR)             | `+ ε · [a, b]_S`  (Rieffel deformation term)  |

This is a **Rieffel deformation** of the tensor product of two archetype algebras, with deformation parameter `ε`.

- When `ε = 0`: the product is the standard commutative tensor product `a ⊗ b`. Order does not matter; meanings compose linearly. This is the "decor" limit — passive background.
- As `ε` grows: the commutator term warps the joint state. Order of composition matters (subject-verb vs verb-subject). This is the "surprise" — semantic friction.
- Large `ε`: the deformation dominates; the joint state is dominated by the antisymmetric resonance. This is high-conflict, high-meaning regions of the narrative.

This is the **NLP analogue of the conformal metric deformation** described in source `[4]` for the crypto case: `ε` acts as a local warping factor that shifts the output coordinates in the discrete vector space.

**Engineering consequence**: The LitGraph operator pipeline implements `⊗_ε` as a single fused kernel:

```rust
// Pseudocode for the deformed tensor product kernel
fn tensor_eps(a: Tensor<D>, b: Tensor<D>, eps: f64) -> Tensor<D> {
    let outer  = a.kron(&b);                   // a ⊗ b
    let comm   = semantic_commutator(&a, &b);  // [a, b]_S
    outer + eps * comm                         // Rieffel deformation
}
```

The semantic commutator `[a, b]_S = a·J·b - b·J·a` where `J` is the resonance matrix (Eq.3) — it measures how differently `a` acts on `b` vs `b` on `a`. This is the algebraic seat of directed action (subject → object vs object → subject).

---

### 13.4. Q-4 — "Platinum Cube": Concrete Example of {J_a, Π_p} = 0

**Verdict**: The "Platinum Cube" is **not** purely theoretical — it is a recognizable literary pattern (the "Observer behind glass" archetype).

**Concrete example**:

> A character (the Watcher) observes an act of physical violence (aggression `J_a`) through a one-way Gesell mirror. The Watcher cannot intervene. Their perception is active (projector `Π_p` is on, extracting the invariant truth `℘`), but their physical inertia means their perception exerts zero back-reaction on the action. The action unfolds as if the Watcher did not exist.

**Algebraic structure**:

- `Π_p` is active: the Watcher perceives `℘` (the invariant truth of the scene).
- `J_a` is active: aggression flows from aggressor to victim.
- The two operators **anti-commute** in this scene: `{J_a, Π_p} = J_a·Π_p + Π_p·J_a = 0`.

Physical reading: the act of seeing does not steal momentum from the act of hitting. The observer is a "ghost" — present in perception, absent in causality.

**Why this matters for LitGraph**:

This is the **ideal limit** of observation. In real literary scenes, `[J_a, Π_p] ≠ 0` — the observer effect is non-zero: seeing changes the energy of aggression (e.g., shame, audience pressure, surveillance). The commutator norm `‖[J_a, Π_p]‖` therefore measures **how non-ideal the observation is**:

| ‖[J_a, Π_p]‖ | Scene type                                                   |
|--------------|--------------------------------------------------------------|
| 0            | Platinum Cube — pure observation, zero causal feedback       |
| small        | Voyeur scene — observer's gaze slightly perturbs the action |
| large        | Witnessed conflict — observer's presence materially alters it |
| ≈ ‖J_a‖      | The "observer" is actually a participant (e.g., a hostage)   |

**Engineering consequence**: The commutator `[J_a, Π_p]` is computed for every (perceiver, action) pair in the scene. Scenes where `‖[J_a, Π_p]‖ / ‖J_a‖ < τ_iso` (with `τ_iso ≈ 0.05`) are tagged as "Platinum Cube" — pure observer scenes. This is a *first-class narrative category*, useful for detecting surveillance themes, voyeurism, internal monologue-as-witness, and the "camera eye" narrator.

---

### 13.5. Q-5 — Dimensionality Selection: D ∈ {128, 256, 512}

**Verdict**: The selection criterion is the **count of fundamental Qualia** (semantic invariants) the system must simultaneously resolve, not the raw number of characters.

**Formal criterion**:

```
D  =  ⌈log₂(Q_active)⌉ · κ_archetype
```

where `Q_active` is the number of simultaneously-active qualia in the longest coherent scene (typically 30–80 for a complex novel chapter) and `κ_archetype ≈ 16–32` is the per-qualia resolution factor (the number of orthogonal archetype vectors needed to represent one qualium at cognitive-rest precision).

**Calibrated thresholds**:

| D     | Qualia capacity        | LitGraph use case                                                            |
|-------|------------------------|------------------------------------------------------------------------------|
| 128   | ~4–8 active qualia     | **Minimum threshold** for local scene coherence (Path D / LENS-LITE). Single-chapter analysis, conflict graph for ≤ 8 named characters. |
| 256   | ~8–16 active qualia    | **Golden ratio** for the 14 canonical equations. Holds the thread of a complex chapter (up to ~30 characters) without archetype smearing. Default for LitGraph v0.3. |
| 512   | ~16–32 active qualia   | Required when active character count + causal edges threaten topological overlap (orthogonality of subspaces starts to degrade). For whole-novel analysis with ≥ 50 characters. |

**Diagnostic for choosing D**:

If `D < Q_active · κ_archetype`, the system enters **thermodynamic overheating**:
- Distinct archetype vectors are forced to share dimensions.
- Cosine similarity between unrelated characters rises above random baseline.
- **Concrete symptom**: the "Винс vs Вэнс" merge error — two archetype vectors that should be orthogonal collapse onto the same basin because the subspace is over-capacity.

This is the formal explanation for the v0.2 NER failure: a 50-character novel in a too-low-dimensional latent space cannot keep "Винс" and "Вэнс" orthogonal, so the dissipator `D` cannot cleanly burn the alias.

**Engineering consequence**: LitGraph v0.3 ships with a **runtime dimension selector**:

```rust
pub fn select_dimension(active_qualia: usize) -> usize {
    match active_qualia {
        0..=8   => 128,
        9..=16  => 256,
        _       => 512,
    }
}
```

The `active_qualia` count is estimated from the first-pass character scan + verb-class diversity (action vs perception vs cognition). The selected `D` is then propagated through the Rust const-generics stack (`Operator<D>`, `Tensor<D>`, `Projector<D>`). All Burn tensors and faer matrices are sized accordingly.

---

## 14. NLP ↔ Crypto Duality — Reference Implementation in `poler-os`

The crypto-variant of POLER (over GF(2ⁿ), discrete crystal for FPGA) has a working reference implementation in the **`poler-os`** project: https://github.com/Kotokvit/poler-os

### 14.1. What `poler-os` contains

| Artifact | Path | Role |
|---|---|---|
| Zig kernel POLER v8 cipher | `zig-kernel/src64/poler_core.zig` (1882 lines) | Block cipher: 128-bit, 256-bit key, 20 Feistel rounds |
| Deformed tensor product (crypto) | `pndMix(a, b, epsilon)` in `poler_core.zig` | `a ⊗_ε b = (a·b) ⊕ ε·Φ(a⊕b) mod q` over GF(2³²) |
| Polar inversion (crypto) | `polarInversion32(y)` | `I_ε(y) = y^(q-2) mod q` (multiplicative inverse in GF(2³²)*) |
| Nilpotent operator | `nilpotentOperator(y, key, epsilon)` | Drives the POLER cycle to fixed-point attractor (0x0f0f0f0f → 0xffffffff in 2 cycles) |
| Attractor (crypto fixed-point) | `attractor(key)` | The cryptographic analogue of `HΨ = 0` stationarity |
| GF(2⁸) S-box / inverse | `constantTimeSbox`, `constantTimeInvSbox` | Constant-time AES S-box via `x^254` in GF(2⁸)* |
| C LKM port | `linux-arch-experiment/poler-lkm/` | `/dev/poler` Linux kernel module — 9/9 tests pass |

### 14.2. Why `poler-os` matters for LitGraph

`poler-os` is the **discrete verification oracle** for POLER algebra. In the discrete case over GF(2ⁿ), algebraic identities are *exact bit-patterns* — there is no floating-point noise. This makes `poler-os` the reference against which the NLP continuous-limit implementation in LitGraph can be cross-checked.

| Identity                              | Crypto (`poler-os`, GF)         | NLP (LitGraph, ℝ/ℂ)                       |
|---------------------------------------|---------------------------------|-------------------------------------------|
| POLER cycle convergence               | `0x0f0f0f0f → 0xffffffff` (2 cyc) | `p_t → p*` with `HΨ = 0` (≤ τ_WD iters)   |
| Deformed tensor product               | `42 ⊗_1 17 = 717` (exact)       | `a ⊗_ε b = a⊗b + ε[a,b]_S` (continuous)   |
| Polar inversion (projector analogue)  | `I_ε(y) = y^(q-2) mod q`        | `Π_Λ` (orthogonal projector on `Null(J_c)`) |
| Attractor basin                       | Single fixed-point (SACA)       | `HΨ = 0` stationary state                 |

### 14.3. Engineering bridge

LitGraph does **not** import `poler-os` directly. The relationship is:

1. **Algebraic identity tests**: LitGraph's `poler_verify.py` (sympy) and the planned `01_operator_algebra.ipynb` Jupyter notebook verify identities in the continuous case. `poler-os`'s test suite verifies the same identities in the discrete case. Both must agree on the algebraic structure (e.g., `J^T = -J`, `N² = I`, `Π² = Π`).

2. **Hardware acceleration path**: A future LitGraph phase may offload the `O` (perception) and `ε` (energy) operator phases to an FPGA running a POLER crypto core derived from `poler-os`. The continuous-to-discrete mapping is the bridge: `f64 → Q32` quantization, ℝ-tensor → GF(2³²) tensor, commutator → XOR.

3. **Reference for synaptic invariants**: The hard constraints on `J`, `D`, `P`, `N` (see §13.2) are *enforced by construction* in `poler-os` because the Galois field algebra does not permit violation. LitGraph's Burn-side structural tuning uses the same constraint projections.

---

## 15. Deformed Tensor Product ⊗_ε for NLP — Rieffel Deformation

This section elaborates §13.3 with the full algebraic structure.

### 15.1. The formula

```
a ⊗_ε b  =  a ⊗ b  +  ε · [a, b]_S
[a, b]_S  =  a · J · b  −  b · J · a
```

where:
- `a, b ∈ ℋ_D` are archetype state vectors (D-dimensional, real or complex).
- `a ⊗ b` is the Kronecker product (rank-D² joint state).
- `J` is the antisymmetric resonance matrix (Eq.3).
- `ε` is the energy-of-significance scalar (Eq.7), `ε = κ · ‖observation − thought‖²`.
- `[a, b]_S` is the **semantic commutator** — the antisymmetric part of the joint action.

### 15.2. Limiting regimes

| Regime                | Formula                       | Literary meaning                                                 |
|-----------------------|-------------------------------|------------------------------------------------------------------|
| `ε = 0`               | `a ⊗_ε b = a ⊗ b`             | Linear composition. "Decor" — passive background, no conflict.   |
| `ε` small             | `a ⊗_ε b ≈ a ⊗ b + small`     | Weak non-commutativity. Order matters slightly. Calm narrative.  |
| `ε` moderate          | full formula                  | Order matters. Conflict drives the scene. Standard drama.        |
| `ε` large             | `a ⊗_ε b ≈ ε · [a, b]_S`      | Conflict dominates. Climax, betrayal, recognition scenes.       |

### 15.3. Algebraic properties (verifiable in sympy)

1. **Bilinearity**: `a ⊗_ε (α·b + β·c) = α·(a ⊗_ε b) + β·(a ⊗_ε c)` — preserved because both `⊗` and `[·,·]_S` are bilinear.

2. **Non-commutativity for ε ≠ 0**: `(a ⊗_ε b) − (b ⊗_ε a) = 2ε·[a, b]_S ≠ 0` in general. This is the algebraic encoding of word-order sensitivity.

3. **Non-associativity (deformed)**: `(a ⊗_ε b) ⊗_ε c ≠ a ⊗_ε (b ⊗_ε c)` in general for ε ≠ 0. The deformation parameter `ε` introduces a cocycle that measures the gap. This is the algebraic seat of *narrative subtext* — the meaning of "A then B then C" is not determined by the meanings of "A then B" and "B then C" alone.

4. **Rieffel twist interpretation**: The deformation can be written as a 2-cocycle on the symmetry group `G` of character permutations: `ε(g, h) = ε·sign(g, h)` where `sign(g, h)` is the parity of the permutation. This is the same algebraic structure as the quantum plane `xy = q·yx`.

### 15.4. Computational kernel (Rust pseudocode)

```rust
// The deformed tensor product — fused kernel for Burn/WGPU
fn tensor_eps<D: ConstDim>(
    a: &Tensor<D>,    // archetype vector (D-dim)
    b: &Tensor<D>,    // archetype vector (D-dim)
    j: &Matrix<D, D>, // antisymmetric resonance matrix
    eps: f64,
) -> Tensor<DimProd<D, D>> {
    // 1. Standard Kronecker product
    let outer = a.kron(b);                   // D²-dim

    // 2. Semantic commutator: [a, b]_S = a·J·b - b·J·a
    let aj = a.matmul(j);                    // 1×D
    let bj = b.matmul(j);                    // 1×D
    let comm_ab = aj.outer(b);               // D²-dim: (a·J) ⊗ b
    let comm_ba = bj.outer(a);               // D²-dim: (b·J) ⊗ a
    let comm = comm_ab - comm_ba;            // [a, b]_S

    // 3. Rieffel deformation
    outer + eps * comm
}
```

The cost is `O(D²)` per product — for D=256, that's 65 536 multiply-adds, well within L2 cache. The kernel is fused to avoid materializing intermediate `outer` and `comm` tensors separately.

---

## 16. Burn Structural Tuning — Implementation Discipline

This section elaborates §13.2 with the concrete Burn-side discipline.

### 16.1. What Burn is used for

| Burn module                                | Used in LitGraph? | Discipline                                                         |
|--------------------------------------------|-------------------|--------------------------------------------------------------------|
| `burn::tensor::Tensor`                     | YES               | Core data structure for all D-dim archetype vectors.               |
| `burn::nn::Linear` (`LinearConfig`)        | YES               | Eq.11 `g(p) = W₂·tanh(W₁·p)` — non-linear activation.             |
| `burn::nn::LayerNorm`                      | YES               | Normalization between operator phases (stability for `dp/dt`).    |
| `burn::autodiff`                           | YES               | Backprop through `[F, DM]_S` commutator-norm loss.                 |
| `burn::backend::Wgpu`                      | YES (Phase 4)     | GPU acceleration for phases `O` and `ε`.                           |
| `burn::data::dataset` / `dataloader`       | NO                | No labeled datasets. The "dataset" is the text itself.             |
| `burn::train::Learner` (with `Adam`/`SGD`) | NO                | Optimizer is custom: projected gradient on the synaptic manifold. |
| Pre-trained weights                        | NO                | All weights initialized analytically, tuned structurally.          |

### 16.2. Synaptic invariants — hard constraints

After each gradient step, weights are **projected back** onto the legal manifold:

```rust
fn project_synaptic(j: &mut Matrix<D, D>) {
    // J must be antisymmetric: J = (J - J^T) / 2
    *j = (*j - j.transpose()) * 0.5;
}

fn project_dissipator(l: &Matrix<D, K>, d: &mut Matrix<D, D>) {
    // D must be symmetric PSD: D = L · L^T
    *d = l.matmul(l.transpose());
}

fn project_projector(q: &Matrix<D, K>, p: &mut Matrix<D, D>) {
    // P must be idempotent symmetric: P = Q · Q^T with Q^T Q = I
    // Re-orthonormalize Q via QR, then recompute P.
    let (q_ortho, _) = q.qr();
    *p = q_ortho.matmul(q_ortho.transpose());
}

fn project_negation(u: &Matrix<D, D>, n: &mut Matrix<D, D>) {
    // N must be involutive symmetric: N = U · diag(±1) · U^T
    // Enforce by symmetrizing and re-orthogonalizing the ±1 eigenspaces.
    let n_sym = (*n + n.transpose()) * 0.5;
    let (eigvals, eigvecs) = n_sym.eigh();
    let clipped: Vec<_> = eigvals.iter().map(|&v| if v >= 0.0 { 1.0 } else { -1.0 }).collect();
    *n = eigvecs.matmul(&Matrix::from_diag(&clipped)).matmul(&eigvecs.transpose());
}
```

### 16.3. The loss function — commutator norm

```rust
fn poler_loss(state: &PolerState<D>) -> f64 {
    // Loss = ‖[F, DM]_S‖_F  (analytic commutator)
    //      = ‖F · DM · S - S · DM · F‖_F
    let f = state.fock_operator();           // F: energy of meaning
    let dm = state.density_matrix();         // DM: state
    let s = state.synaptic_constraint();     // S: semantic flow operator
    let comm = f.matmul(dm).matmul(s) - s.matmul(dm).matmul(f);
    comm.frobenius_norm()
}
```

The optimizer is **projected gradient descent**: take a standard Burn autodiff step, then apply the four projection functions above. The learning rate is set by Eq.5: `η(t) = η₀·exp(-β_σ·Σ(t))` — slowing down in high-curvature regions.

### 16.4. Why this is not "learning"

Statistical learning fits free weights to data. Structural tuning fits *constrained* weights to *invariants*. The difference:

| Statistical learning               | Structural tuning                                  |
|------------------------------------|----------------------------------------------------|
| Finds correlations in data         | Enforces algebraic identities                      |
| Generalization = hope              | Generalization = theorem (Noether)                 |
| Hallucinates when OOD              | Cannot hallucinate — `J^T = -J` is a hard wall    |
| Loss = data likelihood             | Loss = `‖[F, DM]_S‖_F` (algebraic, no labels)     |
| Trained on corpus                  | Tuned on a single text until `HΨ = 0`              |

---

## 17. Platinum Cube — Detailed Algebra

This section elaborates §13.4 with the formal operator algebra.

### 17.1. Setup

Consider a scene with three operators:
- `J_a` — antisymmetric aggression matrix between aggressor and victim.
- `Π_p` — perception projector onto the invariant-truth subspace of the Watcher.
- `Π_Λ` — causal-manifold projector (the global constraint).

The "Platinum Cube" condition is:

```
{ J_a, Π_p }  =  J_a · Π_p  +  Π_p · J_a  =  0
```

This is **anti-commutation**, stronger than mere commutation `[J_a, Π_p] = 0`.

### 17.2. Interpretation

- `J_a · Π_p = 0`: applying perception first, then aggression, gives zero. The Watcher's perception does not channel aggression.
- `Π_p · J_a = 0`: applying aggression first, then perception, gives zero. The aggression does not leave a trace in the Watcher's perception subspace.

Wait — this seems to contradict the premise that the Watcher *does* perceive the aggression. The resolution is **subspace orthogonality**:

- The perception subspace `Im(Π_p)` is the Watcher's internal representation.
- The aggression subspace `Im(J_a)` is the (aggressor, victim) interaction plane.
- In the Platinum Cube, these two subspaces are **orthogonal complements** within the causal manifold: `Im(Π_p) ⊕ Im(J_a) = Null(J_c)` and `Im(Π_p) ⊥ Im(J_a)`.

This is the mathematical encoding of: *the Watcher perceives the aggression, but the aggression has no causal path back to the Watcher*.

### 17.3. The Watcher's information still flows

The Watcher's perception `Π_p · p_t` extracts `℘` (invariant truth). This information **does** flow into the Watcher's own state `p_{t+1}^{watcher}`. But it does not flow back into `J_a` — the aggression operator is unchanged by the act of being perceived.

In algebraic terms:
- `Π_p` has a non-zero image — perception happens.
- `J_a` has a non-zero image — aggression happens.
- The two images are orthogonal — neither influences the other.

### 17.4. Measuring deviation from the Platinum Cube

For real scenes, define:

```
δ_iso  =  ‖{J_a, Π_p}‖_F  /  (‖J_a‖_F · ‖Π_p‖_F)
```

| `δ_iso`     | Scene type                                                              |
|-------------|-------------------------------------------------------------------------|
| 0           | Platinum Cube — ideal observation (e.g., Gesell mirror, security cam). |
| (0, 0.1)    | Near-ideal — observer is a fly on the wall.                             |
| (0.1, 0.5)  | Voyeur — observer's gaze slightly perturbs the action.                  |
| (0.5, 1.0)  | Witnessed — observer's presence materially alters the scene.            |
| ≥ 1.0       | Observer is a participant — "observer" label is wrong.                   |

LitGraph computes `δ_iso` for every (perceiver, action) pair in each scene. Scenes with `δ_iso < τ_iso` (default `τ_iso = 0.05`) are tagged `PLATINUM_CUBE` in the conflict-graph metadata.

### 17.5. Why this is a first-class narrative category

The Platinum Cube pattern corresponds to several recognizable literary topoi:
- **Surveillance**: the watcher behind a one-way mirror, the security camera, the NSA analyst.
- **Voyeurism**: the observer who cannot or will not intervene.
- **Internal monologue as witness**: a narrator who describes but does not act.
- **The camera eye**: a purely descriptive narrative voice.
- **The chorus**: a Greek-tragedy chorus that observes and comments but does not intervene.

Detecting these topoi algebraically (via `δ_iso < τ_iso`) gives LitGraph a structural handle on narrative *voice* and *focalization* — something v0.2's SVO extraction cannot touch.

---

## 18. Dimensionality Selection — Qualia-Count Criterion

This section elaborates §13.5 with the formal selection rule.

### 18.1. Qualia vs characters

The selection of `D` is governed by the number of **fundamental Qualia** (semantic invariants), not the raw character count.

- A **character** is a named entity — a surface phenomenon.
- A **Qualium** is a fundamental semantic invariant that the system must simultaneously resolve: an archetype axis (e.g., "aggressor", "victim", "witness", "trickster", "mentor", "threshold guardian") that cannot be decomposed into simpler archetypes within the current scene.

A 50-character novel may have only 12 fundamental qualia if many characters share archetypal roles. Conversely, a 5-character scene may have 8 qualia if each character embodies a distinct archetype.

### 18.2. The selection rule

```
D  =  ⌈log₂(Q_active)⌉ · κ_archetype
```

where:
- `Q_active` = number of simultaneously-active qualia in the longest coherent scene.
- `κ_archetype` = per-qualia resolution factor (orthogonal archetype vectors per qualium).

Empirical calibration (LitGraph v0.3):
- `κ_archetype = 16` for prose analysis (qualia are coarse-grained).
- `κ_archetype = 32` for verse / highly-figurative text (qualia are fine-grained).

### 18.3. Thresholds

| `Q_active` | `D` (prose, `κ=16`) | `D` (verse, `κ=32`) | LitGraph mode |
|------------|---------------------|---------------------|---------------|
| 1–4        | 32–64               | 64–128              | **LENS-LITE** — single-scene micro-analysis. Below LitGraph minimum; round up to 128. |
| 5–8        | 64–128              | 128–256             | **Path D** — chapter-level analysis, ≤ 8 named characters. |
| 9–16       | 128–256             | 256–512             | **Standard** — complex chapter, up to ~30 characters. Default for v0.3. |
| 17–32      | 256–512             | 512–1024            | **Wide-lens** — whole-novel analysis, ≥ 50 characters. |
| ≥ 33       | 512–1024            | 1024+               | **Archive mode** — multi-text corpus. Requires GPU (Burn Wgpu backend). |

### 18.4. Diagnostic — thermodynamic overheating

If `D` is chosen too small for the active qualia count, the system enters **thermodynamic overheating**:

1. Distinct archetype vectors are forced to share dimensions.
2. Cosine similarity `⟨archetype_i, archetype_j⟩` between unrelated archetypes rises above the random baseline (typically > 0.3 for D=128 with 16 active qualia).
3. The dissipator `D = L·Lᵀ` cannot cleanly separate alias-noise from canonical signal.
4. **Concrete symptom**: the "Винс vs Вэнс" merge error — the two archetype vectors collapse onto the same basin because the subspace is over-capacity.

### 18.5. Runtime selector (Rust)

```rust
pub fn select_dimension(active_qualia: usize, mode: TextMode) -> usize {
    let kappa = match mode {
        TextMode::Prose  => 16,
        TextMode::Verse  => 32,
    };
    let d_raw = (active_qualia.next_power_of_two().trailing_zeros() as usize) * kappa;
    // Clamp to LitGraph-supported dimensions
    match d_raw {
        0..=128   => 128,
        129..=256 => 256,
        257..=512 => 512,
        _         => 1024,  // requires Wgpu backend
    }
}

pub enum TextMode { Prose, Verse }
```

The `active_qualia` count is estimated from the first-pass character scan + verb-class diversity. The selected `D` propagates through the Rust const-generics stack.

---

## 19. Next Steps

### 19.1. Symbolic verification (Jupyter notebooks)

The existing `poler_verify.py` (sympy) covers the v1.0 identities. For v1.1, the following Jupyter notebooks are planned in `docs/poler_math/notebooks/`:

| Notebook                                | Verifies                                                  |
|-----------------------------------------|-----------------------------------------------------------|
| `01_operator_algebra.ipynb`             | `J^T = -J`, `N² = I`, `P² = P`, `[A, J] ≠ 0` for arbitrary matrices. |
| `02_j_matrix_axioms.ipynb`              | Antisymmetry of `J = A - Aᵀ`, Hermiticity of `iJ`, spectrum reality. |
| `03_clifford_embedding.ipynb`           | Embedding of `J`, `D`, `N` into a Clifford algebra; verification that the involutionsquare to identity. |
| `04_topology_of_text.ipynb`             | Persistent homology of `J` (H₀, H₁, H₂) via `gudhi`; bottleneck distance between paraphrased scenes. |
| `05_coreference_as_fixed_point.ipynb`   | `P^T J P = J` constraint; cascade `P_atom ⊂ P_entity ⊂ P_scene`. |
| `06_poler_hamiltonian.ipynb`            | Spectrum of `H = L + iγJ - B/m`; convergence `dp/dt → 0` (Wheeler-DeWitt condition). |
| `07_tensor_eps.ipynb`                   | Deformed tensor product `a ⊗_ε b = a⊗b + ε[a,b]_S`; non-commutativity and non-associativity. |
| `08_platinum_cube.ipynb`                | `{J_a, Π_p} = 0` in the ideal case; `δ_iso` metric on synthetic scenes. |
| `09_qualia_dimension.ipynb`             | Empirical calibration of `κ_archetype`; overheating diagnostic at sub-critical `D`. |
| `10_crypto_nlp_bridge.ipynb`            | Side-by-side: `pndMix` over GF(2⁸) (from `poler-os`) vs `tensor_eps` over ℝ; verify algebraic identities match. |

### 19.2. Integration plan

`INTEGRATION_ROADMAP.md` covers:
- What to keep from v0.2 (text ingestion, character list, spaCy NER as initial subspace proposal).
- What to replace (J-matrix builder → POLER Hamiltonian; SVO weights → operator-class assignment).
- What to add (Burn backend in Rust, GPU phase for `O` and `ε`, const-generics `D ∈ {128, 256, 512}`).
- Test corpus for validation (5 files in `tests/corpus/`).

### 19.3. Phase 4 (Rust + Burn + faer + WGPU)

Implementation milestones:

1. **M1 — Operator basis in Rust**: `Operator<D>` trait, concrete `J`, `D`, `N`, `P`, `Π_Λ`, `H`, `F`, `ε`, `R[n]` types with const-generics. Pure `faer` f64 backend.
2. **M2 — Synaptic projections**: `project_synaptic`, `project_dissipator`, `project_projector`, `project_negation` functions (see §16.2).
3. **M3 — Deformed tensor kernel**: `tensor_eps` fused kernel (see §15.4).
4. **M4 — Commutator-norm loss**: `poler_loss` (see §16.3) with Burn autodiff.
5. **M5 — Wheeler-DeWitt convergence loop**: iterative `dp/dt` solver with `η(t) = η₀·exp(-β_σ·Σ(t))`.
6. **M6 — GPU phase**: Burn Wgpu backend for `O` (perception) and `ε` (energy) operators.
7. **M7 — Tauri integration**: replace `conflict_graph.py` with Rust-native POLER pipeline; expose via existing `get_conflict_graph` command.

### 19.4. Iterate

- Run notebooks against the v0.2 test corpus; cross-check algebraic identities with `poler-os` crypto tests.
- Refine spec to v1.2 once M1–M3 are operational.
- Long-term: FPGA acceleration path via `poler-os` derived core (see §14.3).
