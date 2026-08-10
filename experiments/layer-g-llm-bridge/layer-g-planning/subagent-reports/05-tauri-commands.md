# Subagent 05 — Tauri IPC Commands (`src-tauri/src/commands/`)

- Task ID: `05-tauri-commands`
- Agent: Explore (very thorough)
- Scope: ALL Tauri IPC command modules in `src-tauri/src/commands/`, plus `lib.rs` invoke_handler registration and `main.rs`
- Repo root inspected: `/home/z/my-project/litgraph-desktop/`

---

## 1. Files Inspected

| File | LOC | Purpose |
|---|---:|---|
| `src-tauri/src/main.rs` | 7 | Trivial — `litgraph_desktop_lib::run()` only |
| `src-tauri/src/lib.rs` | 79 | Registers `tauri::generate_handler!` with 29 commands + 3 plugins (`store`, `dialog`, `fs`) + setup hook for `~/.local/share/litgraph/{projects,backups}` dirs |
| `src-tauri/src/commands/mod.rs` | 13 | Declares 10 submodules (`parse_md`, `parse_md_full`, `project`, `versions`, `export`, `ai`, `ner`, `conflict`, `reasoning`, `poler`) |
| `src-tauri/src/commands/parse_md.rs` | 12 | `.md` → graph (Rust-only, delegates to `parser::build_graph`) |
| `src-tauri/src/commands/parse_md_full.rs` | 349 | v0.4.0 auto-pipeline: Rust parse → Python NER enrich → merge |
| `src-tauri/src/commands/project.rs` | 27 | CRUD over `~/.local/share/litgraph/projects/*.litgraph` via `storage` |
| `src-tauri/src/commands/versions.rs` | 165 | Chapter-version snapshots stored in `node.data.versions` (max 50) |
| `src-tauri/src/commands/export.rs` | 213 | JSON / text / markdown export to disk |
| `src-tauri/src/commands/ai.rs` | 62 | AI assistant / continue-chapter / analyze-plot / test-connection / list-ollama-models |
| `src-tauri/src/commands/ner.rs` | 238 | Python-backed NER (spaCy + pymorphy3) via temp-dir subprocess |
| `src-tauri/src/commands/conflict.rs` | 144 | SVO → J-matrix → directed conflict graph (Python pipeline) |
| `src-tauri/src/commands/reasoning.rs` | 580 | Wave-5 reasoning engine: extract-events / extract-instructions / run-cycle / run-cycle-with-ir / get-world-state / validate-text |
| `src-tauri/src/commands/poler.rs` | 705 | Layer F.1: `cmd_compute_epsilon_climax`, `cmd_extract_svo`, `cmd_detect_paradoxes` (pure Rust, no Python) |

Total: 11 files, ~2 635 LOC.

---

## 2. Invoke Handler Registration — Completeness Audit

`lib.rs` registers **29 commands** in `tauri::generate_handler!` (lines 37–76):

```
parse_md, parse_md_full,
list_projects, load_project, save_project, delete_project,
save_version, restore_version, delete_version, list_versions,
export_project,
ai_assistant, ai_continue_chapter, ai_analyze_plot, ai_test_connection, ai_list_ollama_models,
extract_entities, analyze_characters, extract_svo,
get_conflict_graph,
reasoning_extract_events, reasoning_extract_instructions, reasoning_run_cycle, reasoning_run_cycle_with_ir, reasoning_get_world_state, reasoning_validate_text,
cmd_compute_epsilon_climax, cmd_extract_svo, cmd_detect_paradoxes
```

Cross-check against `#[tauri::command]` definitions:

