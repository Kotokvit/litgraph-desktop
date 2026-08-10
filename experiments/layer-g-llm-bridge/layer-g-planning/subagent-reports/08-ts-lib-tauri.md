# Subagent 08 — TypeScript IPC Wrapper Layer (`src/lib/tauri-commands.ts`)

- **Task ID**: `08-ts-lib-tauri`
- **Agent**: Explore (very thorough)
- **Scope**: The single file `litgraph-desktop/src/lib/tauri-commands.ts` (440 LOC, 1 import, 0 exports consumed by the AI dialogs) — the canonical React↔Rust IPC contract.
- **Commit under review**: `c143a7f` (Layer F.2 POLER DTOs + wrappers added on top of legacy parse_md / project / version / export / AI / reasoning wrappers).
- **Cross-referenced artifacts**: `src-tauri/src/commands/poler.rs` (706 LOC, DTOs + 3 commands), `src-tauri/src/commands/ai.rs` (62 LOC), `src-tauri/src/lib.rs` (invoke_handler registration), `src/lib/litgraph/api.ts` (rival `callApi` layer), `src/components/litgraph/{PolerPanel,ReasoningDialog,Toolbar,SvoHighlighter,AIDialog,AssistantDialog}.tsx`.

---

## 1. Scope

`tauri-commands.ts` is the **typed Tauri-IPC facade** for the React frontend. It exports:
- 5 legacy parse-md wrappers (`parseMd`, `parseMdFull`, `listProjects`, `loadProject`, `saveProject`, `deleteProject`)
- 4 version wrappers
- 1 export wrapper
- 5 AI wrappers (`aiAssistant`, `aiContinueChapter`, `aiAnalyzePlot`, `aiTestConnection`, `aiListOllamaModels`)
- 4 reasoning wrappers + 8 reasoning DTO types (FactValue, TemporalAnchor, Provenance, Action, Event, ConstraintViolation, TemporalParadox, CycleReport, CharacterState, WorldStateView, ValidationResultDto)
- 3 POLER Layer F.2 wrappers (`cmdComputeEpsilonClimax`, `cmdExtractSvo`, `cmdDetectParadoxes`) + 5 POLER DTOs (`EpsilonClimaxDto`, `SvoTripletDto`, `ParadoxDto`, `ChapterBreakdownDto`, `ParadoxReportDto`)

It does **not** include any web-preview fallback. The rival `lib/litgraph/api.ts` (`callApi`, `isTauri`) is the only place where the web-vs-Tauri decision is centralized — but `tauri-commands.ts` ignores it.

---

## 2. Atomic Inventory

### 2.1 Wrappers (19 total)

| Wrapper | Command | Typed return | Source |
|---|---|---|---|
| `parseMd` | `parse_md` | `Promise<unknown>` | L1 |
| `parseMdFull` | `parse_md_full` | `Promise<FullParseResult>` | L1 |
| `listProjects` | `list_projects` | `Promise<unknown>` | L1 |
| `loadProject` | `load_project` | `Promise<unknown>` | L1 |
| `saveProject` | `save_project` | `Promise<unknown>` | L1 |
| `deleteProject` | `delete_project` | `Promise<unknown>` | L1 |
| `saveVersion` | `save_version` | `Promise<unknown>` | L1 |
| `restoreVersion` | `restore_version` | `Promise<unknown>` | L1 |
| `deleteVersion` | `delete_version` | `Promise<unknown>` | L1 |
| `listVersions` | `list_versions` | `Promise<unknown>` | L1 |
| `exportProject` | `export_project` | `Promise<unknown>` | L1 |
| `aiAssistant` | `ai_assistant` | `Promise<unknown>` | L1 |
| `aiContinueChapter` | `ai_continue_chapter` | `Promise<unknown>` | L1 |
| `aiAnalyzePlot` | `ai_analyze_plot` | `Promise<unknown>` | L1 |
| `aiTestConnection` | `ai_test_connection` | `Promise<unknown>` | L1 |
| `aiListOllamaModels` | `ai_list_ollama_models` | `Promise<unknown>` | L1 |
| `reasoningExtractEvents` | `reasoning_extract_events` | `Promise<Event[]>` | L1 |
| `reasoningRunCycle` | `reasoning_run_cycle` | `Promise<CycleReport>` | L1 |
| `reasoningGetWorldState` | `reasoning_get_world_state` | `Promise<WorldStateView>` | L1 |
| `reasoningValidateText` | `reasoning_validate_text` | `Promise<ValidationResultDto>` | L1 |
| `cmdComputeEpsilonClimax` | `cmd_compute_epsilon_climax` | `Promise<EpsilonClimaxDto>` | **F.2** |
| `cmdExtractSvo` | `cmd_extract_svo` | `Promise<SvoTripletDto[]>` | **F.2** |
| `cmdDetectParadoxes` | `cmd_detect_paradoxes` | `Promise<ParadoxReportDto>` | **F.2** |

