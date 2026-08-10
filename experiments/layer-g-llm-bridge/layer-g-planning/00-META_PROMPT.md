# META_PROMPT: Orchestration Plan for 17-Subagent Atomic Dissection

> **Document ID**: `00-META_PROMPT.md`
> **Purpose**: Orchestrate 17 parallel subagents to dissect the entire `litgraph-desktop` codebase + all planning documents into atomic reports, then synthesize them into a single **DETAILED_WORK_PROMPT.md** for user approval.
> **Execution Time Budget**: 30–40 minutes (parallel execution)
> **Repository State**: `c143a7f` on `feature/symbolic-ua-lp-engine` (Layer F.2 complete)

---

## 1. Strategic Objective

The user wants a **detailed, reviewable work prompt** that covers:
1. GUI refactoring opportunities (current pain points in the React frontend)
2. Layer G (LLM Reasoning Bridge) implementation plan
3. Architectural debt cleanup (duplicated code, missing tests, security issues)
4. Roadmap alignment (what's planned in POLER_*.md vs what's actually implemented)

This cannot be produced by a single agent reading 177 source files + 8 planning docs.
Instead, we spawn **17 specialized subagents**, each owning a specific scope, and
synthesize their outputs into a master prompt.

---

## 2. Subagent Roster (17 agents in 3 cohorts)

### Cohort A — Rust Backend Dissectors (7 agents)

| # | Agent ID | Scope | Output File |
|---|----------|-------|-------------|
| 1 | `01-core-linguistic` | `litgraph-core/src/linguistic/{lemmatizer,pos_tagger,svo_parser,mod}.rs` + `litgraph-core/src/dict/*` | `01-core-linguistic.md` |
| 2 | `02-core-parser` | `litgraph-core/src/parser/{chapters,characters,locations,themes,epsilon,mod}.rs` | `02-core-parser.md` |
| 3 | `03-core-reasoning` | `litgraph-core/src/reasoning/{narrative_graph,paradox,stub,mod}.rs` | `03-core-reasoning.md` |
| 4 | `04-core-models-ai` | `litgraph-core/src/{models,ai}/*` + `languagetool_weights.rs` + `ukrainian_semantic_categories.rs` + `lib.rs` | `04-core-models-ai.md` |
| 5 | `05-tauri-commands` | `src-tauri/src/commands/{parse_md,parse_md_full,ner,ai,poler,reasoning,conflict,project,versions,export,mod}.rs` | `05-tauri-commands.md` |
| 6 | `06-tauri-poler-bridge` | `src-tauri/src/poler/mod.rs` + mirror modules in `src-tauri/src/{parser,linguistic,dict,ai,models,linguistic_entities,languagetool_weights,ukrainian_semantic_categories}` | `06-tauri-poler-bridge.md` |
| 7 | `07-tauri-python-xtask` | `src-tauri/python/{ner_extract,poler_entities,svo_extract}.py` + `xtask/src/*` + `scripts/*` + `build.rs` | `07-tauri-python-xtask.md` |

### Cohort B — TS/React Frontend Dissectors (5 agents)

| # | Agent ID | Scope | Output File |
|---|----------|-------|-------------|
| 8 | `08-ts-lib-tauri` | `src/lib/tauri-commands.ts` (DTOs + IPC wrappers — the Layer F.2 contract) | `08-ts-lib-tauri.md` |
| 9 | `09-ts-lib-poler-litgraph` | `src/lib/poler/{polerCore,analyze,nerBridge,nerTypes,textGraph,clustering}.ts` + `src/lib/litgraph/{store,types,export,api}.ts` + `src/lib/conflict/*` + `src/lib/utils.ts` | `09-ts-lib-poler-litgraph.md` |
| 10 | `10-components-main` | `src/components/litgraph/{LitApp,Toolbar,LitCanvas,CanvasRenderer,Sidebar,NodeEditor,NodeActions,Inspector,NodePalette,LitNodeView,LitEdgeView}.tsx` | `10-components-main.md` |
| 11 | `11-components-dialogs` | `src/components/litgraph/{AIDialog,AssistantDialog,PolerDialog,PolerPanel,SvoHighlighter,NerDialog,CharacterGraphDialog,ConflictGraphDialog,ReasoningDialog,ReaderDialog,TextMomentsDialog}.tsx` | `11-components-dialogs.md` |
| 12 | `12-components-ui-config` | `src/components/ui/*` + `components.json` + `src/globals.css` + `src/App.tsx` + `src/main.tsx` + `src/types/*` + `tsconfig.json` + `vite.config.ts` + `tailwind.config` + `package.json` | `12-components-ui-config.md` |

### Cohort C — Plans, Docs & Cross-Cutting (5 agents)