| Module | # `#[tauri::command]` fns | Registered? |
|---|---:|---|
| `parse_md.rs` | 1 (`parse_md`) | ✅ |
| `parse_md_full.rs` | 1 (`parse_md_full`) | ✅ |
| `project.rs` | 4 (`list_projects`, `load_project`, `save_project`, `delete_project`) | ✅ |
| `versions.rs` | 4 (`save_version`, `restore_version`, `delete_version`, `list_versions`) | ✅ |
| `export.rs` | 1 (`export_project`) | ✅ |
| `ai.rs` | 5 (`ai_assistant`, `ai_continue_chapter`, `ai_analyze_plot`, `ai_test_connection`, `ai_list_ollama_models`) | ✅ |
| `ner.rs` | 3 (`extract_entities`, `analyze_characters`, `extract_svo`) | ✅ |
| `conflict.rs` | 1 (`get_conflict_graph`) | ✅ |
| `reasoning.rs` | 6 (`reasoning_extract_events`, `reasoning_extract_instructions`, `reasoning_run_cycle_with_ir`, `reasoning_run_cycle`, `reasoning_get_world_state`, `reasoning_validate_text`) | ✅ |
| `poler.rs` | 3 (`cmd_compute_epsilon_climax`, `cmd_extract_svo`, `cmd_detect_paradoxes`) | ✅ |

**Total: 29/29 commands registered. 0 missing.** No orphan `#[tauri::command]` exists in any commands/ file.

> Naming-collision watch: `extract_svo` (ner.rs, Python-backed, returns `serde_json::Value`) and `cmd_extract_svo` (poler.rs, Rust-native, returns `Vec<SvoTripletDto>`) coexist by design — see poler.rs lines 26–31. The `cmd_` prefix is the documented disambiguator. Frontend `cmdExtractSvo` is the Layer-F path; `extractSvo` is legacy.

---

## 3. Command Signatures + Wire Field Names

All commands are `async`, return `Result<T, String>`, and use Tauri's automatic snake_case ↔ camelCase conversion for argument names. DTOs explicitly use `#[serde(rename_all = "camelCase")]` (or per-field `#[serde(rename = "...")]`) so JSON keys on the wire are camelCase.

### 3.1 `parse_md`
```rust
pub async fn parse_md(params: ParseParams) -> Result<ParseResult, String>
```
Wire payload: `{ params: { markdown, projectTitle, author } }` (TS wraps via `parseMd`).

### 3.2 `parse_md_full`
```rust
pub async fn parse_md_full(params: ParseParams) -> Result<FullParseResult, String>
```
Wire payload: `{ params: { markdown, projectTitle, author } }`. Returns `FullParseResult` (`parseResult`, `nerEntities`, `nerMerged`, `pipelineVersion`).

### 3.3 `project` (4 commands)
```rust
pub async fn list_projects()                                          -> Result<Vec<ProjectMeta>, String>
pub async fn load_project(id: String)                                 -> Result<Project, String>
pub async fn save_project(id: String, project: Project)               -> Result<(), String>
pub async fn delete_project(id: String)                               -> Result<(), String>
```
Wire: `{ id }`, `{ id, project }`. No DTOs — `Project` / `ProjectMeta` from `crate::models`.

### 3.4 `versions` (4 commands)
```rust
pub async fn save_version(projectId: String, nodeId: String,
                          label: Option<String>, source: Option<String>)
    -> Result<ChapterVersion, String>
pub async fn restore_version(projectId: String, nodeId: String, versionId: String) -> Result<(), String>
pub async fn delete_version(projectId: String, nodeId: String, versionId: String) -> Result<(), String>
pub async fn list_versions(projectId: String, nodeId: String) -> Result<Vec<ChapterVersion>, String>
```
Wire: all fields camelCase via Tauri convention. `ChapterVersion` (id, timestamp, fullText, wordCount, label?, source?) inherits its serde config from `models::ChapterVersion`.

### 3.5 `export`
```rust
pub async fn export_project(project: Project, format: String, path: String) -> Result<(), String>
```
Wire: `{ project, format, path }` where `format ∈ {"json","text","markdown","md"}`.

### 3.6 `ai` (5 commands) — **see §5 for provider plumbing bug**
```rust
pub async fn ai_assistant(
    project: Project,
    message: String,
    history: Vec<ChatMessage>,
    selected_node_id: Option<String>,
    provider: AiProvider,
) -> Result<String, String>

pub async fn ai_continue_chapter(
    project: Project,
    from_chapter_id: Option<String>,
    custom_prompt: Option<String>,
    provider: AiProvider,
) -> Result<String, String>

pub async fn ai_analyze_plot(project: Project, focus: String, provider: AiProvider) -> Result<String, String>

pub async fn ai_test_connection(provider: AiProvider) -> Result<bool, String>

pub async fn ai_list_ollama_models(url: String) -> Result<Vec<String>, String>
```

