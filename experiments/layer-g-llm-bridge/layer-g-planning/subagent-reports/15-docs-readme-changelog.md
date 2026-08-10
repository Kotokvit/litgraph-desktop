# Subagent 15: docs-readme-changelog

## 1. Scope

- Files inspected: 12 docs + 1 capabilities JSON + spot-checks on `lib.rs` / `AIDialog.tsx` / `src/components/ui/` / `src/components/litgraph/`
- Total LOC: 4 023 doc lines (README 196, CHANGELOG 116, SECURITY 61, LICENSE 21, architecture 620, PROMPT_PLAN 715, reasoning/SPEC 566, poler_math/POLER_SPEC 1 007, llm-sandbox 326, language-rules/README 153, education/README 68, layer-g-planning/00-META_PROMPT 174)
- Key entry points: `README.md:1`, `CHANGELOG.md:1`, `SECURITY.md:1`, `docs/architecture.md:1`, `docs/llm-sandbox.md:1`, `docs/layer-g-planning/00-META_PROMPT.md:1`

## 2. Atomic Inventory

### 2.1 Modules / Files

| File | LOC | Purpose | Public API | Status |
|------|-----|---------|------------|--------|
| `README.md` | 196 | Project overview, install, dev guide | n/a | 🔴 Stale (v0.1.0-era) |
| `CHANGELOG.md` | 116 | Release history | n/a | 🔴 Stuck at 0.2.1 (code is 0.2.2) |
| `SECURITY.md` | 61 | GitHub-token handling for author | n/a | 🟡 Mis-scoped |
| `LICENSE` | 21 | MIT, Copyright (c) 2026 Kotokvit | n/a | 🟢 Clean |
| `docs/architecture.md` | 620 | Canonical 4-layer architecture + math + roadmap | §1–§8 | 🟢 v1.0.0 canonical |
| `docs/PROMPT_PLAN.md` | 715 | Original v0.1.0 bootstrap prompt (12 stages) | n/a | 🔴 Stale artifact |
| `docs/llm-sandbox.md` | 326 | LlmSandbox / Validator trait contracts | §1–§13 | 🟢 v1.0.0 canonical |
| `docs/reasoning/SPEC.md` | 566 | Reasoning Engine v0.1 module contract | §0–§N | 🟢 Reference |
| `docs/poler_math/POLER_SPEC.md` | 1 007 | POLER[Ψ] v0.3 math spec (v1.1) | §0–§19 | 🟢 Reference |
| `docs/poler_math/INTEGRATION_ROADMAP.md` | — | POLER ↔ LitGraph integration plan | — | Reference |
| `docs/poler_math/QUESTIONS_FOR_MATHEMATICIAN.md` | — | 5 open math Qs (resolved v1.1) | — | Reference |
| `docs/poler_math/TOOLKIT.md` | — | Jupyter notebooks index | — | Reference |
| `docs/poler_math/notebooks/*.ipynb` | 4 files | Operator algebra, J-matrix, Clifford, topology | — | Verification |
| `docs/poler_math/sources/*` | 5 files | Raw POLER source materials | — | Citation |
| `docs/language-rules/README.md` | 153 | Reference hub for RU/UK morphology rules | §0–§4 | 🟢 Reference |
| `docs/language-rules/01–05-*.md` | 5 files | Russian declension, Ukrainian declension, proper names, verbs/aspect, SVO syntax | — | Reference |
| `docs/language-rules/stopwords-{ru,uk}.txt` | 2 files | 558 RU + 1 982 UK stopwords | — | Reference |
| `docs/language-rules/raw/*` | 11 files (~580 KB) | Wikipedia / Gramota / slovnyk dumps | — | Citation |
| `docs/education/README.md` | 68 | Math learning series index (6 chapters planned) | — | 🟡 2/6 written |
| `docs/education/01-sets-relations-predicates.md` | — | Ch 1: sets, relations, predicates | — | Ready v1.0.0 |
| `docs/education/02-graphs-dag.md` | — | Ch 2: graphs, DAG, topo-sort | — | Ready v1.0.0 |
| `docs/layer-g-planning/00-META_PROMPT.md` | 174 | THIS 17-subagent orchestration plan | §1–§6 | In-progress |

