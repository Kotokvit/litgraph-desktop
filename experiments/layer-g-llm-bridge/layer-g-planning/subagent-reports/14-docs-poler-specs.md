# Subagent 14: POLER Specification Documents (Canonical + v7.5 Audit + Layer F)

> **Scope owner**: Cross-cutting spec-vs-code alignment analysis of the three POLER specification documents.
> **Files inspected**: 3
>   - `litgraph-desktop/POLER_EPSILON_CANONICAL_SPECIFICATION.md` (262 LOC, v6.5.0-EMPIRICAL-BENCHMARK)
>   - `litgraph-desktop/POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md` (151 LOC, 2026-08-10)
>   - `litgraph-desktop/POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` (801 LOC, v8.0-CANONICAL)
> - Cross-referenced against: `litgraph-core/src/parser/epsilon.rs` (1002 LOC, v7.5-LEM Canonical), subagent reports 01–03, worklog `c143a7f` (Layer F.2 ship), `POLER_UA_LP_MASTER_ROADMAP_V8.md`, `docs/layer-g-planning/00-META_PROMPT.md`.

---

## 1. Scope
- **Spec lineage**: canonical (v6.5.0) → v7.0-LEM (lemmatization) → v7.5-LEM (SVO integration + audit fixes) → v8.0 Layer F (frontend). All three documents are dated 2026-08-10.
- **Cross-cutting role**: These three docs are the canonical mathematical/architectural contract for POLER Engine $\Psi$. Every constant, formula, threshold, and DTO field in production code should trace back to one of these documents.
- **Read path**: Read full text of all three docs + grep of `litgraph-core/src/parser/epsilon.rs` for constant definitions and v7.5 audit fixes + cross-check against subagent reports 01 (`litgraph-core/src/linguistic/*`), 02 (`litgraph-core/src/parser/epsilon.rs`), 03 (`litgraph-core/src/reasoning/*`).

---

## 2. Atomic Inventory

### 2.1 Documents / Sections
| Document | Section(s) | LOC | Purpose | Authority Level |
|---|---|---|---|---|
| `POLER_EPSILON_CANONICAL_SPECIFICATION.md` | §1–10 | 262 | Empirical benchmark + canonical $\varepsilon$ formula + SymPy calculus + §9 spec-vs-impl divergences (B1–B8) + v7.0-LEM roadmap | **Canonical math source** |
| `POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md` | §1–4 + SymPy + verification | 151 | Audit of commits `714b49d` + `90f5bd4`; identified 4 critical discrepancies + 3 doc drifts in `epsilon.rs` v7.5-LEM | **Audit log** (historical) |
| `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` | §1–8 | 801 | Blueprint for Tauri IPC commands (F.1) + React Visualizer (F.2); DTO schemas, component specs, design system | **Frontend blueprint** |

### 2.2 Critical Mathematical Constants (canonical values)
| Constant | Symbol | Canonical Value | Spec Source | Code Location | Status |
|---|---|---|---|---|---|
| Climax Threshold | — | **7.5** | Canonical §2.3 (95th percentile ≈ 7.5252) | `epsilon.rs:140` `CLIMAX_THRESHOLD: f64 = 7.5` | ✅ Aligned |
| Bias delta | $\delta_{\text{bias}}$ | **15.0** | Canonical §4.1 + §8.3 (Nelder-Mead optimum) | `epsilon.rs:133` `DELTA_BIAS: f64 = 15.0` | ✅ Aligned |
| Base threshold | $\theta_{\text{base}}$ | **3.5** | Canonical §4.1 + §8.3 (Nelder-Mead optimum) | `epsilon.rs:137` `THETA_BASE: f64 = 3.5` | ✅ Aligned |
| Emotion multiplier (E term) | — | **1.5** × emotion_count | Canonical §4.1 ($E = 1.5 \times emotion\_count$) | `epsilon.rs:416` `let e_val = 1.5 * emotion_count as f64;` | ✅ Aligned |
| Canon anchor multiplier | — | **3.0** × canon_count | Canonical §4.1 ($C_{canon} = 3.0 \times canon\_count$) | `epsilon.rs:417` `let c_canon = 3.0 * canon_count as f64;` | ✅ Aligned |
| SVO action factor | $\kappa_{SVO}$ | **2.0** | v7.5 Audit §1 (CD2 fix) | `epsilon.rs:431` `2.0 * svo_validated_weight` | ✅ Aligned |
| Sector coefficient | $\kappa$ | **1.0** default, **1.2** Sector Four, **0.85** Lower City | Canonical §B3 | `epsilon.rs:329` (function param) | ✅ Aligned |
| Emotion climax weight | $\gamma_{emo}$ | **1.0** (was 1.5 in spec §3) | Canonical §B1 resolution | `epsilon.rs:144` `GAMMA_EMO: f64 = 1.0` | ✅ Aligned (per B1 resolution) |
| Conflict climax weight | $\lambda_{conf}$ | **12.5** | `POLER_UA_LP_MASTER_ROADMAP_V8.md:122` + `epsilon.rs:32` docstring | `epsilon.rs:147` `LAMBDA_CONF: f64 = 12.5` | ✅ Aligned (NOT in canonical spec; declared in roadmap + code) |
| Rarity clamp min | — | **0.1** | Canonical §4.1 + §B5 | `epsilon.rs:150` `RARITY_MIN: f64 = 0.1` | ✅ Aligned |
| Rarity clamp max | — | **4.5** | Canonical §4.1 + §B5 | `epsilon.rs:153` `RARITY_MAX: f64 = 4.5` | ✅ Aligned |

