# Subagent 13: POLER Master Roadmap (V8) + Layer B/C Implementation Plan

> **Scope owner**: Master roadmap and Layer B/C implementation plan.
> **Documents inspected**:
> - `/home/z/my-project/litgraph-desktop/POLER_UA_LP_MASTER_ROADMAP_V8.md` (189 LOC, dated 2026-08-10, commit `a412ad3`)
> - `/home/z/my-project/litgraph-desktop/POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md` (402 LOC, doc version 1.0.0)
> - Cross-referenced for context: `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` (800 LOC, v8.0-CANONICAL), `litgraph-desktop/worklog.md` (2416 LOC), `litgraph-desktop/docs/layer-g-planning/00-META_PROMPT.md` (this 17-subagent orchestration plan), and reports from subagents 01–07.

---

## 1. Scope
- **Files inspected**: 4 primary + 3 cross-reference
- **Total LOC** (primary docs): 1,391 (189 roadmap + 402 Layer B/C plan + 800 Layer F spec)
- **Key entry points**:
  - `POLER_UA_LP_MASTER_ROADMAP_V8.md:6` — Status line: "Layers A, B, C, D **100% Completed, Verified & Synchronized**"
  - `POLER_UA_LP_MASTER_ROADMAP_V8.md:88–104` — Mermaid Gantt timeline for Layers E–H
  - `POLER_UA_LP_MASTER_ROADMAP_V8.md:143–153` — Layer G section (12 LOC, the entire spec)
  - `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md:54–225` — Layer B detailed Rust spec
  - `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md:229–271` — Layer C SVO extractor spec
  - `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md:364–394` — Gantt + task/deliverable matrix

---

## 2. Atomic Inventory

### 2.1 Documents / Sections

| Document | LOC | Purpose | Key Sections | External Cross-References |
|---|---|---|---|---|
| `POLER_UA_LP_MASTER_ROADMAP_V8.md` | 189 | Master roadmap v8.0. Declares Layers A–D complete; defines Layers E–H future plan with Gantt timeline + verification matrix. | §1 Overview+ASCII pipeline diagram, §2 Layer A–D audit summary, §3 Future Roadmap E–H (Gantt), §3 Layer E detail, §3 Layer F detail, §3 Layer G detail, §3 Layer H detail, §4 Verification/QA matrix, §5 Conclusion | Commit `a412ad3` (anchor); POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md (v8.0-CANONICAL) |
| `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md` | 402 | Detailed Layer B (POS-Tagger) + Layer C (SVO Engine) action plan, doc v1.0.0. Pre-implementation spec; predates the V8 roadmap. | Exec Summary+sitemap, Layer B spec (linguistic background, LT resource analysis, Rust data structures, deterministic algorithm), Layer C spec (UD-Ukrainian integration, SVO triplet struct), Layer D integration, SymPy analytical derivations, spectral matrix analysis, case frame system, micro-architecture, step-by-step Gantt, task matrix | `litgraph-core/src/linguistic/pos_tagger.rs`, `svo_parser.rs`, `lemmatizer.rs`, `xtask/src/build_pos_tables.rs`, `xtask/src/build_svo_templates.rs` |
| `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` | 800 | Layer F React Visualizer + Tauri IPC spec, v8.0-CANONICAL. Approves Layer F as prerequisite to Layer G ("Validate by Using" imperative). | §1 Mission, §2 End-to-end architecture, §3 Layer-by-layer breakdown, §4 Tauri IPC DTOs, §5 React components, §6 PolerPanel example, §7 Performance, §8 Implementation plan | Cites Layer G as downstream consumer |

### 2.2 Roadmap Layer Definitions (V8)