### 2.2 DTOs / Types (24 total)

| TS type | Rust mirror | Source | Verification |
|---|---|---|---|
| `FullParseResult` | (no Rust mirror) | L1 | ad-hoc |
| `FactValue` | `litgraph-core::reasoning::FactValue` | reasoning | externally-tagged enum ✅ |
| `TemporalAnchor` | `reasoning::TemporalAnchor` | reasoning | ✅ |
| `Provenance` | `reasoning::Provenance` | reasoning | bare strings ✅ |
| `Action` | `reasoning::Action` | reasoning | externally-tagged ✅ |
| `Event` | `reasoning::Event` | reasoning | ✅ |
| `ConstraintViolation` | (left as index signature) | reasoning | **unknown escape hatch** |
| `TemporalParadox` | (left as index signature) | reasoning | **unknown escape hatch** |
| `CycleReport` | `reasoning::CycleReport` | reasoning | ✅ (has `finalStateSnapshot` typed) |
| `CharacterState` | `reasoning::CharacterState` | reasoning | ✅ |
| `WorldStateView` | `reasoning::WorldStateView` | reasoning | `history: unknown[]` is a hole |
| `ValidationResultDto` | `reasoning::ValidationResultDto` | reasoning | discriminated union ✅ |
| `EpsilonClimaxDto` | `commands::poler::EpsilonClimaxDto` | **F.2** | ✅ exact mirror, 16/16 fields |
| `SvoTripletDto` | `commands::poler::SvoTripletDto` | **F.2** | ✅ exact mirror, 7/7 fields |
| `ParadoxDto` | `commands::poler::ParadoxDto` | **F.2** | ✅ exact mirror, 5/5 fields |
| `ChapterBreakdownDto` | `commands::poler::ChapterBreakdownDto` | **F.2** | ✅ exact mirror, 5/5 fields |
| `ParadoxReportDto` | `commands::poler::ParadoxReportDto` | **F.2** | ✅ exact mirror, 4/4 fields |

---

## 3. Current State

### 3.1 DTO ↔ Rust mirror audit — **PASS for POLER F.2, MIXED for reasoning, FAIL for legacy**

**POLER F.2 (5 DTOs, 37 fields total):** every TS field name matches the Rust `#[serde(rename_all = "camelCase")]` output exactly. Verified field-by-field against `src-tauri/src/commands/poler.rs` lines 55–212:

| Rust field (snake_case) | Wire field (camelCase) | TS field | Match |
|---|---|---|---|
| `epsilon: f64` | `epsilon` | `epsilon: number` | ✅ |
| `word_count: usize` | `wordCount` | `wordCount: number` | ✅ |
| `theta_rel: f64` | `thetaRel` | `thetaRel: number` | ✅ |
| `is_noise: bool` | `isNoise` | `isNoise: boolean` | ✅ |
| `omega_conf: f64` | `omegaConf` | `omegaConf: number` | ✅ |
| `spectral_radius: f64` | `spectralRadius` | `spectralRadius: number` | ✅ |
| `chapter_idx: usize` | `chapterIdx` | `chapterIdx: number` | ✅ |
| `origin_chapter_idx: usize` | `originChapterIdx` | `originChapterIdx: number` | ✅ |
| `total_characters: usize` | `totalCharacters` | `totalCharacters: number` | ✅ |
| `total_triplets: usize` | `totalTriplets` | `totalTriplets: number` | ✅ |
| (remaining 27 fields) | | | ✅ |