`AiProvider` enum (`src-tauri/src/ai/mod.rs` lines 13–29):
```rust
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiProvider {
    Ollama { url: String, model: String },
    OpenAiCompat { endpoint: String, api_key: String, model: String },
    Zai { api_key: String, model: String },
}
```
Wire shape (internally tagged): `{ "type": "ollama", "url": "...", "model": "..." }`.

### 3.7 `ner` (3 commands)
```rust
pub async fn extract_entities(text: String) -> Result<NerResult, String>
pub async fn analyze_characters(text: String) -> Result<serde_json::Value, String>
pub async fn extract_svo(text: String) -> Result<serde_json::Value, String>
```
`NerResult` (no `camelCase` rename_all — uses per-field `#[serde(rename = "...")]`): `entities`, `stats`, `model`, `version`, `truncated`, `textLength`, `processedLength`, `chunksProcessed?`. `Entity` carries `firstMention` (renamed). `analyze_characters` / `extract_svo` return passthrough JSON from Python.

### 3.8 `conflict`
```rust
pub async fn get_conflict_graph(text: String) -> Result<ConflictGraph, String>
```
`ConflictGraph` uses per-field `#[serde(rename = "...")]` for `from`, `to`, `verbCount`, `pronounResolved`, `nodeCount`, `edgeCount`, `rawTripletCount`, `nodeOrder`, `svoVersion`, `textLength`. (No `camelCase` rename_all — individual renames only.)

### 3.9 `reasoning` (6 commands)
```rust
pub async fn reasoning_extract_events(text: String, project: Project)
    -> Result<Vec<Event>, String>

pub async fn reasoning_extract_instructions(text: String, project: Project)
    -> Result<Vec<SemanticInstruction>, String>

pub async fn reasoning_run_cycle_with_ir(project: Project, instructions: Vec<SemanticInstruction>)
    -> Result<CycleWithIrReport, String>

pub async fn reasoning_run_cycle(project: Project, events: Vec<Event>)
    -> Result<CycleReport, String>

pub async fn reasoning_get_world_state(project: Project, events: Vec<Event>)
    -> Result<WorldStateView, String>

pub async fn reasoning_validate_text(project: Project, events: Vec<Event>, proposed_text: String)
    -> Result<ValidationResultDto, String>
```

DTOs (defined in `reasoning.rs` lines 43–132):
- `WorldStateView` — `#[serde(rename_all = "camelCase")]` + explicit `#[serde(rename = "violationCount")]` / `#[serde(rename = "paradoxCount")]`.
- `ValidationResultDto` — `#[serde(tag = "kind", rename_all = "lowercase")]` with three variants `accept` / `reject` / `retry`; `Reject.feedbackPrompt` is explicitly renamed.
- `CharacterState` — `camelCase` (id, title, attributes, isAlive, location).

Note: `Event`, `CycleReport`, `CycleWithIrReport`, `SemanticInstruction`, `ConstraintViolation`, `TemporalParadox`, `TemporalAnchor`, `Action`, `FactValue`, `Provenance` are re-exported from `crate::reasoning::*` — their serde config lives there, not in `commands/reasoning.rs`.

### 3.10 `poler` (3 commands) — Layer F.1
```rust
pub async fn cmd_compute_epsilon_climax(
    chapter_text: String,
    keyword: Option<String>,
    kappa: Option<f64>,
) -> Result<EpsilonClimaxDto, String>

pub async fn cmd_extract_svo(text: String) -> Result<Vec<SvoTripletDto>, String>

pub async fn cmd_detect_paradoxes(text: String) -> Result<ParadoxReportDto, String>
```