| # | Agent ID | Scope | Output File |
|---|----------|-------|-------------|
| 13 | `13-docs-poler-roadmap` | `POLER_UA_LP_MASTER_ROADMAP_V8.md` + `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md` | `13-docs-poler-roadmap.md` |
| 14 | `14-docs-poler-specs` | `POLER_EPSILON_CANONICAL_SPECIFICATION.md` + `POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md` + `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` | `14-docs-poler-specs.md` |
| 15 | `15-docs-readme-changelog` | `README.md` + `CHANGELOG.md` + `SECURITY.md` + `LICENSE` + `docs/architecture.md` + `docs/PROMPT_PLAN.md` + `docs/reasoning/*` + `docs/poler_math/*` + `docs/language-rules/*` + `docs/education/*` | `15-docs-readme-changelog.md` |
| 16 | `16-tests-ci-build` | `litgraph-core/tests/*` + `litgraph-core/examples/*` + `tests/*` + `.github/workflows/*` + `tauri.conf.json` + `Cargo.toml` (both) + `src-tauri/Cargo.toml` + `src-tauri/tauri.conf.json` + `src-tauri/capabilities/*` (if exists) | `16-tests-ci-build.md` |
| 17 | `17-gap-analysis` | **Cross-cutting**: `rg TODO|FIXME|HACK|XXX` across repo; check AI provider plumbing (UI→invoke→Rust); CSP audit in tauri.conf.json; capabilities/ existence; duplicated code between `litgraph-core/src/*` and `src-tauri/src/*` (md5sum compare); store.ts localStorage vs Tauri Store plugin; Python sandbox; README vs actual UI component count | `17-gap-analysis.md` |

---

## 3. Uniform Output Format (every subagent must follow)

Each subagent writes its report to `/home/z/my-project/subagent-reports/<id>.md`
using this exact structure:

```markdown
# Subagent <id>: <human-readable name>

## 1. Scope
- Files inspected: <count>
- Total LOC: <count>
- Key entry points: <file:line refs>

## 2. Atomic Inventory
### 2.1 Modules / Files
| File | LOC | Purpose | Public API | Dependencies |
|------|-----|---------|------------|--------------|
| ... | ... | ... | ... | ... |

### 2.2 Public Types / Interfaces
- `<TypeName>` (file:line) — <one-line purpose>

### 2.3 Public Functions / Commands
- `<fn_name>(...)` (file:line) — <one-line purpose>

## 3. Current State
- What works: ...
- What's stubbed: ...
- What's missing: ...

## 4. Gaps / Bugs / TODOs
- [BUG] <description> (file:line)
- [TODO] <description> (file:line)
- [STUB] <description> (file:line)

## 5. Refactoring Opportunities
- [REFACTOR] <description> — <expected benefit>

## 6. Layer G Relevance
- How this module relates to the LLM Reasoning Bridge (prompts, validation, world-state, paradox feed)

## 7. Recommended Next Actions
1. <action> — <effort estimate>
2. <action> — <effort estimate>

## 8. Dependencies / Blockers
- Depends on: ...
- Blocks: ...
```

---

## 4. Execution Protocol

### Phase 2 — Parallel Subagent Execution (15–20 min)

The main agent fires **all 17 Task tool calls in a single message** (parallel).

Each subagent:
1. Reads `/home/z/my-project/worklog.md` to understand prior context
2. Reads its assigned files
3. Writes its atomic report to `/home/z/my-project/subagent-reports/<id>.md`
4. Appends a brief entry to `/home/z/my-project/worklog.md`
5. Returns a short summary to the main agent

### Phase 3 — Synthesis (10–15 min)

The main agent:
1. Reads all 17 reports
2. Cross-references findings (e.g., agent 17's "AI provider missing" gap vs agent 11's dialog analysis vs agent 5's command signature)
3. Produces `99-DETAILED_WORK_PROMPT.md` structured as:
   - **Part A**: Executive Summary (current state of the project)
   - **Part B**: GUI Refactoring Plan (priority-ordered, with file:line refs)
   - **Part C**: Layer G Implementation Plan (file-by-file blueprint)
   - **Part D**: Architectural Debt Cleanup (duplicated code, security, tests, CI)
   - **Part E**: Roadmap Alignment (planned vs implemented vs gap)
   - **Part F**: Concrete Task List (numbered, with effort estimates & dependencies)
   - **Part G**: Risk Analysis & Mitigations
   - **Part H**: Verification Checklist

### Phase 4 — Commit & Push

Both documents committed to `docs/layer-g-planning/`:
- `00-META_PROMPT.md` (this file — orchestration plan)
- `99-DETAILED_WORK_PROMPT.md` (synthesized master prompt)
- Plus 17 individual subagent reports (for traceability)

Commit message: `docs(layer-g): meta-prompt + 17 atomic dissection reports + detailed work prompt`

### Phase 5 — User Review

User reviews `99-DETAILED_WORK_PROMPT.md`, approves/modifies, then main agent executes.

---

## 5. Constraints

1. **Read-only**: Subagents do NOT modify any source files — only read & analyze.
2. **Atomic granularity**: Every public type, function, command must be inventoried.
3. **Source of truth**: Rust DTOs > spec docs (when they disagree).
4. **No hallucination**: If a file doesn't exist, say so — don't invent.
5. **Time budget**: Each subagent ≤ 5 min wall-clock. If scope is too large, prioritize entry points.
6. **Language**: Reports in Ukrainian/English mix (technical terms in English, descriptions in Ukrainian where natural).

---

## 6. Success Criteria

The `99-DETAILED_WORK_PROMPT.md` is considered successful if:
- ✅ A user reading only that document can understand the entire project state
- ✅ Every refactoring recommendation has a specific file:line reference
- ✅ Layer G plan includes concrete Rust struct signatures + TS interfaces + Tauri command signatures
- ✅ Task list is ordered by dependency (no task blocks another)
- ✅ Effort estimates are realistic (S/M/L with hour ranges)
- ✅ Risk analysis covers at least 5 risks with mitigations