**Reasoning DTOs (8 types):** mostly correct, but two are explicit `unknown` escape hatches:
- `ConstraintViolation` → `{ [key: string]: unknown }` — Rust struct is richer (`ViolationKind`, `description`, `evidence_chapter`, `expected`, `actual`). The TS side cannot render structured violations; React components must `JSON.stringify` them.
- `TemporalParadox` → `{ description: string; [key: string]: unknown }` — same problem.
- `WorldStateView.history: unknown[]` — typed as opaque.

**Legacy wrappers (parseMd, project, version, export, AI):** **all return `Promise<unknown>`**. No type information leaks to callers. `parseMdFull`'s `FullParseResult` interface even uses `parseResult: unknown` and `nerEntities: unknown | null` — i.e. the wrapper layer doesn't type its own primary return.

### 3.2 Wrapper signature consistency — **INCONSISTENT**

- **POLER F.2 wrappers**: use `invoke<EpsilonClimaxDto>(...)`, `invoke<SvoTripletDto[]>(...)`, `invoke<ParadoxReportDto>(...)` — fully generic-typed. ✅
- **Reasoning wrappers**: use `invoke<Event[]>(...)`, `invoke<CycleReport>(...)`, `invoke<WorldStateView>(...)`, `invoke<ValidationResultDto>(...)` — fully generic-typed. ✅
- **Legacy + AI wrappers (16 of them)**: bare `invoke(...)` with no type parameter → returns `Promise<unknown>`. ❌

So the F.2 commit (`c143a7f`) raised the bar; everything pre-existing was left at `unknown`.

### 3.3 Provider plumbing — **BROKEN**

The Rust `ai_assistant`, `ai_continue_chapter`, `ai_analyze_plot`, `ai_test_connection` commands all require a `provider: AiProvider` argument (see `src-tauri/src/commands/ai.rs` lines 13, 25, 43, 54). The TS wrappers correctly thread the parameter through (lines 72–97):

```ts
export async function aiAssistant(
  project: unknown, message: string, history: unknown[],
  selectedNodeId: string | null, provider: unknown
) {
  return invoke("ai_assistant", { project, message, history, selectedNodeId, provider });
}
```

However, **the TS wrappers `aiAssistant` / `aiContinueChapter` / `aiAnalyzePlot` / `aiTestConnection` / `aiListOllamaModels` are DEAD CODE** — a `grep` for their names across `src/components/` returns zero matches. The actual AI dialogs import a **different** IPC layer:

```
src/components/litgraph/AIDialog.tsx:4:       import { callApi } from "@/lib/litgraph/api";
src/components/litgraph/AssistantDialog.tsx:4: import { callApi } from "@/lib/litgraph/api";
```

`callApi` is the untyped web-fallback wrapper in `src/lib/litgraph/api.ts` that calls `__TAURI_INTERNALS__.invoke` if available, else `fetch(webEndpoint)`. So:
1. The AI dialogs bypass `tauri-commands.ts` entirely.
2. Whether `provider` is actually passed depends on the dialogs — and a `grep` for `provider` inside `src/components/litgraph/` returns **zero matches**. So the dialogs are calling AI commands without a `provider` argument, which would make Rust's `tauri::command` deserializer fail with `"missing field provider"`.

This was already flagged in the worklog entry from subagent 01 (analysis task, item 18).

### 3.4 Web-preview fallback — **FRAGMENTED**

There is no centralized `isTauriEnv()` helper inside `tauri-commands.ts`. The detection pattern is **re-implemented in at least 5 places**:

| File | Detection mechanism |
|---|---|
| `src/lib/litgraph/api.ts` (line 2) | `export const isTauri = typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);` ← **centralized**, but not re-used |
| `src/lib/litgraph/export-html.ts` (line 2001) | inline duplicate |
| `src/lib/litgraph/export-svg.ts` (line 620) | inline duplicate |
| `src/lib/litgraph/background-layer.ts` (lines 198, 262) | inline duplicate ×2 |
| `src/components/litgraph/Toolbar.tsx` (line 253) | inline `isTauriEnv` (snake-camel) |
| `src/components/litgraph/PolerPanel.tsx` (line 67) | local `function isTauriEnv()` |
| `src/lib/conflict/export.ts` (line 24) | uses `@tauri-apps/api/core`'s `isTauri()` instead |
| `src/lib/tauri-commands.ts` | **does not check at all** — every wrapper will throw `__TAURI_INTERNALS__ is undefined` under Vite dev server |