### 2.3 Canonical Formulas (Spec → Code Trace)

#### 2.3.1 Canonical $\varepsilon$ formula (Canonical §4.1)
$$\varepsilon = \frac{\kappa \cdot I_{kw} \cdot \sum_{w \in U} rarity(w) + E + C_{canon} + A_{SVO}}{\sqrt{|U| + \delta_{bias}}}$$

- $I_{kw} = 1 + \ln(1 + kw\_count)$ → `epsilon.rs:397` ✅
- $rarity(w) = -\log_{10}(p_w)$ clamped $[0.1, 4.5]$ → `epsilon.rs:297` (uses `.log10()` per B5 fix) ✅
- $E = 1.5 \cdot emotion\_count$ → `epsilon.rs:416` ✅
- $C_{canon} = 3.0 \cdot canon\_count$ → `epsilon.rs:417` ✅
- $A_{SVO} = 2.0 \cdot svo\_validated\_weight$ (SVO non-empty) OR $2.0 \cdot action\_count$ (fallback) → `epsilon.rs:430-434` ✅
- $\theta_{rel}(\kappa) = 3.5/\kappa$ → `epsilon.rs:440` ✅
- Climax flag: $\varepsilon \ge 7.5$ → `epsilon.rs:442` ✅

#### 2.3.2 Climax $\varepsilon_{climax}$ formula (Canonical §3 / v7.5 Audit §1)
$$\varepsilon_{climax} = \frac{\kappa \cdot I_{loc} \cdot \bar{d}^2 + \gamma_{emo} \cdot E + \lambda_{conf} \cdot \Omega_{conf}}{\ln(e + |U|)}$$

- $I_{loc} = 1 + canon\_count\_in\_chapter$ (no longer hardcoded to 1.0) → `epsilon.rs:544` ✅
- $\bar{d}^2 = (\overline{rarity})^2$ → `epsilon.rs:572-576` ✅
- $\gamma_{emo} = 1.0$ (per B1) → `epsilon.rs:144` ✅
- $\lambda_{conf} = 12.5$ → `epsilon.rs:147` ✅
- $\Omega_{conf}$ from `ConflictAnalyzer.analyze_chapter(...)` → `epsilon.rs:546` ✅
- Denominator $\ln(e + |U|)$ → `epsilon.rs:607` `((std::f64::consts::E + u_len as f64).ln())` ✅
- Asymptotic limit $\lim_{u\to\infty} \varepsilon_{climax} = 0$ → SymPy proof in Canonical §8.1.3; code monotonic decay preserved ✅

---

## 3. Current State — Spec-vs-Code Alignment

### 3.1 v7.5 Audit Findings vs Current `epsilon.rs` (HEAD)
The audit document (dated 2026-08-10) flagged **4 critical discrepancies** + **3 documentation drifts** in commits `714b49d` and `90f5bd4`. Cross-checking against current `litgraph-core/src/parser/epsilon.rs`:

| # | Audit Finding | Severity | Audit-Proposed Fix | Current Code State | Status |
|---|---|---|---|---|---|
| CD1 | Additive double counting in $A_{SVO}$: `(2.0 * action_count) + svo_validated_weight` inflated action verbs twice | 🔴 Critical | Replace `action_count` entirely with SVO-validated triplets | `epsilon.rs:430-434` uses `if !svo_triplets.is_empty() { 2.0 * svo_validated_weight } else { 2.0 * action_count }` — REPLACEMENT semantics | ✅ **Fixed** |
| CD2 | Uncalibrated heuristic weights 2.5/1.5 (transitive vs intransitive) lacked SymPy proof + Nelder-Mead ground truth | 🔴 Critical | Use fixed canonical coefficient 2.0 | `epsilon.rs:431` uses fixed `2.0 * svo_validated_weight` (no 2.5/1.5 distinction) | ✅ **Fixed** |
| CD3 | Negated verbs (`polarity == false`) boosted $A_{SVO}$ identically to affirmative | 🔴 Critical | Apply indicator $\mathbb{I}(\text{polarity} == \text{true})$ filter | `epsilon.rs:423-427` `.filter(\|t\| t.polarity).map(\|t\| t.confidence).sum()` | ✅ **Fixed** |
| CD4 | Unfiltered homonym nouns in `ACTION_VERBS` fallback (e.g., `"мати"`, `"діти"`) counted as action verbs | 🔴 Critical | Filter via Layer B `PosTagger` | `epsilon.rs:432-433` — fallback path `2.0 * action_count` STILL uses raw `ACTION_VERBS` string matching without POS filter. **CD4 only partially fixed**: when SVO parser extracts triplets, action_count is bypassed; but when SVO returns empty (rare token, parser failure), the homonym bug reappears | 🟡 **Partially Fixed** |
| DD1 | `epsilon.rs:1` header read "v7.0-LEM Canonical" instead of "v7.5-LEM Canonical" | 🟡 Drift | Update header | `epsilon.rs:1` now reads `"Epsilon-алгоритм важності фрагмента тексту (POLER v7.5-LEM Canonical)"` | ✅ **Fixed** |
| DD2 | `compute_epsilon_climax()` lacked SVO interaction docs | 🟡 Drift | Add doc | `epsilon.rs:460-499` has full docstring covering analyzer integration, I_loc, Ω_conf | ✅ **Fixed** |
| DD3 | `compute_epsilon_lemmatized()` didn't pass lemmatized tokens to SVO parser | 🟡 Drift | Pipe lemmatized tokens | `epsilon.rs:420` still calls `SvoParser::new().parse_text(chapter_text)` on RAW text — SVO parser internally lemmatizes, but the ε_canonical tokenization (line 374-388) and SVO tokenization are NOT shared | 🟡 **Not Fixed** — subagent 02 confirmed: "climax path doesn't lemmatize even when called via compute_epsilon_climax_with_analyzer" |