### 2.2 Public Types / Interfaces (canonical docs only)

- `LlmSandbox` trait (`docs/llm-sandbox.md` §12) — `fn generate(prompt, seed) -> CandidateText`
- `Validator` trait (`docs/llm-sandbox.md` §12) — `fn validate(candidate, base, task) -> ValidationResult`
- `Prompt`, `Context`, `ProjectedNode`, `CandidateText`, `ValidationResult` (`docs/llm-sandbox.md` §12)
- `SemanticIR`, `SemanticMention`, `SemanticRelation`, `Provenance`, `EpistemicType`, `ConfidenceFn`, `TaskSpec`, `WorldStateDelta` (`docs/architecture.md` §4)
- `Fact`, `Event`, `WorldState`, `Rule`, `Constraint`, `ContradictionReport`, `Hypothesis`, `TemporalAnchor` (`docs/reasoning/SPEC.md` §2)

### 2.3 Public Functions / Commands (documented vs actual)

- README claims "15 Tauri commands" — **actually 29 registered** in `src-tauri/src/lib.rs:37-76` (parse_md, parse_md_full, 4×project, 4×versions, export_project, 5×ai, 3×ner, 1×conflict, 6×reasoning, 3×poler).

## 3. Current State

- **What works (docs)**: `docs/architecture.md` (620 LOC, v1.0.0 canonical) is the single source of truth for L1/L1.5/L2/L3/L4 + Planner + LLM Context Builder + Validator; `docs/llm-sandbox.md` (326 LOC, v1.0.0 canonical) formalizes the LLM closed-loop contract; `docs/reasoning/SPEC.md` (566 LOC) is the v0.1 contract for `src-tauri/src/reasoning/`; `docs/poler_math/POLER_SPEC.md` (1 007 LOC, v1.1) covers POLER[Ψ] v0.3 math with 5 resolved Q&A; `docs/language-rules/` is a clean reference hub with raw source dumps; `LICENSE` is clean MIT.
- **What's stubbed**: `docs/education/` advertises 6 chapters but only 2 are written (03–06 marked "planned"); `docs/layer-g-planning/` contains only `00-META_PROMPT.md` (this orchestration), the `99-DETAILED_WORK_PROMPT.md` synthesis is still pending.
- **What's missing**: a dedicated Layer G specification document (see §4 below); CHANGELOG entries for 0.2.2 and commits `68d44f7`→`c143a7f` (≈17 commits, Layers A–F.2); README has no mention of POLER, Layer A–F, litgraph-core linguistic layer, reasoning engine, conflict graph, xtask, or Python sandbox.

## 4. Gaps / Bugs / TODOs

- **[BUG] README.md:58 — "47 shadcn/ui компонентов"** — actual count is **7** (`badge, button, dialog, dropdown-menu, input, label, textarea`). Off by ~6.7×.
- **[BUG] README.md:57 — "11 компонентов litgraph"** — actual count is **22** `.tsx` files in `src/components/litgraph/`. Off by 2×.
- **[BUG] README.md:54 — "15 Tauri commands"** — actual count is **29** registered handlers in `lib.rs:37-76`.
- **[BUG] README.md:64 & CHANGELOG.md:86 — "AiSettingsDialog.tsx"** — file does **not exist** in `src/components/litgraph/` (verified via LS).
- **[BUG] README.md:60 — "Все `fetch('/api/...')` заменены на `invoke('...')`"** — verified false: `AIDialog.tsx` contains **zero** `invoke` and **zero** `provider` references; AI commands will fail at runtime with `missing field provider` (per worklog line 18).
- **[BUG] CHANGELOG.md:1 — latest documented version is 0.2.1 (2026-08-07)** — actual version is **0.2.2** in `package.json:4`, `src-tauri/Cargo.toml:3`, `src-tauri/tauri.conf.json:3`. The 0.2.2 release is **undocumented**.
- **[BUG] CHANGELOG.md — commit `c143a7f` (Layer F.2) is NOT in CHANGELOG** — nor are the ~17 intervening commits covering Layer A (lemmatizer), Layer B (POS-tagger, 450 LanguageTool rules), Layer C (SVO + Epsilon v7.5), Layer E (NarrativeGraph + ConflictAnalyzer), Layer F.1 (Tauri IPC), Layer F.2 (React Visualizer), xtask, python ner_extract, conflict_graph.
- **[BUG] SECURITY.md:1 — entire file is mis-scoped** — it is a "how to handle your GitHub token" guide, not a security policy. Contains:
  - No CSP disclosure (tauri.conf.json has `csp: null` — worklog confirmed)
  - No mention of `capabilities/` (which actually EXISTS at `src-tauri/capabilities/default.json` with `fs:allow-read-file`/`allow-write-text-file` `**` wildcards — overbroad)
  - No Python sandbox disclosure (`src-tauri/python/{ner_extract,poler_entities,svo_extract}.py` spawn spaCy subprocess)
  - No vulnerability reporting process, no security contact, no CVE policy
  - No dependency audit / supply-chain policy