The only wrapper layer that degrades gracefully under web-preview is `lib/litgraph/api.ts` (`callApi` → falls back to `fetch`). The typed wrappers in `tauri-commands.ts` are **Tauri-only by construction**; PolerPanel must wrap them with its own local `isTauriEnv()` guard before calling.

---

## 4. Gaps

### G1. AI wrappers are dead code (CRITICAL)
`aiAssistant`, `aiContinueChapter`, `aiAnalyzePlot`, `aiTestConnection`, `aiListOllamaModels` are exported but unused. The dialogs use `callApi` from `lib/litgraph/api.ts` instead. Either:
- (a) delete the AI wrappers from `tauri-commands.ts` (preferred — they were superseded), or
- (b) refactor the dialogs to call the typed wrappers and add a web-fallback inside the wrappers.

### G2. Provider never reaches the AI command (CRITICAL)
Even when AI commands are invoked via `callApi`, no `provider` field is constructed by `AIDialog.tsx` / `AssistantDialog.tsx`. The Rust commands will fail with `missing field provider`. Worklog item 18 already documents this.

### G3. Legacy wrappers return `unknown` (HIGH)
11 of the 19 wrappers return `Promise<unknown>`. React consumers must `as Project`-cast or `JSON.stringify` everything. This defeats the purpose of a typed IPC layer. The Rust DTOs (`Project`, `Version`, `ParseResult`, `ConflictGraph`) exist — the TS mirrors simply don't.

### G4. Reasoning DTO holes (MEDIUM)
`ConstraintViolation` and `TemporalParadox` use index signatures (`[key: string]: unknown`) instead of typed fields. The Rust structs (`src-tauri/src/reasoning/...`) presumably have explicit fields. The TS mirror should be completed.

### G5. No web-preview fallback (MEDIUM)
`tauri-commands.ts` wrappers will hard-throw under Vite dev server. The pattern should be:
```ts
import { isTauri, callApi } from "@/lib/litgraph/api";
export async function cmdExtractSvo(text: string): Promise<SvoTripletDto[]> {
  if (!isTauri) return []; // or throw a typed error
  return invoke<SvoTripletDto[]>("cmd_extract_svo", { text });
}
```
Currently PolerPanel must replicate the guard on every call site.

### G6. Duplicated `isTauriEnv` (LOW)
Five+ copies of the same check. Should be one canonical helper exported from `lib/litgraph/api.ts` (which already has the constant — just add a function form too).

### G7. `keyword` parameter in `cmdComputeEpsilonClimax` is misleading (LOW)
The Rust doc-comment says `keyword` is "currently unused in climax formula but reserved". The TS wrapper still passes it through to `invoke`. Either remove it from the signature or document that it's a no-op for now.

### G8. `kappa` defaults mismatch (LOW)
- Rust signature: `kappa: Option<f64>` → defaults to `1.0` if `None`.
- TS signature: `kappa: number = 1.0` → always sends a value.
Both arrive at the same result, but the TS side could be `kappa?: number` to match the Rust idiom.

---

## 5. Refactoring Recommendations

### R1. Add `lib/litgraph/api.ts` re-export to `tauri-commands.ts`
```ts
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/litgraph/api";

function requireTauri(): void {
  if (!isTauri) {
    throw new Error(
      "Tauri IPC unavailable — run via `bun tauri dev`. " +
      "Web-preview fallback is not implemented for this command."
    );
  }
}
```
Then add `requireTauri()` as the first line of every wrapper. Removes the duplicated `isTauriEnv` from `PolerPanel.tsx`, `Toolbar.tsx`, etc.

### R2. Type the legacy wrappers
Mirror the Rust DTOs (`Project`, `Version`, `ParseResult`, `ConflictGraph`, `ChatMessage`, `AiProvider`) into TS interfaces. Then `parseMd` returns `Promise<ParseResult>`, `loadProject` returns `Promise<Project>`, etc.