### 3.2 v7.5 Audit Unit Tests vs Current Code
The audit proposed 3 specific unit tests. All 3 are present in current code:
- `test_negated_verb_does_not_boost_a_svo` (`epsilon.rs:797`) — asserts ε difference = $2.0/\sqrt{18} \approx 0.4714$ between affirmative and negated
- `test_homonym_noun_not_counted_as_action_verb` (`epsilon.rs:823`) — asserts `"мати бачить сина"` extracts triplet with polarity=true (bypassing ACTION_VERBS)
- `test_svo_replaces_action_count_no_double_counting` (`epsilon.rs:859`) — asserts ε ≈ 1.61 (replacement) vs ε ≈ 2.09/2.20 (additive bug)

→ **Audit verification plan: 100% executed.**

---

## 4. Layer F Spec vs Implementation (commit `c143a7f`)

### 4.1 What the Layer F Spec Prescribed
The 801-LOC spec is structured as:
- §1: System vision + "Validate by Using" imperative (Layer F before Layer G)
- §2: End-to-end architecture diagram (A→B→C→D→E→F.1→F.2)
- §3: Mathematical recap of Layers A–E
- §4: Rust DTO schemas (`SvoTripletDto`, `ParadoxDto`, `EpsilonClimaxDto`) + TypeScript API bindings
- §5: HSL visual token system (glassmorphic dark UI)
- §6: Component blueprints for `SvoHighlighter.tsx` and `PolerPanel.tsx` (full source)
- §7: Performance strategies (regex-free tokenization, lazy tab loading)
- §8: 5-task implementation table (F.2.1 through F.2.5)

### 4.2 What `c143a7f` Actually Shipped (per worklog line 344-389)
- ✅ F.2.1 `src/lib/tauri-commands.ts` — POLER DTO interfaces + IPC functions added
- ✅ F.2.2 `src/components/litgraph/SvoHighlighter.tsx` — Created
- ✅ F.2.3 `src/components/litgraph/PolerPanel.tsx` — Created (3-tab modal)
- ✅ F.2.4 Toolbar integration in `LitApp.tsx`
- ⏳ F.2.5 Verification on `sfera.md`/`kasiopia.md` — Pending

### 4.3 Spec-vs-Implementation Divergences

#### 4.3.1 🟡 TypeScript DTO field naming (CRITICAL — pre-known, was the audit trigger for `c143a7f`)
Per worklog line 350: *"Discovered spec's TS DTOs differ from actual Rust DTOs (spec said totalParadoxes/chapterBreakdowns/frobeniusNorm/noiseFiltered, but Rust sends paradoxes/chapters/omegaConf/isNoise). Used Rust DTOs as source of truth."*

| Field | Spec §4.2 Says | Rust `poler.rs` Actually Sends | Resolution |
|---|---|---|---|
| Paradox report root | `totalParadoxes: number` + `paradoxes[]` + `chapterBreakdowns[]` | `paradoxes: Vec<ParadoxDto>` + `chapters: Vec<ChapterBreakdownDto>` | Rust wins (no `totalParadoxes`; field name `chapters` not `chapterBreakdowns`) |
| Conflict metric | `frobeniusNorm: number` | `omegaConf: f64` (Frobenius is named "omegaConf" in Rust DTO `EpsilonClimaxDto.omega_conf`) | Rust wins (renamed in TS impl) |
| Noise flag | `noiseFiltered: boolean` | `isNoise: bool` (Rust `EpsilonResult.is_noise`) | Rust wins (renamed) |

**Decision documented**: Per `docs/layer-g-planning/00-META_PROMPT.md:159` constraint #3 — *"Source of truth: Rust DTOs > spec docs (when they disagree)"*.

#### 4.3.2 🔴 Layer F §3.4 Climax Formula is Mathematically WRONG (diverges from canonical spec)
Layer F spec §3.4 (line 132) defines:
$$\varepsilon_{climax} = 2.0 \cdot A_{SVO}^{validated} + 1.5 \cdot \Omega_{conf} + 1.2 \cdot I_{loc}$$

This is **mathematically inconsistent** with the canonical climax formula (Canonical §3 / v7.5 Audit §1):
$$\varepsilon_{climax} = \frac{\kappa \cdot I_{loc} \cdot \bar{d}^2 + \gamma_{emo} \cdot E + \lambda_{conf} \cdot \Omega_{conf}}{\ln(e + |U|)}$$