DTOs (all `#[serde(rename_all = "camelCase")]`):
- `EpsilonClimaxDto` — epsilon, normalized, wordCount, uniqueWords, emotionCount, kwCount, canonCount, actionCount, thetaRel, isNoise, isClimax, formulaVariant, omegaConf, spectralRadius, nodeCount, edgeCount.
- `SvoTripletDto` — actor, verb, target?, instrument?, location?, polarity, confidence.
- `ParadoxDto` — kind, character, chapterIdx, originChapterIdx, explanation.
- `ChapterBreakdownDto` — chapterIdx, title, characterCount, tripletCount, characters.
- `ParadoxReportDto` — paradoxes, chapters, totalCharacters, totalTriplets.

Wire payloads (camelCase): `{ chapterText, keyword: null|string, kappa }`, `{ text }`, `{ text }`.

---

## 4. Layer F.1 → Layer G Bridge — Patterns to Follow for `cmd_generate_llm_hypotheses`

Layer G (LLM Reasoning Bridge) per `worklog.md` Stage Summary (line 393) is the planned next step: *"generate prompts for AI when temporal paradoxes are detected."* The integration point is `commands/poler.rs`. Patterns to mirror:

### 4.1 Module placement
- New command goes into `commands/poler.rs` (extending the existing 3-command Layer F.1 set) OR a sibling `commands/llm_bridge.rs` registered in `mod.rs` and `lib.rs::generate_handler!`.
- If a separate `commands/llm_bridge.rs` is chosen, follow the exact `mod.rs` + `lib.rs` two-line registration dance (declare `pub mod llm_bridge;` in `commands/mod.rs` and add `commands::llm_bridge::cmd_generate_llm_hypotheses` to `generate_handler!`).