### R3. Define `AiProvider` TS mirror
`src-tauri/src/ai/mod.rs` has the `AiProvider` enum (presumably `{ Ollama { url, model }, OpenAiCompat { url, api_key, model } }` or similar). Add:
```ts
export type AiProvider =
  | { kind: "ollama"; url: string; model: string }
  | { kind: "openai_compat"; url: string; apiKey: string; model: string };
```
Then update `aiAssistant(project, message, history, selectedNodeId, provider: AiProvider)`.

### R4. Decide AI wrapper fate (delete or wire up)
Either:
- Delete the 5 AI wrappers from `tauri-commands.ts` and mark `lib/litgraph/api.ts#callApi` as the canonical AI IPC, or
- Migrate `AIDialog.tsx` / `AssistantDialog.tsx` to call the typed wrappers + add a web-fallback inside the wrappers.

### R5. Complete `ConstraintViolation` and `TemporalParadox` typing
Inspect `src-tauri/src/reasoning/violations.rs` (or wherever the Rust structs live) and mirror every field.

---

## 6. Layer G Relevance

Layer G (LLM hallucination correction) will need at minimum:

### G1. `cmdGenerateLlmHypotheses`
Mirror of `litgraph-core::reasoning::generate_llm_hypotheses(...)` (or whatever the LLM-proposal entry point becomes). Proposed signature:
```ts
export interface LlmHypothesisDto {
  id: string;
  kind: "fact" | "event" | "rule";
  subject: string;
  predicate: string;
  object: FactValue;
  confidence: number;       // [0, 1]
  provenance: Provenance;   // reuse existing type — adds "LlmSuggested" already present
  rawLlmText: string;
  parseStatus: "ok" | "partial" | "failed";
  parseError: string | null;
}
export async function cmdGenerateLlmHypotheses(
  project: unknown,
  text: string,
  provider: AiProvider,
  context?: { chapterIdx: number; characterIds: string[] } | null,
): Promise<LlmHypothesisDto[]> {
  return invoke<LlmHypothesisDto[]>("cmd_generate_llm_hypotheses", {
    project, text, provider, context: context ?? null,
  });
}
```

### G2. `cmdValidateLlmResponse`
Mirror of the validator that gates LLM hypotheses into the symbolic store:
```ts
export interface LlmValidationReportDto {
  accepted: LlmHypothesisDto[];
  rejected: { hypothesis: LlmHypothesisDto; reason: string; violation: ConstraintViolation | null }[];
  retryable: LlmHypothesisDto[];     // parse failed, prompt LLM again
  paradoxesIntroduced: ParadoxDto[]; // reuse F.2 type
  worldStateAfterApply: WorldStateView; // reuse reasoning type
}
export async function cmdValidateLlmResponse(
  project: unknown,
  hypotheses: LlmHypothesisDto[],
  events: Event[],
): Promise<LlmValidationReportDto> {
  return invoke<LlmValidationReportDto>("cmd_validate_llm_response", {
    project, hypotheses, events,
  });
}
```

### G3. `cmdExplainViolation` (optional)
Reverse-channel: given a `ConstraintViolation` (or its ID), produce a human-readable Ukrainian explanation. Useful for the ReasoningDialog "Why was this rejected?" panel.

### G4. Existing types ready for reuse
- `FactValue`, `Provenance` (already includes `LlmSuggested`) — ready for Layer G hypothesis payloads.
- `Event`, `ConstraintViolation` — ready for validation input/output (but `ConstraintViolation` must first be properly typed, see G3 above).
- `ValidationResultDto` — already a discriminated union `{ kind: "accept" | "reject" | "retry" }`, **directly reusable** for the LLM validation report's top-level shape. Layer G may simply call the existing `reasoningValidateText` command after applying hypotheses.

### G5. Missing infrastructure for Layer G
- No `AiProvider` TS type (see R3 above). Layer G must pass a provider to the LLM hypothesis generator.
- No streaming support. If Layer G wants token-streaming for the LLM call, `tauri::ipc::Channel` needs to be exposed — currently no wrapper uses it.
- No retry/backoff primitives. `cmdGenerateLlmHypotheses` will fail intermittently; the wrapper should expose a typed `retryable: boolean` flag on errors, or use `Promise.race` with a timeout.