Divergences:
1. **No division by $\ln(e + |U|)$** → Layer F formula has no length normalization, so long fragments would explode to infinity (Canonical §8.1.3 SymPy proof: $\lim_{u\to\infty} \varepsilon_{climax} = 0$ — VIOLATED)
2. **No $\kappa$ sector scaling** → Layer F formula doesn't apply Sector Four (1.2×) or Lower City (0.85×) multipliers
3. **Wrong coefficient on $\Omega_{conf}$** — Layer F uses 1.5; canonical uses $\lambda_{conf} = 12.5$
4. **Wrong coefficient on $I_{loc}$** — Layer F uses 1.2; canonical uses $\kappa \cdot \bar{d}^2$ (variable)
5. **Includes $A_{SVO}$ in $\varepsilon_{climax}$** — Canonical §B2 explicitly says: *"$\varepsilon_{climax}$ contains $\Omega_{conf}$, but does NOT contain $C_{canon} + A_{SVO}$"*
6. **Missing $\gamma_{emo} \cdot E$ term** — Layer F formula has no emotion component at all
7. **$\bar{d}^2$ missing** — Canonical has $\kappa \cdot I_{loc} \cdot \bar{d}^2$ (rarity-squared intensity); Layer F replaces with constant $1.2 \cdot I_{loc}$

**Code state**: `epsilon.rs:607-612` implements the CANONICAL formula (with $\ln$ denominator, $\kappa$, $\gamma_{emo}$, $\lambda_{conf}=12.5$, no $A_{SVO}$). Code is correct; Layer F spec §3.4 is wrong.

→ **Action needed**: Patch Layer F spec §3.4 to match canonical formula. The spec's other sections (DTO schemas, component blueprints) are still valid; only the math recap is broken.

#### 4.3.3 🔴 Layer F §3.4 θ_rel Formula Diverges from Canonical
Layer F §3.4 (line 139): $\theta_{rel} = \frac{1.5}{\sqrt{word\_count}}$

Canonical §4.1 + §B4: $\theta_{rel}(\kappa) = \frac{3.5}{\kappa}$

Code `epsilon.rs:440`: `let theta_rel = THETA_BASE / kappa;` (matches canonical, NOT Layer F)

→ **Three-way divergence** on $\theta_{rel}$. Code follows canonical; Layer F spec is wrong.

#### 4.3.4 🟡 I_loc Definition Diverges Between Specs
- Layer F §3.4: $I_{loc} = 1 + \ln(1 + canon\_count)$
- Canonical spec: not explicitly defined (only mentioned in v7.5 Audit §1 derivation)
- v7.5 Audit §1: $I_{loc}$ passed as parameter, derived from canon anchors
- Code `epsilon.rs:544`: `let i_loc = 1.0 + canon_count_in_chapter as f64;` (linear, not log)

→ Code uses **linear** $I_{loc} = 1 + canon\_count$; Layer F spec prescribes **logarithmic** $I_{loc} = 1 + \ln(1 + canon\_count)$. This is a minor discrepancy but affects climax scoring for chapters with many canon anchors (e.g., 10 anchors → linear=11 vs log=3.4).

#### 4.3.5 ✅ Component Blueprints Used Verbatim
The `SvoHighlighter.tsx` and `PolerPanel.tsx` source code in Layer F spec §6.1 and §6.2 was used as the direct basis for `c143a7f`. Per worklog line 385 (commit `c143a7f`), the components shipped successfully. No divergence reported in component structure, prop signatures, or HSL token system.

---

## 5. Mathematical Rigor Audit

### 5.1 Frobenius Norm $\|A\|_F$
- **Layer F §3.4 definition**: $\Omega_{conf} = \|A_{POS}\|_F = \sqrt{\sum_{i=1}^n \sum_{j=1}^n |a_{ij}|^2}$
- **Canonical §B7 definition**: $\Omega_{conf}(C) = \sum_{P \neq C} |J(C,P)| \cdot A(C,P)$ — **per-character conflict sum**, not matrix norm
- **Code (subagent 03 `mod.rs:138`)**: `frobenius_norm(matrix) = (a_ij²).sum().sqrt()` — matches Layer F, NOT canonical

→ **Spec-vs-spec divergence**: Layer F and Canonical disagree on what $\Omega_{conf}$ IS. Layer F treats it as the matrix Frobenius norm (scalar); Canonical treats it as a per-character quantity (vector). Code follows Layer F.

### 5.2 Spectral Radius $\rho(A_{POS})$
- **Layer F §3.5**: Power Iteration + Rayleigh quotient: $v^{(k+1)} = Av^{(k)}/\|Av^{(k)}\|_2$, $\rho = \lim \frac{v^T A v}{v^T v}$
- **Canonical §8.2**: $\rho(A) = \max_i |\lambda_i| = 41.5212$ (computed via NumPy SVD, not power iteration, but mathematically equivalent for symmetric matrices)
- **Code (subagent 03 `mod.rs:163`)**: `spectral_radius_power_iteration` — Power Iteration + Rayleigh quotient, returns `lambda.max(0.0)` (Perron-Frobenius clamp)

→ Code matches Layer F. Canonical used SVD as ground truth; both are mathematically correct for non-negative symmetric matrices. Code's `lambda.max(0.0)` would mask negative eigenvalues for signed matrices, but for adjacency matrices this is fine (subagent 03 noted this).

### 5.3 $\theta_{rel}$ — Three Definitions (see §4.3.3)
Code follows Canonical; Layer F diverges.

### 5.4 log10 vs ln for `rarity(w)`
- **Canonical §B5**: standardize on $\log_{10}$ for `rarity(w)`; $\ln$ for $I_{kw}$, $I_{loc}$, $\ln(e + |U|)$
- **Code `epsilon.rs:297`**: `-(p_w.max(1e-10).log10())` ✅ (B5 fix applied)
- **Code `epsilon.rs:397`**: `1.0 + (1.0_f64 + kw_count as f64).ln()` for $I_{kw}$ ✅
- **Code `epsilon.rs:607`**: `(std::f64::consts::E + u_len as f64).ln()` for climax denominator ✅