- **[TODO] README.md:66-70 — "🚧 Что осталось"** section is wildly stale (says only "Установить Rust и собрать"); doesn't mention Layer G, architectural debt, the duplicate `litgraph-core` vs `src-tauri/src` parser/models/ai code, the missing Semantic IR layer, the missing Z3 integration, or the missing `ConfidenceFn` trait.
- **[TODO] README.md:148-151 — project structure tree** is stale: omits `litgraph-core/`, `xtask/`, `src-tauri/python/`, `src-tauri/src/{poler,reasoning,linguistic,linguistic_entities,dict}/`, `src/lib/poler/`, `src/lib/conflict/`, `tests/corpus/`, `docs/{architecture.md,reasoning/,poler_math/,language-rules/,education/,llm-sandbox.md,layer-g-planning/}`.
- **[TODO] docs/PROMPT_PLAN.md (715 LOC)** — original v0.1.0 bootstrap prompt for an external AI helper; §8 "Критерии готовности v0.1.0" all done; §11 bootstrap commands no longer accurate; §12 references Z.ai preview URL. Should be archived or moved to `docs/history/`.
- **[TODO] docs/education/ — 4 of 6 chapters unwritten** (03 SAT/SMT, 04 probabilities/κ, 05 Rust contracts, 06 E2E example).
- **[TODO] Layer G documentation gap (CRITICAL)** — **NO dedicated Layer G spec exists**. Searched `docs/` and root: only `docs/layer-g-planning/00-META_PROMPT.md` (this orchestration), no `POLER_LAYER_G_*.md`, no `docs/layer-g-planning/SPEC.md`. Layer G is referenced in:
  - `docs/architecture.md` §3.8 (LLM Loop) + §6 Roadmap item 5 ("Budget + escalation in LLM loop") — implicit only
  - `docs/llm-sandbox.md` §1–§13 (the LlmSandbox/Validator contract that Layer G must implement)
  - `docs/layer-g-planning/00-META_PROMPT.md:14` lists "Layer G implementation plan" as a TODO output
  - `worklog.md:434,443,500,546` records design decisions made in worklog but never crystallized into a spec doc
  - **Contrast**: Layer F has its own `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` (separate subagent scope); Layer G (the explicitly next planned step) has none.

## 5. Refactoring Opportunities