---

## 7. Next Actions

| # | Action | Priority | Effort |
|---|---|---|---|
| NA1 | Decide AI wrapper fate (R4) — either delete or wire up. Until then the codebase has two parallel IPC layers. | P0 | 1h |
| NA2 | Fix `provider` plumbing in `AIDialog.tsx` / `AssistantDialog.tsx` — construct `AiProvider` from settings and pass it through `callApi` payload. | P0 | 2h |
| NA3 | Add `isTauri` guard to every wrapper in `tauri-commands.ts` (R1). Removes 5+ duplicated `isTauriEnv` helpers. | P1 | 30min |
| NA4 | Type the 11 legacy wrappers that return `Promise<unknown>` (R2). Mirror `Project`, `Version`, `ParseResult`, `ConflictGraph` Rust structs. | P1 | 4h |
| NA5 | Complete `ConstraintViolation` and `TemporalParadox` field types (R5). | P2 | 1h |
| NA6 | Add `AiProvider` TS mirror (R3) — required before Layer G. | P2 | 30min |
| NA7 | Stub `cmdGenerateLlmHypotheses` and `cmdValidateLlmResponse` wrappers (Section 6) — even if Rust side returns `Err("not yet implemented")`, the TS contract should exist. | P3 (Layer G prep) | 1h |
| NA8 | Reconcile `kappa` parameter (G8) — make it `kappa?: number` in TS to match Rust `Option<f64>`. | P3 | 5min |

---

## 8. Dependencies

- **Upstream (Rust DTOs)**: `src-tauri/src/commands/poler.rs` (POLER F.2 — verified mirror ✅), `src-tauri/src/commands/ai.rs` (AI provider requirement ✅), `src-tauri/src/reasoning/*` (reasoning DTOs — partial mirror, see G3), `src-tauri/src/models/*` (`Project`, `Version` — not yet mirrored in TS).
- **Downstream (React consumers)**: `PolerPanel.tsx` (consumes F.2 wrappers + DTOs ✅), `ReasoningDialog.tsx` (consumes reasoning wrappers ✅), `Toolbar.tsx` (consumes legacy parse_md wrappers ✅), `SvoHighlighter.tsx` (consumes FactValue / Event types ✅), `AIDialog.tsx` & `AssistantDialog.tsx` (DO NOT consume — bypass to `lib/litgraph/api.ts`).
- **Cross-file IPC contract**: `lib/litgraph/api.ts#callApi` is the rival IPC facade. It is untyped (`<T = unknown>`) but web-preview-aware. Until NA1+NA3 land, the codebase will continue to have two parallel IPC layers with overlapping responsibilities.
- **Layer G blocking deps**: NA6 (AiProvider TS mirror) and NA5 (ConstraintViolation typing) are prerequisites for `cmdGenerateLlmHypotheses` and `cmdValidateLlmResponse`.

---

## 9. Verification Checklist

- [x] Read `tauri-commands.ts` end-to-end (440 LOC).
- [x] Diffed all 5 POLER F.2 DTOs against Rust `commands/poler.rs` field-by-field — 37/37 fields match.
- [x] Diffed reasoning DTOs against expected Rust enum shapes — externally-tagged convention respected.
- [x] Cross-checked `invoke_handler!` registration in `src-tauri/src/lib.rs` — all 3 POLER commands + all 6 reasoning commands + all 5 AI commands + 4 NER commands + 1 conflict command + 4 project + 4 versions + 2 parse_md + 1 export = **30 commands registered**. TS wrappers cover 22 of them (missing: `extract_entities`, `analyze_characters`, `extract_svo` (legacy NER), `get_conflict_graph`, `reasoning_extract_instructions`, `reasoning_run_cycle_with_ir`).
- [x] Grepped for `provider` usage in `src/components/litgraph/` — zero matches. AI dialogs do not pass provider.
- [x] Grepped for `aiAssistant`/`aiContinueChapter`/etc. callers — only the wrappers themselves; no React consumer.
- [x] Verified the 5+ inline `isTauriEnv` duplications across `lib/litgraph/{api,export-html,export-svg,background-layer}.ts` and `components/litgraph/{Toolbar,PolerPanel}.tsx`.