→ B5 bug (previously used `-(p.ln())` instead of `-log10(p)`) is FIXED. All log usages match canonical.

### 5.5 Asymptotic Stability
- **Canonical §8.1.3 SymPy proof**: $\lim_{u \to \infty} \varepsilon = 0$, $\lim_{u \to \infty} \varepsilon_{climax} = 0$
- **Code**: $\varepsilon$ divides by $\sqrt{|U| + 15} \to \infty$ → 0 ✅; $\varepsilon_{climax}$ divides by $\ln(e + |U|) \to \infty$ → 0 ✅
- Partial derivative $\partial \varepsilon / \partial u < 0$ (monotonic decay) — preserved ✅

### 5.6 Partial Derivative Consistency (Canonical §8.1)
- $\frac{\partial \varepsilon_{climax}}{\partial \Omega_{conf}} = \frac{\lambda_{conf}}{\ln(e + u)} > 0$ — code uses $\lambda_{conf} = 12.5$ in numerator, $\ln(e + u)$ in denominator ✅
- Sensitivity to POS accuracy: $\frac{\partial \varepsilon}{\partial \mu_{pos}} = \frac{2.0 \cdot N_{homonyms}}{\sqrt{|U| + \delta_{bias}}}$ — verified by audit §2; code's polarity filter + 2.0 SVO coefficient match this derivation ✅

---

## 6. Layer G Hints Across the Three Specs

### 6.1 Direct Mentions in Scope
| Document | Mentions Layer G? | Location | Quote |
|---|---|---|---|
| `POLER_EPSILON_CANONICAL_SPECIFICATION.md` | ❌ No | — | (No reference to Layer G, LLM Reasoning Bridge, or hypotheses) |
| `POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md` | ❌ No | — | (Pure math/code audit; no Layer G context) |
| `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` | ✅ Yes (2 mentions) | §1.2 line 47, §1.2 line 50 | See below |

### 6.2 Extracted Layer G Requirements (from Layer F §1.2 "Validate by Using Imperative")
1. **Layer G name**: "LLM Reasoning Bridge" (line 47)
2. **Layer G ordering constraint**: Layer F (React Visualizer) MUST be developed BEFORE Layer G — fundamental software engineering requirement
3. **Three rationales for F-before-G ordering**:
   - **User-in-the-Loop Visibility**: Symbolic engine is useless if literary analysts cannot inspect outputs. Visualizing $\varepsilon$, SVO triplets, temporal paradoxes allows human inspection of parsing quality.
   - **Empirical Calibration of Anomaly Thresholds**: Before prompting LLMs to resolve temporal paradoxes (Layer G), humans must see which text patterns trigger false positives vs true narrative errors (`Dead-Speaking`, `Spatial Teleportation`).
   - **API Ergonomics Validation**: Integrating Tauri Rust DTOs directly into React components verifies data payload structures, serialization speed, camelCase conversion accuracy, and component state flows.
4. **Layer G scope** (implicit from §1.2 + §2 architecture diagram): consumes Layer E outputs (paradoxes, $\Omega_{conf}$, $\rho(A_{POS})$) + Layer D $\varepsilon_{climax}$ → invokes LLM to propose hypotheses (`Flashback`, `Dream`, `Resurrection`, `Impostor` per subagent 03)