- **[REFACTOR] Split README.md** into a lean user-facing `README.md` (install, run, AI providers, license) + `docs/ARCHITECTURE_OVERVIEW.md` (canonical layer reference, status table, roadmap). Keeps the canonical source-of-truth in `docs/architecture.md`, avoids drift in README. — **Benefit**: eliminates the 6 stale claims above; single source of truth for component counts and command list (auto-generate from `lib.rs:37-76`).
- **[REFACTOR] Rewrite SECURITY.md** as a real policy: scope (Tauri 2 desktop app), threat model (local LLM, local files, no network egress by design per `llm-sandbox.md` F1), known issues (`csp: null` in `tauri.conf.json:24`, overbroad `fs:**` in `capabilities/default.json:25-42`, Python subprocess in `src-tauri/python/`), reporting process (GitHub Security Advisory), disclosure timeline. Move the GitHub-token guide to `CONTRIBUTING.md`. — **Benefit**: aligns with industry norms; surfaces real risks currently invisible to reviewers.
- **[REFACTOR] CHANGELOG.md backfill** — add `## [0.2.2] — 2026-08-08` covering Layers A–F.2 (lemmatizer, POS-tagger 450 rules, SVO+Epsilon v7.5, NarrativeGraph+ConflictAnalyzer, Tauri IPC, React Visualizer, xtask, python ner/svo extract). Then add `## [Unreleased] — Layer G` section. — **Benefit**: closes the version drift; gives reviewers a map of what changed since 0.2.1.
- **[REFACTOR] Archive docs/PROMPT_PLAN.md** → `docs/history/PROMPT_PLAN_v0.1.0.md`; add a one-line pointer from README to current roadmap (`docs/architecture.md` §6). — **Benefit**: removes 715 lines of stale instructions that contradict the current code.
- **[REFACTOR] Auto-generate README's "Структура проекта" tree** from `find . -type d` in CI, or use a `// SECTION: tree` marker with a script. — **Benefit**: prevents the tree from drifting again.
- **[REFACTOR] Add `docs/layer-g-planning/SPEC.md`** — fill the Layer G documentation gap (see §4). Should mirror the structure of `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md`: rationale, Rust struct signatures (`LlmBridge`, `HypothesisReport`, `BudgetTracker`), Tauri command signatures (`cmd_generate_llm_hypotheses(text, provider, options) -> HypothesisReportDto`), TS interface contract, prompt templates, validation flow against `Validator` trait, integration with `cmd_detect_paradoxes`. — **Benefit**: unblocks Layer G implementation; converts worklog design notes into reviewable artifact.

## 6. Layer G Relevance

**This subagent's scope is the Layer G documentation surface itself.** Findings:

1. **No Layer G spec exists** — `docs/layer-g-planning/` contains only `00-META_PROMPT.md` (this very orchestration plan), no `SPEC.md`. By contrast, Layer F.1/F.2 each got dedicated specs (`POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md`), and Layers A–E are covered by `POLER_UA_LP_MASTER_ROADMAP_V8.md` + `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md` (subagent 13 scope).
2. **Layer G is the explicit "next planned step"** — `worklog.md:393` ("Next planned step: Layer G (LLM Reasoning Bridge) — generate prompts for AI when temporal paradoxes are detected"), `00-META_PROMPT.md:14` (objective #2), `docs/architecture.md` §6 Roadmap item 5.
3. **Layer G's mathematical contract is already specified** — `docs/llm-sandbox.md` §7 closed-loop formula `Nodes →π Context →Template Prompt →LLM T_cand →NLP/IR/Event ΔW →Validator Nodes'` and §12 Rust traits (`LlmSandbox`, `Validator`, `Prompt`, `Context`, `ProjectedNode`, `CandidateText`, `ValidationResult`) are precisely what Layer G must implement. The contract is ready; the **integration spec** (which Tauri command consumes which Layer E output, how the budget loop wires into React UI, which prompt templates ship first) is what's missing.
4. **Layer G design notes exist only in worklog** — `worklog.md:434` (readiness assessment: "contract достаточен для non-streaming single-shot RU/UK/EN задач; для production Layer G нужны R3 (AiClient trait) + R4 (HTTP status hardening)"), `:443-444` (what Layer G can reuse vs must add), `:491-500` (planned `commands/llm_bridge.rs` module + `cmd_generate_llm_hypotheses` signature), `:518-521` (architectural concern: two parallel reasoning systems in `litgraph-core/src/reasoning/` vs `src-tauri/src/reasoning/`; `Paradox` lacks `id` and `evidence_text`). None of this is in a reviewable spec doc.
5. **`docs/llm-sandbox.md` is the Layer G contract** — already canonical v1.0.0; Layer G spec should reference it, not duplicate.

## 7. Recommended Next Actions

1. **Create `docs/layer-g-planning/SPEC.md`** — Layer G implementation blueprint (Rust traits, Tauri command signatures, prompt templates, Validator integration, budget loop, React UI surface). Effort: **M (4–6 h)**. Blocks: Layer G implementation; unblocked by: this 17-subagent dissection producing `99-DETAILED_WORK_PROMPT.md` Part C.
2. **Backfill CHANGELOG.md** — add `## [0.2.2] — 2026-08-08` section covering Layers A–F.2 (≈17 commits, `68d44f7`→`c143a7f`). Effort: **S (1 h)**. Blocks: 0.3.0 release notes; unblocked by: nothing.
3. **Rewrite SECURITY.md** as a real policy + move token guide to `CONTRIBUTING.md`. Effort: **S (1–2 h)**. Blocks: nothing; unblocked by: nothing.
4. **Fix README.md stale claims** — 7→"7 shadcn/ui components", 11→"22 litgraph components", 15→"29 Tauri commands", remove `AiSettingsDialog.tsx` mention, fix `fetch→invoke` claim, refresh "🚧 Что осталось" with Layer G + architectural debt, refresh project structure tree. Effort: **S (1–2 h)**. Blocks: nothing; unblocked by: nothing.
5. **Archive `docs/PROMPT_PLAN.md`** → `docs/history/PROMPT_PLAN_v0.1.0.md` + add pointer. Effort: **XS (15 min)**.
6. **Finish `docs/education/` chapters 03–06** (SAT/SMT, probabilities/κ, Rust contracts, E2E example). Effort: **L (per chapter 4–8 h)**. Low priority, blocks: only onboarding of new contributors.
7. **Auto-generate component/command counts** in README via a `scripts/check_readme_stats.ts` CI check that compares README claims against `src/components/ui/`, `src/components/litgraph/`, and `lib.rs:37-76`. Effort: **S (1–2 h)**. Prevents recurrence of the "47 vs 7" drift.

## 8. Dependencies / Blockers

- **Depends on**:
  - This subagent's report + the 16 other subagent reports → synthesized into `99-DETAILED_WORK_PROMPT.md` Part C (Layer G plan).
  - Layer G spec (#1 above) needs the cross-cutting findings from subagents 5 (commands), 8 (tauri-commands.ts IPC), 11 (AIDialog/AssistantDialog plumbing bug), 13 (POLER roadmap), 14 (POLER Layer F spec), 17 (gap analysis) to ground Tauri command signatures and prompt-template choices.
- **Blocks**:
  - Layer G implementation (no spec → no implementation per `00-META_PROMPT.md` §6 success criterion "Layer G plan includes concrete Rust struct signatures + TS interfaces + Tauri command signatures").
  - 0.3.0 release (CHANGELOG backfill is prerequisite to writing 0.3.0 release notes).
  - Public release readiness (SECURITY.md must exist as a real policy before any external contributor PRs).

---

## 3-Sentence Summary (for main agent)

README.md, CHANGELOG.md, and SECURITY.md are all materially inaccurate against the actual codebase: README claims 47 shadcn/ui components (actually 7), 11 litgraph components (actually 22), 15 Tauri commands (actually 29), and a non-existent `AiSettingsDialog.tsx`; CHANGELOG stops at 0.2.1 while code is at 0.2.2 with ~17 undocumented commits including Layer F.2 (`c143a7f`); SECURITY.md is a GitHub-token how-to rather than a real policy and is silent on the `csp: null` in `tauri.conf.json:24`, the overbroad `fs:**` wildcards in `capabilities/default.json:25-42`, and the Python subprocess sandbox. The critical Layer G documentation gap is confirmed: despite Layer G being the explicitly next planned step (`worklog.md:393`, `00-META_PROMPT.md:14`), there is **no `POLER_LAYER_G_*.md` or `docs/layer-g-planning/SPEC.md`** — Layer G's mathematical contract lives in `docs/llm-sandbox.md` (canonical v1.0.0) and its design notes are scattered across `worklog.md:434,443,500,518-521`, but no reviewable integration spec exists. Recommended next actions in priority order: (1) author `docs/layer-g-planning/SPEC.md` from the synthesized Part C of `99-DETAILED_WORK_PROMPT.md`, (2) backfill CHANGELOG 0.2.2, (3) rewrite SECURITY.md as a real policy, (4) fix README's six stale claims.