### 4.2 DTO contract pattern (camelCase)
Poler.rs defines DTOs that *mirror* core types but with `#[serde(rename_all = "camelCase")]`. For Layer G, define:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmHypothesisDto {
    pub hypothesis_id: String,
    pub paradox_ref: ParadoxDto,        // reuse poler DTO
    pub prompt: String,                 // full system+user prompt
    pub provider_hint: String,          // "ollama" / "openai" / "zai"
    pub temperature: f64,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HypothesisReportDto {
    pub paradoxes: Vec<ParadoxDto>,
    pub hypotheses: Vec<LlmHypothesisDto>,
    pub total_paradoxes: usize,
}
```

### 4.3 Command signature pattern
Mirror `cmd_detect_paradoxes` — accept `text: String` (or accept a pre-computed `ParadoxReportDto` to avoid re-running Layer E):

```rust
#[tauri::command]
pub async fn cmd_generate_llm_hypotheses(
    text: String,
    provider: AiProvider,               // reuse crate::ai::AiProvider
    options: Option<HypothesisOptions>, // kappa, temperature, max_tokens
) -> Result<HypothesisReportDto, String>
```

Why `provider: AiProvider`: the existing `ai::*` commands all take this enum (`ai/mod.rs` lines 13–29), so Layer G should reuse it rather than redefining — keeps the wire shape identical to `ai_assistant`/`ai_analyze_plot`. Frontend already has the shape (see `tauri-commands.ts` lines 71–97).

### 4.4 Layer-E consumption pattern
`cmd_detect_paradoxes` (poler.rs lines 396–472) shows the canonical recipe:

1. `detect_chapters(&text)` → `Vec<ParsedChapter>` (Layer A).
2. Per chapter: `detect_characters(ch_text)` (Layer B) + `SvoParser::new().parse_text(ch_text)` (Layer C).
3. Aggregate into `ManuscriptAnalysis` (Layer E struct).
4. `ParadoxDetector::new().detect(&manuscript)` → `Vec<Paradox>`.
5. Map to DTOs via `ParadoxDto::from(p)`.

For Layer G, the new command should call `cmd_detect_paradoxes`'s internal pipeline (or refactor it into a `pub(crate) fn detect_paradoxes_inner(text: &str) -> Result<(Vec<Paradox>, Vec<ChapterBreakdownDto>), String>`) and then for each `Paradox` construct an LLM prompt — most likely reusing `crate::ai::prompts::*` helpers or adding a new `build_paradox_hypothesis_prompt(paradox, project)` function.

### 4.5 Determinism note
The poler.rs module header (lines 32–37) explicitly states: *"All three commands are pure and deterministic: same input ⇒ same output. No I/O, no global mutable state, no LLM calls."* The Layer G command will **break** this contract by definition (it calls `ai::chat`). This must be called out in the new module's rustdoc — either:
- Place Layer G in a separate `commands/llm_bridge.rs` to keep `poler.rs` "pure symbolic" zone, OR
- Document clearly inside `poler.rs` that `cmd_generate_llm_hypotheses` is the single non-deterministic exception.

Recommendation: **separate module** (`commands/llm_bridge.rs`) — preserves the poler.rs purity invariant and mirrors the `ai.rs` / `poler.rs` separation.

### 4.6 Registration checklist for the new command
1. Create `src-tauri/src/commands/llm_bridge.rs` with `#[tauri::command] pub async fn cmd_generate_llm_hypotheses(...)`.
2. Add `pub mod llm_bridge;` to `commands/mod.rs` (after `pub mod poler;`).
3. Add `commands::llm_bridge::cmd_generate_llm_hypotheses,` to `generate_handler!` in `lib.rs` (after the `// poler` block, lines 72–75).
4. Add TS wrapper in `src/lib/tauri-commands.ts` mirroring `cmdDetectParadoxes` (lines 437–439).
5. (Optional) Add a `libgraph_core::reasoning::*` re-export in `src-tauri/src/poler/mod.rs` if new core types are needed.

---

## 5. AI Provider Plumbing — Bug Confirmed ✅

### 5.1 Backend contract
All 5 `ai::*` commands in `commands/ai.rs` declare `provider: AiProvider` as a **required (non-Option) parameter**:

```rust
pub async fn ai_assistant(..., provider: AiProvider) -> Result<String, String>
pub async fn ai_continue_chapter(..., provider: AiProvider) -> Result<String, String>
pub async fn ai_analyze_plot(..., provider: AiProvider) -> Result<String, String>
pub async fn ai_test_connection(provider: AiProvider) -> Result<bool, String>
```

Tauri's `#[tauri::command]` macro serializes the JS payload via serde; missing required fields cause `Err("missing field `provider`")` to be returned to the frontend.

### 5.2 Frontend defect — AIDialog.tsx

`src/components/litgraph/AIDialog.tsx` lines 61–92 — `runAI()`:

```ts
const payload: Record<string, unknown> = { project };
if (mode === "continue-chapter") {
  if (selectedNodeId) payload.fromChapterId = selectedNodeId;
  if (customPrompt.trim()) payload.customPrompt = customPrompt.trim();
} else {
  payload.focus = focus;
}
const text = await callApi<string>(cmdName, endpoint, payload);
```

**No `provider` field is added.** In Tauri mode this calls `invoke("ai_continue_chapter", { project, fromChapterId, customPrompt })` → backend deserialization fails → user sees a serde error. The web-preview fallback (`/api/ai/continue-chapter`) would also need provider info, but the route handler doesn't get one either.

### 5.3 Frontend defect — AssistantDialog.tsx

`src/components/litgraph/AssistantDialog.tsx` lines 70–77 — `send()`:

```ts
const text = await callApi<string>("ai_assistant", "/api/ai/assistant", {
  project,
  message: content,
  history: messages.map((m) => ({ role: m.role, content: m.content })),
  selectedNodeId,
});
```

**No `provider` field.** Same failure mode as AIDialog.

### 5.4 Evidence the wire layer is ready
`src/lib/tauri-commands.ts` lines 71–97 defines correct wrappers that DO take `provider: unknown`:

```ts
export async function aiAssistant(project, message, history, selectedNodeId, provider) {
  return invoke("ai_assistant", { project, message, history, selectedNodeId, provider });
}
// similar for aiContinueChapter, aiAnalyzePlot, aiTestConnection, aiListOllamaModels
```

But **neither AIDialog nor AssistantDialog uses these wrappers** — they call `callApi` directly (the Tauri/web dual-path shim in `src/lib/litgraph/api.ts`) and forget to pass `provider`. This is the root cause.

### 5.5 No AI settings UI
There is no `AiSettingsDialog.tsx` in `src/components/litgraph/` (README claims it exists — see `worklog.md` line 18). Without a UI to capture provider config (URL, model, API key), even fixing the payload would leave no source for the `provider` value. The fix requires:
1. Persisting an `AiProvider` JSON in `useLitStore` (or `tauri-plugin-store`).
2. Adding UI to capture/validate it (re-using `ai_test_connection`).
3. Wiring `AIDialog` / `AssistantDialog` to pass it through `callApi`.

### 5.6 Earlier-audit confirmation
This confirms the prior audit (`worklog.md` line 18: *"AIDialog.tsx/AssistantDialog.tsx не передають provider в invoke — AI-команды из Tauri упадут с 'missing field provider'"*) — bug is **still present**, not yet fixed in current HEAD.

---

## 6. Other Observations

### 6.1 Python dependency graph
`commands/ner.rs` and `commands/conflict.rs` both use `run_python_with_text_file` (ner.rs lines 87–164) — a robust temp-dir subprocess runner that:
- Searches `~/.litgraph-venv/bin/python`, then `$LITGRAPH_PYTHON`, then `python3`.
- Writes script + extra_files + input_text to `/tmp/litgraph_scripts_<pid>_<nanos>/`.
- Cleans up with `fs::remove_dir_all`.
- Returns stdout as `String` (JSON expected).

`conflict.rs` (lines 122–128) bundles `ner_extract.py` + `svo_extract.py` alongside `conflict_graph.py` via `include_str!` — this is the right pattern if Layer G ever needs to shell out to Python (it shouldn't).

### 6.2 Two parallel SVO pipelines
| Path | Module | Backend | Returns |
|---|---|---|---|
| Legacy | `commands::ner::extract_svo` | Python spaCy + dependency parsing | `serde_json::Value` (untyped) |
| Layer F.1 | `commands::poler::cmd_extract_svo` | Pure Rust (litgraph-core `SvoParser`) | `Vec<SvoTripletDto>` (typed, camelCase) |

Both are registered and can coexist. Frontend should prefer `cmd_extract_svo` for new UI; `extract_svo` is for legacy code paths.

### 6.3 Two parallel paradox pipelines
| Path | Module | Backend |
|---|---|---|
| Wave 5 | `commands::reasoning::reasoning_validate_text` | `ReasoningCycle` + `LlmBridge::validate_response` (uses `parse_text_fallback` — limited) |
| Layer E | `commands::poler::cmd_detect_paradoxes` | `ParadoxDetector::detect` (uses `ManuscriptAnalysis` from Layer A/B/C) |

These overlap conceptually but use different engines. Layer G should consume the **Layer E** output (`ParadoxReportDto`) since it has richer context (per-chapter character lists + triplet counts) for prompt construction.

### 6.4 Naming convention drift
- `commands::poler` uses `cmd_*` prefix (e.g. `cmd_extract_svo`).
- `commands::reasoning` uses `reasoning_*` prefix (e.g. `reasoning_extract_events`).
- `commands::ner` uses bare verbs (`extract_entities`, `extract_svo`).
- `commands::ai` uses `ai_*` prefix.

A future Layer G command should pick one convention — `cmd_generate_llm_hypotheses` (matching poler.rs) or `llm_bridge_generate_hypotheses` (matching reasoning.rs's module-prefix style). Recommendation: use `cmd_` prefix if it lives in `poler.rs`, or `llm_bridge_*` if it lives in a new module.

### 6.5 Test coverage
- `commands/poler.rs`: 8 unit tests (lines 483–705) — DTO conversions + smoke tests via the public poler API (no Tauri runtime).
- `commands/reasoning.rs`: 7 `#[tokio::test]` tests (lines 452–578) — full async command paths exercised directly (no Tauri runtime).
- `commands/parse_md_full.rs`: 0 tests.
- `commands/conflict.rs`: 0 tests.
- `commands/ner.rs`: 0 tests (Python integration implicit).
- `commands/{parse_md,project,versions,export,ai}.rs`: 0 tests.

### 6.6 Error-message language
All `Err(String)` returns use Ukrainian/Russian human-readable messages (e.g. `"Порожній текст глави — не можна обчислити ε_climax"`). Frontend surfaces them verbatim. Layer G should follow the same convention.

### 6.7 Capability/permission model
`tauri.conf.json` CSP is `null` (per `worklog.md` line 16) and no `capabilities/` directory exists. The 3 registered plugins (`tauri_plugin_store`, `tauri_plugin_dialog`, `tauri_plugin_fs`) are open by default in Tauri 2.x dev mode but need explicit capabilities for production builds. This is a security concern but out of scope for this subagent.

---

## 7. Atomic Findings Summary

| # | Finding | Severity | Action |
|---|---|---|---|
| F-01 | All 29 commands are registered in `generate_handler!` — 0 missing | ✅ OK | None |
| F-02 | `AIDialog.tsx` and `AssistantDialog.tsx` omit `provider` in `callApi` payload; Rust commands declare `provider: AiProvider` as required → AI IPC calls fail with `missing field provider` | 🔴 High | Fix frontend to thread provider through; add `AiSettingsDialog` to capture config |
| F-03 | `tauri-commands.ts` has correct AI wrappers that take `provider`, but the dialogs don't use them — they use `callApi` directly | 🟡 Med | Refactor dialogs to use `aiAssistant`/`aiContinueChapter`/`aiAnalyzePlot` from `tauri-commands.ts` |
| F-04 | Layer G `cmd_generate_llm_hypotheses` should reuse `AiProvider` enum from `crate::ai` (not redefine) to keep wire shape consistent with `ai_*` commands | 🟢 Info | Follow pattern when implementing |
| F-05 | Layer G should live in a new `commands/llm_bridge.rs` module (NOT inside `poler.rs`) to preserve the poler.rs "pure & deterministic" contract documented at poler.rs lines 32–37 | 🟢 Info | Architectural recommendation |
| F-06 | Two overlapping paradox detectors exist (Wave 5 `reasoning_validate_text` vs Layer E `cmd_detect_paradoxes`); Layer G should consume Layer E output for richer context | 🟡 Med | Document choice in Layer G spec |
| F-07 | Two overlapping SVO extractors exist (`ner::extract_svo` Python vs `poler::cmd_extract_svo` Rust); frontend should standardize on `cmd_extract_svo` | 🟡 Low | Migrate callers |
| F-08 | 5 of 11 command modules have **zero** tests (`parse_md_full`, `conflict`, `ner`, `parse_md`, `project`, `versions`, `export`, `ai`) | 🟡 Med | Add unit tests for at least the pure-logic modules (`project`, `versions`, `export`) |
| F-09 | `ner.rs::Entity` / `conflict.rs::ConflictGraph` use per-field `#[serde(rename = "...")]` instead of `#[serde(rename_all = "camelCase")]` — inconsistent with poler/reasoning DTOs | 🟢 Low | Refactor for consistency (breaking wire change — coordinate with frontend) |
| F-10 | `lib.rs` registers 3 plugins but no `capabilities/` directory exists; production Tauri 2.x builds will need explicit capability files for fs/dialog/store | 🔴 High (prod only) | Out of scope; flag for security subagent |

---

## 8. Next Actions

1. **(Layer G)** Implement `commands/llm_bridge.rs` with `cmd_generate_llm_hypotheses(text: String, provider: AiProvider, options: Option<HypothesisOptions>) -> Result<HypothesisReportDto, String>`. Consume `cmd_detect_paradoxes`'s pipeline (refactor into `pub(crate) fn`) and reuse `crate::ai::prompts` for prompt building.
2. **(Provider plumbing fix)** Add `aiProvider: AiProvider | null` to `useLitStore`; persist via `tauri-plugin-store`; build `AiSettingsDialog.tsx` (the one README claims exists); wire `AIDialog`/`AssistantDialog` to pass `provider` through `callApi`.
3. **(Refactor)** Replace direct `callApi("ai_*", ...)` calls in `AIDialog`/`AssistantDialog` with the typed wrappers in `tauri-commands.ts` to centralize the wire contract.
4. **(Tests)** Add unit tests for `commands/project.rs`, `commands/versions.rs`, `commands/export.rs` — pure functions, easy wins.
5. **(Consistency)** Migrate `ner.rs::Entity` and `conflict.rs::Conflict*` to `#[serde(rename_all = "camelCase")]` in a coordinated frontend+backend PR.

---

## 9. End