### 6.3 External Context (Cross-References)
- `POLER_UA_LP_MASTER_ROADMAP_V8.md:143` — "Layer G: LLM Reasoning Bridge & Hypothesis Verifier" (full Layer G definition outside this subagent's scope)
- `docs/layer-g-planning/00-META_PROMPT.md:14` — "Layer G (LLM Reasoning Bridge) implementation plan" (planning scaffold exists)
- `litgraph-core/src/reasoning/mod.rs:18` — code docstring: "Future LLM-backed implementations (Layer G)"
- `litgraph-core/src/reasoning/paradox.rs:23` — code docstring: "they are *signals* that Layer G (LLM Reasoning) should resolve by proposing [hypotheses]"

→ **Layer G is referenced 4× in code docstrings and 2× in Layer F spec, but NEVER in the Canonical or v7.5 Audit specs.** The Canonical spec focuses purely on math; the v7.5 Audit focuses on code-level bug fixes. Layer G planning context lives in `docs/layer-g-planning/` and `POLER_UA_LP_MASTER_ROADMAP_V8.md` — outside the 3 documents in this subagent's scope.

---

## 7. Gaps / Bugs / Spec Drift

### 7.1 [SPEC-DRIFT P0] Layer F §3.4 Climax Formula is Mathematically Wrong
- **Location**: `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md:132`
- **Issue**: Formula $\varepsilon_{climax} = 2.0 \cdot A_{SVO} + 1.5 \cdot \Omega_{conf} + 1.2 \cdot I_{loc}$ lacks $\ln(e+|U|)$ denominator, $\kappa$, $\gamma_{emo} \cdot E$, $\bar{d}^2$; introduces $A_{SVO}$ which Canonical §B2 explicitly forbids in climax
- **Code impact**: NONE — code follows canonical correctly
- **Documentation impact**: HIGH — any engineer reading Layer F spec as the source of truth would implement the wrong formula
- **Fix**: Replace Layer F §3.4 formula with canonical climax formula (see §2.3.2 above)

### 7.2 [SPEC-DRIFT P0] Layer F §3.4 θ_rel Formula Diverges
- **Location**: `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md:139`
- **Issue**: $\theta_{rel} = 1.5/\sqrt{word\_count}$ vs Canonical $\theta_{rel} = 3.5/\kappa$
- **Fix**: Replace with $\theta_{rel}(\kappa) = 3.5/\kappa$

### 7.3 [SPEC-DRIFT P1] Layer F §3.4 I_loc Formula Diverges from Code
- **Location**: `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md:137`
- **Issue**: Spec says $I_{loc} = 1 + \ln(1 + canon\_count)$; code uses linear $I_{loc} = 1 + canon\_count$
- **Fix**: Either patch spec to linear, or patch code to logarithmic (canonical math doesn't dictate — pick one and document)

### 7.4 [SPEC-DRIFT P0] Layer F §3.4 Ω_conf Definition Diverges from Canonical §B7
- **Layer F says**: $\Omega_{conf} = \|A_{POS}\|_F$ (Frobenius norm, scalar)
- **Canonical §B7 says**: $\Omega_{conf}(C) = \sum_{P \neq C} |J(C,P)| \cdot A(C,P)$ (per-character, vector)
- **Code follows Layer F** (Frobenius norm)
- **Fix**: Update Canonical §B7 to acknowledge Layer F's Frobenius interpretation as the canonical one (since code already uses it); OR add a separate field for per-character directed conflict for future "aggressor/victim" UI

### 7.5 [CODE-DRIFT P1] v7.5 Audit CD4 (Homonym Filter) Only Partially Fixed
- **Location**: `epsilon.rs:432-433`
- **Issue**: When SVO parser returns empty (rare token, parser failure, single-token input), code falls back to `2.0 * action_count` which still uses raw `ACTION_VERBS` string matching WITHOUT `PosTagger` filtering. Homonym nouns like `"мати"`, `"діти"` can still be counted as action verbs in the fallback path.
- **Fix**: In fallback, filter `action_count` through `pos_tagger::tag_word(w, &candidates)` and only count tokens where `selected_tag.class == PosClass::Verb`

### 7.6 [CODE-DRIFT P2] v7.5 Audit DD3 (Lemmatized Token Pipeline) Not Fixed
- **Location**: `epsilon.rs:420`
- **Issue**: `compute_epsilon_climax_inner` calls `SvoParser::new().parse_text(chapter_text)` on RAW chapter text, not on the lemmatized token stream used by the canonical ε path. Subagent 02 confirmed: "climax path doesn't lemmatize even when called via compute_epsilon_climax_with_analyzer"
- **Symmetry break**: Canonical ε has `compute_epsilon_lemmatized` variant (uses lemmatizer); climax has no such variant
- **Fix**: Add `use_lemmatizer: bool` parameter to `compute_epsilon_climax_inner`; pass lemmatized tokens to `SvoParser::extract_triplets` instead of re-tokenizing raw text

### 7.7 [DOC-MISSING P1] Canonical Spec Doesn't Mention $\lambda_{conf} = 12.5$
- **Location**: `POLER_EPSILON_CANONICAL_SPECIFICATION.md` — no explicit value for $\lambda_{conf}$
- **Issue**: Code uses `LAMBDA_CONF: f64 = 12.5` (also in `POLER_UA_LP_MASTER_ROADMAP_V8.md:122`), but the canonical spec doc never states this value. Anyone reading only the canonical spec would not know $\lambda_{conf} = 12.5$.
- **Fix**: Add §4.2 to canonical spec: "Constants: $\gamma_{emo} = 1.0$ (per B1), $\lambda_{conf} = 12.5$, $\delta_{bias} = 15.0$, $\theta_{base} = 3.5$"

### 7.8 [DOC-MISSING P2] Canonical Spec Doesn't Define $I_{loc}$ Formula
- **Location**: `POLER_EPSILON_CANONICAL_SPECIFICATION.md` — $I_{loc}$ mentioned in §8.1.2 derivative but never explicitly defined
- **Issue**: Code uses $I_{loc} = 1 + canon\_count$ (linear); Layer F says $I_{loc} = 1 + \ln(1 + canon\_count)$ (log). Canonical spec is silent.
- **Fix**: Pick one (recommend linear, matches code) and add explicit definition to canonical spec §4.1

### 7.9 [DOC-DRIFT P2] Layer F DTO Schemas Still Show Wrong Field Names
- **Location**: `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md:308-330`
- **Issue**: Spec §4.2 TS interfaces still show `totalParadoxes`, `chapterBreakdowns`, `frobeniusNorm`, `noiseFiltered` — but `c143a7f` implemented using Rust DTOs as source of truth (`paradoxes`, `chapters`, `omegaConf`, `isNoise`)
- **Fix**: Patch Layer F §4.2 to match Rust DTOs (since `docs/layer-g-planning/00-META_PROMPT.md:159` declares Rust as source of truth)

---

## 8. Refactoring Opportunities

### 8.1 [REFACTOR] Unify $\theta_{rel}$ across all 3 specs
- Currently 2 definitions: Canonical $3.5/\kappa$ vs Layer F $1.5/\sqrt{word\_count}$
- Pick canonical ($3.5/\kappa$, matches code) — patch Layer F §3.4
- Effort: **S** (15 min doc edit)

### 8.2 [REFACTOR] Add a constants table to canonical spec §4
- Currently constants are scattered: $\delta_{bias}$ in §4.1, $\theta_{base}$ in §4.1, $\gamma_{emo}$ in §B1, $\lambda_{conf}$ nowhere, climax threshold 7.5 in §2.3
- Consolidate into §4.0 "Constants Table" for quick reference
- Effort: **S** (30 min doc edit)

### 8.3 [REFACTOR] Reconcile $\Omega_{conf}$ definition
- Canonical §B7 (per-character sum) vs Layer F §3.4 (Frobenius norm) — pick one (recommend Frobenius, matches code)
- Optionally add `omega_conf_directed(C)` as a separate per-character field for future aggressor/victim UI
- Effort: **S** (1h doc + code review)

### 8.4 [REFACTOR] Patch Layer F §4.2 DTO schemas to match Rust
- Update TS interfaces to match `poler.rs` Rust DTOs (which are the source of truth per meta-prompt §5.3)
- Effort: **S** (30 min doc edit)

### 8.5 [REFACTOR] Backfill missing audit fix CD4 (homonym POS filter)
- Apply `PosTagger` filter to `ACTION_VERBS` fallback path in `epsilon.rs:432-433`
- Effort: **M** (2-4h: requires loading PosTagger rules, integrating with epsilon.rs)

### 8.6 [REFACTOR] Backfill missing audit fix DD3 (lemmatized climax path)
- Add `use_lemmatizer: bool` parameter to `compute_epsilon_climax_inner`
- Effort: **M** (3-5h: token pipeline refactor)

---

## 9. Layer G Relevance

### 9.1 Layer G Visibility in the 3 POLER Specs
- **Canonical spec**: 0 mentions of Layer G — purely mathematical, no LLM context
- **v7.5 Audit**: 0 mentions of Layer G — purely code-level bug audit
- **Layer F spec**: 2 mentions — only "Layer G comes after Layer F" + "LLM resolves temporal paradoxes"

### 9.2 What Layer G Engineers Should Know Before Reading These Specs
1. **Canonical spec is the math source of truth** — read §4 (formulas), §8 (SymPy proofs), §9 (B1–B8 resolutions). Skip §1–3 (empirical benchmark on 2 manuscripts — historical context only).
2. **v7.5 Audit is historical** — all 4 critical discrepancies are fixed in code EXCEPT CD4 (partial) and DD3 (not fixed). Don't re-audit; verify the partial fixes if Layer G depends on action-verb classification.
3. **Layer F spec §3.4 math is WRONG** — DO NOT use Layer F's climax formula or $\theta_{rel}$ formula as a reference. Always defer to Canonical §4.1 + v7.5 Audit §1.
4. **Layer F spec §6 (component blueprints) is VALID** — `SvoHighlighter.tsx` and `PolerPanel.tsx` were implemented verbatim in `c143a7f`.
5. **Layer F spec §4.2 DTO schemas are STALE** — use Rust DTOs from `src-tauri/src/commands/poler.rs` as source of truth (per meta-prompt §5.3).

### 9.3 Layer G Inputs Available from POLER Stack (per Layer F spec + subagents 01–03)
| Input | Source | Layer G Use |
|---|---|---|
| $\varepsilon_{climax}$ per chapter | `cmd_compute_epsilon_climax` (Tauri) | Prioritize which chapters to LLM-analyze (is_climax=true) |
| SVO triplets per chapter | `cmd_extract_svo` (Tauri) | Build "action timeline" per character |
| `ParadoxReport` (DeadSpeaking) | `cmd_detect_paradoxes` (Tauri) | Trigger LLM hypothesis generation (Flashback/Dream/Resurrection/Impostor) |
| $\Omega_{conf}$ (Frobenius norm) | `EpsilonClimaxDto.omegaConf` | Conflict intensity scoring |
| $\rho(A_{POS})$ (spectral radius) | `EpsilonClimaxDto.spectralRadius` | Graph connectivity indicator |
| `SvoTriplet.polarity` | `SvoTripletDto.polarity` | Filter out negated actions from LLM prompt context |
| `SvoTriplet.confidence` | `SvoTripletDto.confidence` | Token-budget triage (high-confidence triplets first) |

### 9.4 Layer G Inputs NOT Available (Gaps)
1. **`SpatialTeleportation` paradoxes never emitted** (subagent 03 paradox.rs:90-93 placeholder)
2. **`Paradox.id` missing** (subagent 03 — must use composite key)
3. **`Paradox.evidence_text` missing** (subagent 03 — must re-fetch chapter text across IPC boundary)
4. **`Paradox.confidence` missing** (subagent 03 — no token-budget triage for paradoxes)
5. **`ConflictReport.paradoxes` always empty from `NarrativeGraph::analyze`** (subagent 03 — must call `ParadoxDetector::detect` separately)
6. **No `analyze_full()` helper** in `litgraph-core` (subagent 03 — orchestration must be duplicated)

→ **These gaps are documented in subagent 03 report §6, not in the 3 POLER specs.** Layer G implementation should consult subagent 03 report directly for the full gap list.

---

## 10. Recommended Next Actions

1. **[P0] Fix Layer F §3.4 climax formula** — replace `2.0 · A_SVO + 1.5 · Ω_conf + 1.2 · I_loc` with canonical `(κ · I_loc · d̄² + γ_emo · E + λ_conf · Ω_conf) / ln(e + |U|)`. Eliminates the most dangerous spec-vs-canonical divergence. — **S effort (15 min doc edit)**
2. **[P0] Fix Layer F §3.4 θ_rel formula** — replace `1.5/√(word_count)` with `3.5/κ`. — **S effort (5 min doc edit)**
3. **[P0] Patch Layer F §4.2 DTO schemas** — match Rust DTOs (`omegaConf` not `frobeniusNorm`, `isNoise` not `noiseFiltered`, `chapters` not `chapterBreakdowns`). — **S effort (30 min doc edit)**
4. **[P1] Add constants table to Canonical §4** — explicitly state γ_emo=1.0, λ_conf=12.5, δ_bias=15.0, θ_base=3.5, climax_threshold=7.5. — **S effort (30 min doc edit)**
5. **[P1] Reconcile Ω_conf definition** — update Canonical §B7 to acknowledge Frobenius norm as canonical (matches code); add directed per-character variant for future UI. — **S effort (1h doc + code review)**
6. **[P1] Fix CD4 homonym filter in fallback path** — apply `PosTagger` to `ACTION_VERBS` fallback in `epsilon.rs:432-433`. — **M effort (2-4h)**
7. **[P2] Fix DD3 lemmatized climax path** — add `use_lemmatizer: bool` to `compute_epsilon_climax_inner`. — **M effort (3-5h)**
8. **[P2] Reconcile I_loc formula** — pick linear (matches code) or log (matches Layer F spec), update both. — **S effort (15 min doc edit)**
9. **[P2] Add Layer G cross-reference section to Layer F spec** — point readers to `docs/layer-g-planning/00-META_PROMPT.md` and `POLER_UA_LP_MASTER_ROADMAP_V8.md §Layer G`. — **S effort (15 min doc edit)**
10. **[P3] Add v7.5 Audit closure status section** — mark CD1–CD3 + DD1–DD2 as ✅ Fixed, CD4 as 🟡 Partial, DD3 as ❌ Not Fixed, with current code line references. — **S effort (30 min doc edit)**

---

## 11. Dependencies / Blockers

- **Depends on (for verification)**:
  - `litgraph-core/src/parser/epsilon.rs` (current code state, v7.5-LEM Canonical, 1002 LOC)
  - `src-tauri/src/commands/poler.rs` (Rust DTO definitions, source of truth)
  - Subagent reports 01 (`litgraph-core/src/linguistic/*`), 02 (`litgraph-core/src/parser/*`), 03 (`litgraph-core/src/reasoning/*`)
  - `docs/layer-g-planning/00-META_PROMPT.md` (Layer G planning context)
  - `POLER_UA_LP_MASTER_ROADMAP_V8.md` (canonical $\lambda_{conf} = 12.5$ reference)
- **Blocks**:
  - **Layer G implementation** — until Layer F §3.4 math is corrected, any engineer reading the spec as source of truth will implement the wrong climax formula. **Critical blocker for Layer G if Layer G consumes climax scores** (which it does, per §9.3 above).
  - **Doc accuracy audit** — until Layer F §4.2 DTOs are patched, the spec misleads frontend engineers.
  - **Spec-canonical alignment** — until $\Omega_{conf}$ and $I_{loc}$ definitions are reconciled, there's no single source of truth for these formulas.
- **Cross-cutting constraint**: Per `docs/layer-g-planning/00-META_PROMPT.md:159` constraint #3 — *"Source of truth: Rust DTOs > spec docs (when they disagree)"*. This means code is canonical even when specs disagree. The 3 POLER specs should be patched to match code, not vice versa.

---

## 12. Summary

**Spec-vs-code alignment status**: Code (`epsilon.rs` v7.5-LEM) is mathematically correct and matches the canonical climax formula. The v7.5 Audit's 4 critical discrepancies are 3/4 fixed (CD1, CD2, CD3 ✅; CD4 partial). The 3 documentation drifts are 2/3 fixed (DD1, DD2 ✅; DD3 not fixed). The Layer F spec was used as the blueprint for `c143a7f` but contains 3 math errors in §3.4 (wrong climax formula, wrong $\theta_{rel}$, wrong $I_{loc}$) — code is correct; spec is wrong. The Layer F §4.2 DTO schemas are stale (pre-`c143a7f` audit discovery that Rust DTOs differ); Rust DTOs are the source of truth per meta-prompt §5.3.

**Layer G hints**: Only Layer F spec mentions Layer G (2 references in §1.2), declaring it the "LLM Reasoning Bridge" that comes AFTER Layer F and resolves temporal paradoxes. Canonical and v7.5 Audit specs have 0 Layer G references. Layer G planning context lives outside these 3 docs — in `docs/layer-g-planning/00-META_PROMPT.md` and `POLER_UA_LP_MASTER_ROADMAP_V8.md §Layer G`.

**Mathematical rigor**: Frobenius norm and spectral radius are correctly defined in code and match Layer F §3.4/§3.5 (which are correct on these two). $\theta_{rel}$ has a 3-way divergence (Canonical $3.5/\kappa$ vs Layer F $1.5/\sqrt{word\_count}$ vs code $3.5/\kappa$ — code matches canonical). $\Omega_{conf}$ has a spec-vs-spec divergence (Layer F Frobenius norm vs Canonical §B7 per-character sum — code matches Layer F). All other math (log10 rarity, ln denominators, asymptotic stability, partial derivatives) is correctly implemented.