| Layer | Roadmap Section | Status in V8 | Actual Status (per worklog + subagents 01–07) | Drift |
|---|---|---|---|---|
| **A: Lemmatizer** | §2 (line 51–54) | Done | Done — `litgraph-core/src/linguistic/lemmatizer.rs` (334 LOC), 227k lemmata, 2.23M wordforms, 16.94 MB `lemma_index.json.gz`. Subagent 01 verified 36 tests pass. | None |
| **B: POS-Tagger** | §2 (line 56–61) | Done | Done — `pos_tagger.rs` (870 LOC), 450 LanguageTool rules, 37,728 case frames, 32 tests pass. Build artifact `pos_rules.json.gz` = 243 KB (roadmap claimed ~1.2 MB — 5× smaller because plan parsed Java regex syntax that 28 LT rules couldn't compile in Rust). | Minor: artifact size, regex coverage |
| **C: SVO Parser** | §2 (line 63–69) | Done | Done — `svo_parser.rs`, 3,234 CoNLL-U templates (40.84 KB artifact). `SvoTriplet.location: Option<String>` field exists. Subagent 03 confirmed: location field is **loaded but never consumed** by `NarrativeGraph` or `ParadoxDetector` — required for SpatialTeleportation. | Field exists, unused — blocks Layer E spatial paradox |
| **D: POLER ε v7.5-LEM** | §2 (line 71–82) | Done | Done — `epsilon.rs` (1001 LOC per subagent 02). 6 spec-divergent spots identified by subagent 02 (I_kw uses ln vs log10, dead `_global_counts`/`_total_words` params, climax path not lemmatized, I_loc always 1.0 because CANON_ANCHORS lowercase vs Capitalized-required detector, A_SVO replacement logic confusing). | Substantive — 6 spec drift spots |
| **E: Narrative Graph + Ω_conf + Paradox** | §3 (line 108–123) | Future (Gantt: e1+e2+e3, 6 days after 2026-08-11) | **Implemented** (commit `37be4b6` + `6091ae9`) — `litgraph-core/src/reasoning/{narrative_graph,paradox,stub,mod}.rs` (1253 LOC, ~883 production + 370 tests). Subagent 03 verified. | Roadmap is STALE — published before Layer E was built |
| **F: Tauri Desktop + React Visualizer** | §3 (line 126–140) | Future (Gantt: f1+f2, 5 days) | **Implemented** (commits `d1b3390` Layer F.1 + `c143a7f` Layer F.2). 3 IPC commands + PolerPanel/SvoHighlighter (1211 insertions). | Roadmap is STALE — Layer F.2 already shipped |
| **G: LLM Reasoning Bridge & Hypothesis Verifier** | §3 (line 143–153) | Future (Gantt: g1+g2, 4 days after f2) | **NOT implemented at the specified paths**. The roadmap names `litgraph-core/src/reasoning/llm_bridge.rs` + `litgraph-core/src/reasoning/hypotheses.rs` — these files **do not exist**. Files of the same name exist at `src-tauri/src/reasoning/{llm_bridge,hypotheses}.rs` (~1100 + ~910 LOC), but they belong to the **Wave 5 semantic-IR reasoning system**, NOT the POLER Layer E pipeline. Subagent 03 flagged this as "CRITICAL ARCHITECTURAL NOTE" — two parallel `TemporalParadox` types must be reconciled before Layer G can consume both. | Severe — wrong file location + architectural duplication |
| **H: Release & Native Packaging** | §3 (line 156–166) | Future (Gantt: h1, 2 days after g2) | Not started. | None (correctly future) |

### 2.3 Public Types / Spec Contracts Declared in the Roadmap

- `cmd_compute_epsilon(text, keyword, kappa)` → `EpsilonResult` JSON (Layer F deliverable, roadmap line 132) — **REALITY**: actual command is `cmd_compute_epsilon_climax(chapter_text, keyword?, kappa=1.0)` returning `EpsilonClimaxDto` (renamed).
- `cmd_extract_svo(text)` → `Vec<SvoTriplet>` JSON (Layer F deliverable, roadmap line 133) — **REALITY**: matches (signature unchanged).
- `cmd_detect_contradictions(project)` → `ContradictionReport` (Layer F deliverable, roadmap line 134) — **REALITY**: actual command is `cmd_detect_paradoxes(text)` returning `ParadoxReportDto` (renamed + signature changed: takes raw text, not Project).
- Hypothesis kinds (Layer G, roadmap line 150): *Flashback Narrative*, *Dream Sequence*, *Unrecorded Resurrection*, *Disguised Identity* — **4 kinds enumerated**, but no Rust enum, no prompt template, no Tauri command signature.

### 2.4 Verification / QA Matrix (roadmap §4, lines 172–181)

| Stage | Roadmap Expected | Reality |
|---|---|---|
| Layer A lemma index | 2,234,167 wordforms | ✅ Matches (worklog `symbolic-ua-lp-02`) |
| Layer B POS rules | 450 rules | ✅ Matches (subagent 01: 450 compiled, 28 source rules skipped due to Java regex incompatibility) |
| Layer C SVO templates | 3,234 CoNLL-U rules | ✅ Matches |
| Layer D negated verb Δε | 0.4714 ± 0.02 | ✅ Matches (commit a412ad3 strengthened tests) |
| Full litgraph-core suite | 81/81 PASSED | ❌ STALE — by commit c143a7f it's 86/86 (5 new POS-tagger tests added in commit 70fc9aa); roadmap snapshot is from a412ad3 |
| Tauri suite | 280/280 PASSED | ⚠️ Cannot verify in sandbox (no GTK3 dev libs); user-reported figure, unconfirmed |

---

## 3. Current State

### What works (per roadmap V8 claims, verified against subagent reports)
- Layer A lemmatizer: 227,051 lemmata, 56–73% coverage, 8 unit tests.
- Layer B POS-tagger: 3-pass disambiguation (LT rules → case government → fallbacks), 32 unit tests.
- Layer C SVO extractor: 3,234 UD-IU templates, `SvoTriplet` struct with 7 fields including `location` (loaded but unused).
- Layer D ε v7.5-LEM: canonical + lemmatized + climax formulas; commit a412ad3 strengthened spec-critical tests.

### What's stubbed
- `ParadoxKind::SpatialTeleportation` — declared in enum (`paradox.rs:37`) but **never emitted** by `detect()`. Docstring explicitly says: *"Not yet implemented. Requires Layer F location normalization to detect non-adjacent location pairs."* (Subagent 03, paradox.rs:90–93.) The Layer E entry in the roadmap V8 (§3, line 119) lists "Spatial Teleportation Paradox" as a deliverable — **incomplete**.
- `ConflictReport.paradoxes` when produced by `NarrativeGraph::analyze` — always `Vec::new()` (subagent 03, narrative_graph.rs:206). Roadmap implicitly assumes a single-call analyzer produces both Ω_conf and paradoxes; reality requires manual orchestration in `cmd_detect_paradoxes` (src-tauri/src/commands/poler.rs:455–464).
- Layer G at the specified paths (`litgraph-core/src/reasoning/{llm_bridge,hypotheses}.rs`) — files do not exist.

### What's missing
- Layer G concrete contract: no Rust `enum HypothesisKind`, no `struct Hypothesis`, no `HypothesisReport` DTO, no `cmd_generate_llm_hypotheses` Tauri command. The 4 enumerated kinds (Flashback/Dream/Resurrection/Disguise) exist only as prose.
- Layer G prompt templates: zero template scaffolding in `litgraph-core/src/ai/prompts.rs` (subagent 04 confirmed). The `build_assistant_prompt`/`build_continue_chapter_prompt`/`build_analyze_plot_prompt` builders exist but are RU/UK-language-agnostic narrative assistants, not paradox-resolution templates.
- Layer G validator: no `validate_hypothesis(h: &Hypothesis, manuscript: &ManuscriptAnalysis) -> ValidationResult` function. The Wave 5 `LlmBridge::validate_response` exists at `src-tauri/src/reasoning/llm_bridge.rs:136` but is for Action::Write* constraints, not Layer E paradox resolution.
- Layer H (Release & Packaging): no `scripts/build_release.sh` exists; CI has only `.github/workflows/release.yml` for tag-triggered builds.
- No Layer G→Layer F frontend wiring: PolerPanel's Paradox Feed tab (POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md:746–773) renders paradoxes but has **no "Generate Hypothesis" button** to invoke Layer G.

---

## 4. Gaps / Bugs / TODOs

### [GAP] Layer G target file paths in roadmap are wrong
- Roadmap line 145: `litgraph-core/src/reasoning/llm_bridge.rs`, `litgraph-core/src/reasoning/hypotheses.rs`
- Reality: those files exist at `src-tauri/src/reasoning/{llm_bridge,hypotheses}.rs` but serve the Wave 5 semantic-IR reasoning system (different `TemporalParadox` type, different `WorldState` model, different lifecycle).
- The `litgraph-core/src/reasoning/` directory currently contains only `mod.rs`, `narrative_graph.rs`, `paradox.rs`, `stub.rs` (Layer E). No llm_bridge.rs/hypotheses.rs have been added there.
- Impact: an engineer reading the roadmap and creating those files would collide with the existing src-tauri/reasoning ones; an engineer looking for "Layer G llm_bridge.rs" via ripgrep would find the WRONG file.

### [GAP] Roadmap is stale relative to commit history
- Roadmap published at commit `a412ad3` (commit `01bc548` adds the doc itself).
- Since then, 5+ subsequent commits implement Layer E (`37be4b6`), Layer E mod re-exports (`6091ae9`), Layer F.1 IPC (`d1b3390`), Layer F.2 React (`c143a7f`).
- The roadmap's "Status: Layers A, B, C, D 100% Completed" line and "Future Roadmap: Layers E – H Action Plan" framing are out of date — Layers E and F are implemented, but the roadmap treats them as future work.

### [GAP] Layer G spec is too thin to be actionable
- Total Layer G section: 12 LOC (lines 143–153).
- Deliverable 1: "Contradiction Resolution Prompts" — 3-line description, no prompt template, no schema for the LLM response.
- Deliverable 2: "Automated Verifier" — 1-line description, no validation algorithm, no acceptance criteria.
- Compare to Layer B/C plan (402 LOC) and Layer F spec (800 LOC) which provide concrete Rust struct signatures, Tauri command DTOs, and component JSX examples. Layer G has none.

### [GAP] Roadmap claims of Layer E deliverables are partial
- Roadmap line 117–120 lists: (1) Character Adjacency Matrix A_POS + ρ(A_POS) with Δρ=4.16%; (2) Temporal Paradox Detector with both DeadSpeaking and Spatial Teleportation; (3) Climax Metric ε_climax with Ω_conf integration.
- Reality (subagent 03): (1) ✅ implemented; (2) ⚠️ only DeadSpeaking implemented, SpatialTeleportation is dead code; (3) ✅ implemented but `analyze_chapter` path doesn't return paradoxes (manual merge required).

### [DRIFT] Layer B/C plan vs implementation
- Plan promised artifact `pos_rules.json.gz` ~1.2 MB; reality 243 KB (~5× smaller).
  - Cause: 28 LanguageTool Java-regex patterns failed to compile under Rust `regex` crate (Java regex syntax incompatibility). The build script silently skips them. The plan did not anticipate this.
- Plan promised 25+ unit tests for Layer B; reality 32 tests (exceeds plan — positive drift).
- Plan promised `PosTagger::tag_sentence()` API; reality matches.
- Plan promised `SvoParser::extract_triplets()`; reality is `SvoParser::new().parse_text()` (slight rename).
- Plan promised SymPy sensitivity ∂ε/∂μ_pos = 0.7303 with N_homonyms=2; reality (worklog `symbolic-ua-lp-04-rebase-sync`) verified this exact figure.
- Plan promised spectral radius reduction Δρ = 4.16% (ρ_raw=21.4523 → ρ_POS=20.5606); this is a **paper/analytical** projection, not yet validated against a real `A_POS` matrix built from manuscript data. Subagent 03 confirmed `NarrativeGraph::adjacency_matrix()` exists but no benchmark script in repo computes the empirical Δρ.

### [DRIFT] Roadmap Gantt timeline vs actual delivery
- Roadmap Gantt: Layer E = 6 days (2026-08-11 → 2026-08-16), Layer F = 5 days, Layer G = 4 days, Layer H = 2 days. Total = 17 days.
- Reality: Layer E + Layer F.1 + Layer F.2 delivered across 4 commits between 2026-08-08 and 2026-08-10 (per worklog dates in commits `37be4b6`, `6091ae9`, `d1b3390`, `c143a7f`). The 11-day Layer E + F plan compressed to ~2–3 calendar days.
- The Gantt timeline is therefore wildly over-estimated; Layer G's planned 4 days could plausibly be 1–2 days given the velocity.

### [DRIFT] Roadmap V8 verification matrix is stale
- Roadmap line 180: "Full Suite: 81/81 PASSED".
- Reality at commit c143a7f: 86/86 tests pass in litgraph-core (5 new POS-tagger integration tests added in 70fc9aa).
- The roadmap's QA matrix was correct at a412ad3 but should have been bumped to 86/86 before Layer F.2 ship.

---

## 5. Refactoring Opportunities

### [REFACTOR] Promote Layer G from prose to contract spec
- **Action**: Replace the 12-line Layer G section with a dedicated `POLER_LAYER_G_IMPLEMENTATION_PLAN.md` (mirroring the 402-line Layer B/C plan), specifying:
  - `enum HypothesisKind { Flashback, Dream, Resurrection, Disguise }` Rust signature
  - `struct Hypothesis { paradox_ref: ParadoxRef, kind: HypothesisKind, confidence: f64, evidence_text: String, prompt_template_id: &str }`
  - `struct HypothesisReport { hypotheses: Vec<Hypothesis>, unresolvable_paradoxes: Vec<ParadoxRef>, stats: HypothesisStats }`
  - `#[tauri::command] async fn cmd_generate_llm_hypotheses(text: String, provider: AiProvider, options: Option<HypothesisOptions>) -> Result<HypothesisReportDto, String>`
  - 4 prompt templates (one per HypothesisKind) with variables `{{character}}`, `{{death_chapter_idx}}`, `{{speaking_chapter_idx}}`, `{{death_chapter_text}}`, `{{speaking_chapter_text}}`, `{{witnesses}}`
  - Validation algorithm: hypothesis is accepted iff (a) prompt template rendered non-empty, (b) LLM returned JSON parseable to `Hypothesis` schema, (c) `kind` is one of the 4 enumerated, (d) `confidence >= 0.5`, (e) `evidence_text` non-empty and references both chapter indices.
- **Expected benefit**: makes Layer G actionable in <1 day of implementation; unblocks subagents 05/11 (Tauri commands) and 14 (Layer G detailed work prompt).

### [REFACTOR] Resolve the two parallel `TemporalParadox` types
- Per subagent 03 (lines 451–456): `litgraph-core/src/reasoning/paradox.rs::Paradox` (Layer E, kinds: DeadSpeaking | SpatialTeleportation) vs `src-tauri/src/reasoning/contradictions.rs::TemporalParadox` (Wave 5, different fields).
- The roadmap implicitly assumes ONE paradox type feeds Layer G; reality has TWO.
- **Action**: Add a §"Paradox Type Unification" subsection to Layer G plan deciding: (a) consume Layer E only (Wave 5 deprecated), (b) consume Wave 5 only (Layer E bypassed), or (c) unify via adapter trait `trait ParadoxFeed { fn paradoxes(&self) -> Vec<UnifiedParadox>; }`.
- **Expected benefit**: prevents Layer G from accidentally importing the wrong type and getting irrelevant fields.

### [REFACTOR] Update the roadmap to V9 reflecting post-a412ad3 reality
- **Action**: Bump roadmap version V8 → V9 with:
  - Status line: "Layers A, B, C, D, E, F.1, F.2 100% Completed; Layers G, H pending."
  - Move Layers E + F from §3 "Future Roadmap" to §2 "Completed Milestones".
  - Add Layer E/F audit summary (subagent 03 + 06 + 11 findings).
  - Add a §"Known Drift" subsection documenting the SpatialTeleportation stub and the `analyze_chapter` empty-paradoxes gap.
  - Bump test count 81 → 86 in the QA matrix.
  - Update Layer G target file paths to either (a) new files in `litgraph-core/src/reasoning/` OR (b) extension of the existing `src-tauri/src/reasoning/` modules — explicit decision required.
- **Expected benefit**: roadmap becomes an accurate source of truth again.

### [REFACTOR] Add `Paradox::id` and `Paradox::evidence_text` fields
- Per subagent 03 (action items 2, lines 417–419): the current `Paradox` struct lacks stable identifier and evidence text. Layer G prompt construction needs both.
- The roadmap's Layer G "Contradiction Resolution Prompts" deliverable cannot be implemented without these fields.
- **Action**: Add roadmap line item to Layer E audit acknowledging the gap and queueing the field additions.

### [REFACTOR] Implement `analyze_full()` helper
- Per subagent 03 (line 417): the canonical way to get a complete `ConflictReport` with populated `paradoxes` should be a single function, not manual orchestration in `cmd_detect_paradoxes`.
- The roadmap's Layer F deliverable `cmd_detect_contradictions` (line 134) implicitly assumes this; reality has it scattered across Tauri command code.

---

## 6. Layer G Relevance

### 6.1 What the V8 roadmap specifies for Layer G
1. **Target files**: `litgraph-core/src/reasoning/llm_bridge.rs`, `litgraph-core/src/reasoning/hypotheses.rs` (roadmap line 145).
2. **Deliverable 1 — Contradiction Resolution Prompts** (roadmap line 148–150):
   - Automated generation of structured LLM prompts when a temporal paradox is detected.
   - Proposed hypotheses: *Flashback Narrative*, *Dream Sequence*, *Unrecorded Resurrection*, *Disguised Identity*.
3. **Deliverable 2 — Automated Verifier** (roadmap line 151–152):
   - Validate proposed LLM hypotheses against temporal constraints and fact logs before updating narrative state.
4. **Gantt timeline** (roadmap line 99–101):
   - g1 (after f2, 2d): "Flashback & Dream Hypothesis Generator"
   - g2 (after g1, 2d): "Hypothesis Verification Pipeline"

### 6.2 What the roadmap does NOT specify (gaps blocking implementation)
- ❌ No Rust enum for `HypothesisKind` (only prose names).
- ❌ No `Hypothesis` struct definition (fields, serialization).
- ❌ No `HypothesisReport` DTO shape.
- ❌ No Tauri command signature for `cmd_generate_llm_hypotheses`.
- ❌ No prompt template bodies (no Jinja/Handlebars/inline-string format).
- ❌ No validator algorithm (acceptance criteria: confidence threshold? evidence citation? kind-specific rules?).
- ❌ No LLM provider plumbing (which `AiProvider` variants are supported? streaming? token budget?).
- ❌ No frontend wiring (how does PolerPanel's Paradox Feed trigger hypothesis generation? where do hypotheses render?).
- ❌ No file-location decision (litgraph-core vs src-tauri/src/reasoning).
- ❌ No reconciliation plan for the two parallel `TemporalParadox` types.
- ❌ No dependency declaration on Layer E gaps (`Paradox::id`, `Paradox::evidence_text`, `analyze_full()`, SpatialTeleportation implementation).

### 6.3 Cross-checks against subagents 01–07
| Subagent | Finding | Roadmap impact |
|---|---|---|
| 01 (core-linguistic) | `SvoTriplet.location` field is loaded but unused by `NarrativeGraph` and `ParadoxDetector`. DisambigAction::Immunize is a no-op. Dead code in pos_tagger.rs:711–714. | Roadmap Layer E "Spatial Teleportation Paradox" deliverable is blocked on consuming this field. |
| 02 (core-parser) | `epsilon.rs` has 6 spec-divergent spots. `themes.rs` (209 LOC) is dead code. `locations::count_in_text` lacks word-boundary check (bug). | Roadmap Layer D audit (§2 line 71–82) claims "Rigorous Test Assertions" — these gaps contradict that claim. SpatialTeleportation additionally blocked on per-chapter location normalization. |
| 03 (core-reasoning) | `SpatialTeleportation` never emitted by `detect()`. `NarrativeGraph::analyze` always returns empty `paradoxes`. `Paradox` lacks `id` and `evidence_text` fields. Two parallel `TemporalParadox` types (Layer E + Wave 5). | Roadmap Layer E deliverables #2 (paradox detector) and Layer G contract (hypothesis referencing) are both incomplete. |
| 04 (core-models-ai) | `AiProvider` enum + `chat()` dispatcher are sufficient for non-streaming single-shot RU/UK tasks. No streaming, no JSON-mode, no temperature/max_tokens/top_p. `Openaicompat` vs `OpenAiCompat` identifier drift between crates. | Roadmap Layer G's "Automated Verifier" implies structured LLM output — current `AiProvider` cannot enforce JSON. Streaming would benefit UX but is not blocking. |
| 05 (tauri-commands) | AI provider plumbing bug: dialogs call `callApi` directly without `provider` field; IPC fails with "missing field `provider`". Layer G integration point identified: new `commands/llm_bridge.rs` module + `cmd_generate_llm_hypotheses` command. `poler.rs` purity invariant says "no LLM calls". | Roadmap Layer G target path `litgraph-core/src/reasoning/llm_bridge.rs` would VIOLATE the poler.rs purity invariant if it tried to call LLMs from there. Reality: Layer G must live in `src-tauri/src/commands/llm_bridge.rs` to access `crate::ai::chat()`. |
| 06 (tauri-poler-bridge) | 14 byte-identical duplicates (= 40,794 LOC) between litgraph-core and src-tauri. 6 truly diverged files. | Roadmap claim (§2 line 82) "Workspace Architecture: src-tauri re-exports litgraph_core::linguistic and litgraph_core::parser::epsilon directly, eliminating 2,066 lines of duplicate code" — true for the linguistic+epsilon modules, but the broader 40k LOC duplication (parser/, models/, ai/, dict/, languagetool_weights.rs) is NOT addressed by the roadmap. |
| 07 (tauri-python-xtask) | Python invocation safe (no shell=True) but not hardened (temp file 0o644, no size cap, system python3 fallback). | Roadmap Layer H "Zero-Allocation Performance Optimization" target >60,000 frags/sec — Python cold-start 2-3s per call (subagent 07) makes this unachievable for any flow that touches Python. Layer G must avoid Python; should stay pure Rust + LLM HTTP. |

### 6.4 Roadmap claim vs reality table (consolidated)
| Roadmap claim (V8) | Reality (subagents 01–07 + worklog) | Verdict |
|---|---|---|
| Layers A–D "100% Completed, Verified & Synchronized" (line 6) | True for A, B, C; Layer D has 6 spec-divergent spots (subagent 02) | Mostly true |
| 81/81 litgraph-core tests pass (line 180) | 86/86 by commit c143a7f | Stale (positive direction) |
| 280/280 src-tauri tests pass (line 181) | Unverifiable in sandbox (no GTK3 dev libs) | Unconfirmed |
| Layer E deliverable: "Temporal Paradox Detector (Dead Speaking, Spatial Teleportation)" (line 119) | Only DeadSpeaking implemented; SpatialTeleportation is dead code | Partial |
| Layer E deliverable: ε_climax with Ω_conf = ‖A_POS‖_F integration (line 121–123) | Implemented; but `analyze_chapter` returns empty `paradoxes` (manual orchestration required) | Implemented with caveat |
| Layer F deliverable: `cmd_compute_epsilon(text, keyword, kappa)` (line 132) | Actual: `cmd_compute_epsilon_climax(chapter_text, keyword?, kappa=1.0)` | Renamed + signature drift |
| Layer F deliverable: `cmd_detect_contradictions(project)` → `ContradictionReport` (line 134) | Actual: `cmd_detect_paradoxes(text)` → `ParadoxReportDto` | Renamed + signature drift |
| Layer F deliverable: "React-Flow Graph Renderer & Reader Overlay (`ReaderDialog.tsx`, `GraphCanvas.tsx`)" (line 128, 135–139) | Actual: `PolerPanel.tsx` + `SvoHighlighter.tsx` modal in Toolbar (different file names, different UX pattern — modal vs dedicated canvas) | Renamed + UX drift |
| Layer G target files: `litgraph-core/src/reasoning/{llm_bridge,hypotheses}.rs` (line 145) | Files do not exist there; same-named files exist at `src-tauri/src/reasoning/` for Wave 5 system | Wrong path |
| Layer G deliverable: 4 hypothesis kinds (Flashback/Dream/Resurrection/Disguise) (line 150) | No Rust enum, no prompt templates, no validator — prose only | Not started |
| Layer G deliverable: "Automated Verifier" (line 151–152) | No validator function, no acceptance criteria, no algorithm | Not started |
| Layer H deliverable: >60,000 frags/sec, AppImage, Windows Portable (line 162–166) | No `scripts/build_release.sh`; only `.github/workflows/release.yml` for tag builds; no benchmark script targeting 60k frags/sec | Not started |
| Layer B/C plan: pos_rules.json.gz ~1.2 MB (Layer B/C plan line 388) | Actual: 243 KB | 5× smaller (28 LT rules skipped) |
| Layer B/C plan: 25+ POS-tagger unit tests (line 390) | Actual: 32 tests | Exceeds plan |
| Layer B/C plan: SymPy ∂ε/∂μ_pos = 0.7303 (line 294) | Verified exactly (worklog `symbolic-ua-lp-04-rebase-sync`) | Matches |
| Layer B/C plan: Δρ spectral reduction 4.16% (line 319) | Paper projection; not validated against real `A_POS` matrix from manuscript | Untested empirically |

---

## 7. Recommended Next Actions

1. **Write `POLER_LAYER_G_IMPLEMENTATION_PLAN.md` (mirroring the 402-line Layer B/C plan structure)** — S (4–6h). Specify Rust enum `HypothesisKind`, struct `Hypothesis`, DTO `HypothesisReportDto`, Tauri command `cmd_generate_llm_hypotheses`, 4 prompt templates, validator algorithm, frontend wiring plan. **Critical blocker for Layer G implementation.**

2. **Decide Layer G file location: new files in `litgraph-core/src/reasoning/` OR extension of `src-tauri/src/reasoning/`** — S (1h discussion + decision). Document the decision in the new Layer G plan. Recommended: new `src-tauri/src/commands/llm_bridge.rs` (Tauri command) + new `litgraph-core/src/reasoning/hypotheses.rs` (pure-Rust hypothesis data structures + validator, no LLM calls) — separates pure logic from IPC, preserves `poler.rs` purity invariant.

3. **Add `Paradox::id: String` (UUIDv4) and `Paradox::evidence_text: String` fields to `litgraph-core/src/reasoning/paradox.rs`** — S (1h). Unblocks Layer G hypothesis referencing and prompt construction. Identified by subagent 03.

4. **Add `analyze_full()` helper in `litgraph-core/src/reasoning/mod.rs`** that runs `NarrativeGraph::analyze` + `ParadoxDetector::detect` and merges — S (1–2h). Removes duplicate orchestration from `cmd_detect_paradoxes`. Identified by subagent 03.

5. **Update POLER_UA_LP_MASTER_ROADMAP to V9** reflecting post-a412ad3 reality (Layers E + F complete, Layer G spec detailed, Layer H pending) — M (3–4h). Move Layers E + F from §3 to §2; bump test count 81→86; document known drift (SpatialTeleportation stub, `analyze_chapter` empty paradoxes, two parallel TemporalParadox types).

6. **Reconcile or document the two parallel `TemporalParadox` types** — M (4–6h). Either (a) deprecate Wave 5 `src-tauri/src/reasoning/contradictions.rs::TemporalParadox`, (b) bypass Layer E `Paradox`, or (c) introduce adapter trait `ParadoxFeed`. Identified by subagent 03 as "CRITICAL ARCHITECTURAL NOTE".

7. **Implement SpatialTeleportation paradox detection** — M (4–8h, per subagent 03 action item 3). Requires Layer F location normalization (canonical names from `SvoTriplet.location`), per-chapter location timeline, transit-verb lexicon. Unblocks Layer G from consuming only DeadSpeaking signals.

8. **Add "Generate Hypothesis" button to PolerPanel Paradox Feed tab** — S (1–2h, frontend only after Layer G Tauri command exists). Wire to `cmd_generate_llm_hypotheses`. Render returned hypotheses as expandable cards under each paradox entry.

9. **Add Layer G prompt templates to `litgraph-core/src/ai/prompts.rs`** — S (2h). 4 templates, one per `HypothesisKind`, with variable placeholders for character, death chapter, speaking chapter, witnesses. Reuse existing `build_messages(system, user, history)` helper.

10. **Validate Δρ=4.16% empirically against real manuscript `A_POS` matrices** — S (2–3h). Write `scripts/benchmark_spectral_radius.py` that builds `NarrativeGraph` from `tests/corpus/*.md`, computes raw vs POS-filtered ρ, and asserts the reduction matches the paper projection. Closes the Layer B/C plan's only empirically-unverified claim.

11. **Fix Layer D epsilon.rs 6 spec-divergent spots** identified by subagent 02 — M (4–6h). I_kw log10 unification, dead `_global_counts`/`_total_words` removal, climax-path lemmatization, I_loc capitalization fix, A_SVO replacement clarification. Pre-requisite for Layer G's reliable climax-threshold gating.

---

## 8. Dependencies / Blockers

### Depends on (upstream)
- Subagent 03's findings on `Paradox` struct gaps (`id`, `evidence_text`) — must be resolved before Layer G prompt construction.
- Subagent 04's `AiProvider` contract — sufficient for non-streaming Layer G but should be extended (R8: `prompts::build_layer_g_prompt` builder, R3: `AiClient` trait + `ChatOptions`).
- Subagent 05's `cmd_generate_llm_hypotheses` Tauri command design — must be registered in `lib.rs::generate_handler!` and exposed via `src/lib/tauri-commands.ts`.
- Subagent 11's PolerPanel UX — must add "Generate Hypothesis" affordance to Paradox Feed tab.
- Subagent 14 (POLER specs) — Layer G plan must align with `POLER_EPSILON_CANONICAL_SPECIFICATION.md` (the canonical formula doc) and `POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md`.

### Blocks (downstream)
- Layer H (Release & Packaging) — cannot ship a stable release without Layer G implemented (paradox feed in PolerPanel is currently read-only; users cannot resolve paradoxes via LLM).
- The 17-subagent synthesis (`99-DETAILED_WORK_PROMPT.md`) — this report feeds Part C (Layer G Implementation Plan) and Part E (Roadmap Alignment) of the master work prompt.
- The user's broader goal of "detailed, reviewable work prompt" — without a concrete Layer G contract, the work prompt cannot enumerate file-by-file tasks.

### External blockers
- The roadmap V8 itself — its Layer G section is too thin to be implementable without the recommended V9 update.
- The two parallel `TemporalParadox` types — architectural decision required before Layer G can consume paradoxes safely.

---

## 9. Summary

The POLER UA-LP Master Roadmap V8 is a **partially-stale, partially-thin** planning document. It accurately describes Layers A–D (verified complete by subagents 01–04) but treats Layers E and F as future work even though they are already implemented (commits `37be4b6`, `d1b3390`, `c143a7f`). The Layer G section is the weakest: 12 lines of prose, no Rust types, no prompt templates, no Tauri command signature, wrong target file paths (names `litgraph-core/src/reasoning/llm_bridge.rs` which doesn't exist; the same-named files at `src-tauri/src/reasoning/` belong to the unrelated Wave 5 system per subagent 03). The Layer B/C plan (402 LOC) is a much stronger template — its spec was largely honored in implementation, with documented drift (artifact 5× smaller due to Java regex incompatibility; 32 tests vs promised 25+; SymPy projection Δρ=4.16% analytically correct but empirically unvalidated).

**Layer G specification completeness: ~15%**. The 4 hypothesis kinds are enumerated as prose (Flashback/Dream/Resurrection/Disguise); the verifier is one sentence. To reach implementable (~85%), the roadmap needs: (1) Rust `HypothesisKind` enum + `Hypothesis`/`HypothesisReport` structs, (2) `cmd_generate_llm_hypotheses` Tauri command signature, (3) 4 prompt template bodies with variable placeholders, (4) validator algorithm with acceptance criteria, (5) file-location decision (litgraph-core vs src-tauri/src/reasoning), (6) two-`TemporalParadox`-types reconciliation plan, (7) Layer E gap closures (`Paradox::id`, `Paradox::evidence_text`, `analyze_full()`, SpatialTeleportation implementation).
